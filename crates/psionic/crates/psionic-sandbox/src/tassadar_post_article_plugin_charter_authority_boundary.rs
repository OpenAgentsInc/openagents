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
    build_tassadar_post_article_plugin_capability_boundary_report,
    TassadarPostArticlePluginCapabilityBoundaryReport,
    TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_charter_authority_boundary_report.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-plugin-charter-authority-boundary.sh";

const BRIDGE_CONTRACT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_bridge_contract_report.json";
const CONTROL_PLANE_PROOF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_control_plane_decision_provenance_proof_report.json";
const POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_turing_completeness_closeout_audit_report.json";
const WORLD_MOUNT_COMPATIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_world_mount_compatibility_report.json";
const BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_internal_compute_profile_publication_report.json";
const MODULE_PROMOTION_STATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_promotion_state_report.json";
const MODULE_TRUST_ISOLATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_trust_isolation_report.json";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const PLUGIN_SYSTEM_AND_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const CANONICAL_ARCHITECTURE_BOUNDARY_REF: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const CANONICAL_ARCHITECTURE_ANCHOR_CRATE: &str = "psionic-transformer";
const ARTICLE_CLOSEOUT_PROFILE_ID: &str = "tassadar.internal_compute.article_closeout.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginCharterAuthorityBoundaryStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginCharterDependencyClass {
    ProofCarrying,
    GovernanceDependency,
    PublicationDependency,
    CompatibilityDependency,
    ObservationalContext,
    DesignInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCharterMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub computational_model_runtime_contract_id: String,
    pub computational_model_runtime_contract_digest: String,
    pub computational_model_substrate_model_id: String,
    pub computational_model_substrate_model_digest: String,
    pub control_plane_proof_report_id: String,
    pub control_plane_proof_report_digest: String,
    pub plugin_boundary_report_id: String,
    pub plugin_boundary_report_digest: String,
    pub closeout_audit_report_id: String,
    pub closeout_audit_report_digest: String,
    pub canonical_architecture_anchor_crate: String,
    pub canonical_architecture_boundary_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCharterDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticlePluginCharterDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCharterLawRow {
    pub law_id: String,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginStateClassRow {
    pub state_class_id: String,
    pub owner_plane: String,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginGovernanceRow {
    pub governance_id: String,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCharterValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCharterAuthorityBoundaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub bridge_contract_report_ref: String,
    pub control_plane_decision_provenance_proof_report_ref: String,
    pub plugin_capability_boundary_report_ref: String,
    pub post_article_turing_completeness_closeout_audit_report_ref: String,
    pub world_mount_compatibility_report_ref: String,
    pub broad_internal_compute_profile_publication_report_ref: String,
    pub module_promotion_state_report_ref: String,
    pub module_trust_isolation_report_ref: String,
    pub plugin_system_audit_ref: String,
    pub post_article_turing_audit_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding: TassadarPostArticlePluginCharterMachineIdentityBinding,
    pub dependency_rows: Vec<TassadarPostArticlePluginCharterDependencyRow>,
    pub law_rows: Vec<TassadarPostArticlePluginCharterLawRow>,
    pub state_class_rows: Vec<TassadarPostArticlePluginStateClassRow>,
    pub governance_rows: Vec<TassadarPostArticlePluginGovernanceRow>,
    pub validation_rows: Vec<TassadarPostArticlePluginCharterValidationRow>,
    pub charter_status: TassadarPostArticlePluginCharterAuthorityBoundaryStatus,
    pub charter_green: bool,
    pub current_publication_posture: String,
    pub first_plugin_tranche_posture: String,
    pub internal_only_plugin_posture: bool,
    pub proof_vs_audit_distinction_frozen: bool,
    pub observer_model_frozen: bool,
    pub three_plane_contract_frozen: bool,
    pub adversarial_host_model_frozen: bool,
    pub state_class_split_frozen: bool,
    pub host_executes_but_does_not_decide: bool,
    pub semantic_preservation_required: bool,
    pub choice_set_integrity_frozen: bool,
    pub resource_transparency_frozen: bool,
    pub scheduling_ownership_frozen: bool,
    pub no_externalized_learning_frozen: bool,
    pub plugin_language_boundary_frozen: bool,
    pub first_plugin_tranche_closed_world: bool,
    pub anti_interpreter_smuggling_frozen: bool,
    pub downward_non_influence_frozen: bool,
    pub governance_receipts_required: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub deferred_issue_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticlePluginCharterAuthorityBoundaryReportError {
    #[error(transparent)]
    PluginBoundary(#[from] crate::TassadarPostArticlePluginCapabilityBoundaryReportError),
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

pub fn build_tassadar_post_article_plugin_charter_authority_boundary_report() -> Result<
    TassadarPostArticlePluginCharterAuthorityBoundaryReport,
    TassadarPostArticlePluginCharterAuthorityBoundaryReportError,
> {
    let bridge: BridgeContractFixture = read_repo_json(BRIDGE_CONTRACT_REPORT_REF)?;
    let control_plane: ControlPlaneProofFixture = read_repo_json(CONTROL_PLANE_PROOF_REPORT_REF)?;
    let plugin_boundary = build_tassadar_post_article_plugin_capability_boundary_report()?;
    let closeout: PostArticleTuringCloseoutFixture =
        read_repo_json(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF)?;
    let world_mount: WorldMountCompatibilityFixture =
        read_repo_json(WORLD_MOUNT_COMPATIBILITY_REPORT_REF)?;
    let publication: BroadInternalComputeProfilePublicationFixture =
        read_repo_json(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF)?;
    let promotion: ModulePromotionStateFixture = read_repo_json(MODULE_PROMOTION_STATE_REPORT_REF)?;
    let trust: ModuleTrustIsolationFixture = read_repo_json(MODULE_TRUST_ISOLATION_REPORT_REF)?;

    let machine_identity_binding = TassadarPostArticlePluginCharterMachineIdentityBinding {
        machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
        canonical_model_id: bridge.bridge_machine_identity.canonical_model_id.clone(),
        canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
        canonical_route_descriptor_digest: bridge
            .bridge_machine_identity
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: bridge
            .bridge_machine_identity
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: bridge
            .bridge_machine_identity
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: bridge.bridge_machine_identity.continuation_contract_id.clone(),
        continuation_contract_digest: bridge
            .bridge_machine_identity
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: bridge.computational_model_statement.statement_id.clone(),
        computational_model_runtime_contract_id: bridge
            .computational_model_statement
            .runtime_contract_id
            .clone(),
        computational_model_runtime_contract_digest: bridge
            .computational_model_statement
            .runtime_contract_digest
            .clone(),
        computational_model_substrate_model_id: bridge
            .computational_model_statement
            .substrate_model_id
            .clone(),
        computational_model_substrate_model_digest: bridge
            .computational_model_statement
            .substrate_model_digest
            .clone(),
        control_plane_proof_report_id: control_plane.report_id.clone(),
        control_plane_proof_report_digest: control_plane.report_digest.clone(),
        plugin_boundary_report_id: plugin_boundary.report_id.clone(),
        plugin_boundary_report_digest: plugin_boundary.report_digest.clone(),
        closeout_audit_report_id: closeout.report_id.clone(),
        closeout_audit_report_digest: closeout.report_digest.clone(),
        canonical_architecture_anchor_crate: String::from(CANONICAL_ARCHITECTURE_ANCHOR_CRATE),
        canonical_architecture_boundary_ref: String::from(CANONICAL_ARCHITECTURE_BOUNDARY_REF),
        detail: format!(
            "machine_identity_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` computational_model_statement_id=`{}` control_plane_proof_report_id=`{}` plugin_boundary_report_id=`{}` and closeout_audit_report_id=`{}` stay bound to `{}`.",
            bridge.bridge_machine_identity.machine_identity_id,
            bridge.bridge_machine_identity.canonical_model_id,
            bridge.bridge_machine_identity.canonical_route_id,
            bridge.computational_model_statement.statement_id,
            control_plane.report_id,
            plugin_boundary.report_id,
            closeout.report_id,
            CANONICAL_ARCHITECTURE_ANCHOR_CRATE,
        ),
    };

    let current_publication_posture =
        String::from("internal_only_until_later_plugin_platform_gates");
    let first_plugin_tranche_posture = plugin_boundary.first_plugin_tranche_posture.clone();
    let internal_only_plugin_posture = publication.current_served_profile_id
        == ARTICLE_CLOSEOUT_PROFILE_ID
        && publication.published_profile_ids == vec![String::from(ARTICLE_CLOSEOUT_PROFILE_ID)]
        && !publication.suppressed_profile_ids.is_empty();
    let proof_vs_audit_distinction_frozen = bridge
        .computational_model_statement
        .proof_class_statement
        .contains("remain distinct proof classes");
    let observer_model_frozen = control_plane.observer_model.green
        && control_plane.observer_model.replay_receipt_required
        && control_plane
            .observer_model
            .gate_verdict_bound_to_machine_identity;
    let three_plane_contract_frozen = plugin_boundary.tcm_v1_substrate_retained
        && plugin_boundary.plugin_capability_plane_reserved
        && control_plane.control_plane_ownership_green;
    let adversarial_host_model_frozen = control_plane.hidden_control_channel_rows.len() >= 6
        && !control_plane
            .training_inference_boundary
            .runtime_adaptation_allowed
        && control_plane
            .information_boundary
            .model_hidden_signal_ids
            .iter()
            .any(|signal| signal == "helper_selection");
    let host_executes_but_does_not_decide = control_plane.control_plane_ownership_green
        && control_plane.decision_provenance_proof_complete
        && closeout.control_plane_proof_part_of_truth_carrier;
    let semantic_preservation_required = closeout.canonical_route_truth_carrier
        && closeout.control_plane_proof_part_of_truth_carrier
        && plugin_boundary.downward_non_influence_reserved;
    let choice_set_integrity_frozen = plugin_boundary.choice_set_integrity_reserved
        && bridge
            .reserved_later_invariant_ids
            .iter()
            .any(|id| id == "choice_set_integrity");
    let resource_transparency_frozen = plugin_boundary.resource_transparency_reserved
        && bridge
            .reserved_later_invariant_ids
            .iter()
            .any(|id| id == "resource_transparency");
    let scheduling_ownership_frozen = plugin_boundary.scheduling_ownership_reserved
        && bridge
            .reserved_later_invariant_ids
            .iter()
            .any(|id| id == "scheduling_ownership");
    let no_externalized_learning_frozen = !control_plane
        .training_inference_boundary
        .runtime_adaptation_allowed
        && !control_plane
            .training_inference_boundary
            .telemetry_decision_authority
        && !control_plane
            .training_inference_boundary
            .logging_decision_authority
        && !control_plane
            .training_inference_boundary
            .cache_behavior_decision_authority;
    let plugin_language_boundary_frozen = plugin_boundary.plugin_execution_layer_separate
        && world_mount.allowed_case_count >= 1
        && world_mount.denied_case_count >= 1;
    let first_plugin_tranche_closed_world = plugin_boundary
        .closed_world_operator_curated_first_tranche_required
        && first_plugin_tranche_posture == "closed_world_operator_curated_only_until_audited";
    let anti_interpreter_smuggling_frozen = plugin_boundary.plugin_execution_layer_separate
        && !closeout.weighted_plugin_control_allowed
        && control_plane.hidden_control_channel_rows.len() >= 6;
    let downward_non_influence_frozen = plugin_boundary.downward_non_influence_reserved;
    let governance_receipts_required = promotion.challenge_open_count >= 1
        && promotion.quarantined_count >= 1
        && trust.refused_case_count >= 1
        && trust.privilege_escalation_refusal_count >= 1
        && !publication.suppressed_profile_ids.is_empty();
    let rebase_claim_allowed = closeout.rebase_claim_allowed;
    let plugin_capability_claim_allowed = false;
    let weighted_plugin_control_allowed = false;
    let plugin_publication_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;

    let dependency_rows = build_dependency_rows(
        &bridge,
        &control_plane,
        &plugin_boundary,
        &closeout,
        &world_mount,
        &publication,
        &promotion,
        &trust,
        internal_only_plugin_posture,
    );
    let law_rows = build_law_rows(
        &bridge,
        &plugin_boundary,
        &closeout,
        internal_only_plugin_posture,
        proof_vs_audit_distinction_frozen,
        observer_model_frozen,
        three_plane_contract_frozen,
        adversarial_host_model_frozen,
        host_executes_but_does_not_decide,
        semantic_preservation_required,
        choice_set_integrity_frozen,
        resource_transparency_frozen,
        scheduling_ownership_frozen,
        no_externalized_learning_frozen,
        plugin_language_boundary_frozen,
        first_plugin_tranche_closed_world,
        anti_interpreter_smuggling_frozen,
        downward_non_influence_frozen,
    );
    let state_class_rows = build_state_class_rows(&control_plane, &plugin_boundary, &world_mount);
    let state_class_split_frozen = state_class_rows.iter().all(|row| row.green);
    let governance_rows = build_governance_rows(
        &plugin_boundary,
        &publication,
        &promotion,
        &trust,
        &world_mount,
        internal_only_plugin_posture,
    );
    let validation_rows = build_validation_rows(
        &bridge,
        &control_plane,
        &plugin_boundary,
        &closeout,
        &world_mount,
        &publication,
        &promotion,
        &trust,
        no_externalized_learning_frozen,
        resource_transparency_frozen,
        scheduling_ownership_frozen,
        host_executes_but_does_not_decide,
        semantic_preservation_required,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        weighted_plugin_control_allowed,
        plugin_publication_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
    );

    let charter_green = dependency_rows.iter().all(|row| row.satisfied)
        && law_rows.iter().all(|row| row.green)
        && state_class_rows.iter().all(|row| row.green)
        && governance_rows.iter().all(|row| row.green)
        && validation_rows.iter().all(|row| row.green)
        && internal_only_plugin_posture
        && proof_vs_audit_distinction_frozen
        && observer_model_frozen
        && three_plane_contract_frozen
        && adversarial_host_model_frozen
        && state_class_split_frozen
        && host_executes_but_does_not_decide
        && semantic_preservation_required
        && choice_set_integrity_frozen
        && resource_transparency_frozen
        && scheduling_ownership_frozen
        && no_externalized_learning_frozen
        && plugin_language_boundary_frozen
        && first_plugin_tranche_closed_world
        && anti_interpreter_smuggling_frozen
        && downward_non_influence_frozen
        && governance_receipts_required
        && rebase_claim_allowed
        && !plugin_capability_claim_allowed
        && !weighted_plugin_control_allowed
        && !plugin_publication_allowed
        && !served_public_universality_allowed
        && !arbitrary_software_capability_allowed;
    let charter_status = if charter_green {
        TassadarPostArticlePluginCharterAuthorityBoundaryStatus::Green
    } else {
        TassadarPostArticlePluginCharterAuthorityBoundaryStatus::Incomplete
    };

    let mut report = TassadarPostArticlePluginCharterAuthorityBoundaryReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_plugin_charter_authority_boundary.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_CHECKER_REF,
        ),
        bridge_contract_report_ref: String::from(BRIDGE_CONTRACT_REPORT_REF),
        control_plane_decision_provenance_proof_report_ref: String::from(
            CONTROL_PLANE_PROOF_REPORT_REF,
        ),
        plugin_capability_boundary_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF,
        ),
        post_article_turing_completeness_closeout_audit_report_ref: String::from(
            POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF,
        ),
        world_mount_compatibility_report_ref: String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
        broad_internal_compute_profile_publication_report_ref: String::from(
            BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
        ),
        module_promotion_state_report_ref: String::from(MODULE_PROMOTION_STATE_REPORT_REF),
        module_trust_isolation_report_ref: String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
        plugin_system_audit_ref: String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
        post_article_turing_audit_ref: String::from(POST_ARTICLE_TURING_AUDIT_REF),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        supporting_material_refs: vec![
            String::from(BRIDGE_CONTRACT_REPORT_REF),
            String::from(CONTROL_PLANE_PROOF_REPORT_REF),
            String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
            String::from(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF),
            String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
            String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
            String::from(MODULE_PROMOTION_STATE_REPORT_REF),
            String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
            String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            String::from(POST_ARTICLE_TURING_AUDIT_REF),
            String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            String::from(CANONICAL_ARCHITECTURE_BOUNDARY_REF),
        ],
        machine_identity_binding,
        dependency_rows,
        law_rows,
        state_class_rows,
        governance_rows,
        validation_rows,
        charter_status,
        charter_green,
        current_publication_posture,
        first_plugin_tranche_posture,
        internal_only_plugin_posture,
        proof_vs_audit_distinction_frozen,
        observer_model_frozen,
        three_plane_contract_frozen,
        adversarial_host_model_frozen,
        state_class_split_frozen,
        host_executes_but_does_not_decide,
        semantic_preservation_required,
        choice_set_integrity_frozen,
        resource_transparency_frozen,
        scheduling_ownership_frozen,
        no_externalized_learning_frozen,
        plugin_language_boundary_frozen,
        first_plugin_tranche_closed_world,
        anti_interpreter_smuggling_frozen,
        downward_non_influence_frozen,
        governance_receipts_required,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        weighted_plugin_control_allowed,
        plugin_publication_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        deferred_issue_ids: Vec::new(),
        claim_boundary: String::from(
            "this report freezes the bounded post-article plugin charter above the canonical rebased Transformer carrier without mutating `TCM.v1` or widening the current public claim surface. It binds the plugin lane to one canonical machine identity and computational-model statement, inherits the pre-plugin control-plane proof as a hard dependency, freezes operator/internal-only publication posture, explicit state ownership, semantic-preservation and scheduling laws, adversarial-host and anti-interpreter-smuggling posture, and governance receipts, and still leaves weighted plugin capability, plugin publication, served/public universality, and arbitrary software capability blocked until later manifest/runtime/controller/platform issues land.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "post-article plugin charter report binds machine_identity_id=`{}`, canonical_route_id=`{}`, charter_status={:?}, dependency_rows={}, law_rows={}, governance_rows={}, rebase_claim_allowed={}, plugin_capability_claim_allowed={}, and deferred_issue_ids={}.",
        report.machine_identity_binding.machine_identity_id,
        report.machine_identity_binding.canonical_route_id,
        report.charter_status,
        report.dependency_rows.len(),
        report.law_rows.len(),
        report.governance_rows.len(),
        report.rebase_claim_allowed,
        report.plugin_capability_claim_allowed,
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_charter_authority_boundary_report|",
        &report,
    );
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn build_dependency_rows(
    bridge: &BridgeContractFixture,
    control_plane: &ControlPlaneProofFixture,
    plugin_boundary: &TassadarPostArticlePluginCapabilityBoundaryReport,
    closeout: &PostArticleTuringCloseoutFixture,
    world_mount: &WorldMountCompatibilityFixture,
    publication: &BroadInternalComputeProfilePublicationFixture,
    promotion: &ModulePromotionStateFixture,
    trust: &ModuleTrustIsolationFixture,
    internal_only_plugin_posture: bool,
) -> Vec<TassadarPostArticlePluginCharterDependencyRow> {
    vec![
        TassadarPostArticlePluginCharterDependencyRow {
            dependency_id: String::from("post_article_plugin_capability_boundary"),
            dependency_class: TassadarPostArticlePluginCharterDependencyClass::ProofCarrying,
            satisfied: plugin_boundary.boundary_green
                && plugin_boundary.rebase_claim_allowed
                && !plugin_boundary.plugin_capability_claim_allowed,
            source_ref: String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
            bound_report_id: Some(plugin_boundary.report_id.clone()),
            bound_report_digest: Some(plugin_boundary.report_digest.clone()),
            detail: String::from(
                "the charter inherits the already-green plugin-capability boundary instead of reconstructing the capability-plane split ad hoc.",
            ),
        },
        TassadarPostArticlePluginCharterDependencyRow {
            dependency_id: String::from("bridge_machine_identity_and_computational_model"),
            dependency_class: TassadarPostArticlePluginCharterDependencyClass::ProofCarrying,
            satisfied: bridge.computational_model_statement.canonical_machine_identity_id
                == bridge.bridge_machine_identity.machine_identity_id
                && bridge.computational_model_statement.substrate_model_id == "tcm.v1",
            source_ref: String::from(BRIDGE_CONTRACT_REPORT_REF),
            bound_report_id: Some(bridge.report_id.clone()),
            bound_report_digest: Some(bridge.report_digest.clone()),
            detail: String::from(
                "the charter inherits one canonical machine identity and one computational-model statement from the rebased bridge instead of mutating `TCM.v1`.",
            ),
        },
        TassadarPostArticlePluginCharterDependencyRow {
            dependency_id: String::from("control_plane_decision_provenance_proof"),
            dependency_class: TassadarPostArticlePluginCharterDependencyClass::ProofCarrying,
            satisfied: control_plane.control_plane_ownership_green
                && control_plane.decision_provenance_proof_complete,
            source_ref: String::from(CONTROL_PLANE_PROOF_REPORT_REF),
            bound_report_id: Some(control_plane.report_id.clone()),
            bound_report_digest: Some(control_plane.report_digest.clone()),
            detail: String::from(
                "plugin control inheritance stays downstream of the pre-plugin control-plane proof rather than being claimed fresh here.",
            ),
        },
        TassadarPostArticlePluginCharterDependencyRow {
            dependency_id: String::from("post_article_turing_completeness_closeout"),
            dependency_class: TassadarPostArticlePluginCharterDependencyClass::ProofCarrying,
            satisfied: closeout.closeout_green
                && closeout.historical_tas_156_still_stands
                && closeout.canonical_route_truth_carrier
                && closeout.control_plane_proof_part_of_truth_carrier,
            source_ref: String::from(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF),
            bound_report_id: Some(closeout.report_id.clone()),
            bound_report_digest: Some(closeout.report_digest.clone()),
            detail: String::from(
                "the charter inherits the post-article closeout posture and keeps wider plugin/public claims blocked.",
            ),
        },
        TassadarPostArticlePluginCharterDependencyRow {
            dependency_id: String::from("local_plugin_system_spec"),
            dependency_class: TassadarPostArticlePluginCharterDependencyClass::DesignInput,
            satisfied: true,
            source_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            bound_report_id: None,
            bound_report_digest: None,
            detail: String::from(
                "the local plugin-system note is cited explicitly as the design input for charter laws without being misrepresented as already-shipped runtime truth.",
            ),
        },
        TassadarPostArticlePluginCharterDependencyRow {
            dependency_id: String::from("plugin_system_and_turing_audit"),
            dependency_class: TassadarPostArticlePluginCharterDependencyClass::ObservationalContext,
            satisfied: true,
            source_ref: String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            bound_report_id: None,
            bound_report_digest: None,
            detail: String::from(
                "the March 20 plugin audit remains the observational context for keeping computability, programmability, and productization distinct.",
            ),
        },
        TassadarPostArticlePluginCharterDependencyRow {
            dependency_id: String::from("post_article_turing_audit"),
            dependency_class: TassadarPostArticlePluginCharterDependencyClass::ObservationalContext,
            satisfied: true,
            source_ref: String::from(POST_ARTICLE_TURING_AUDIT_REF),
            bound_report_id: None,
            bound_report_digest: None,
            detail: String::from(
                "the companion post-article Turing audit remains the observational context for keeping the capability plane above the rebased route.",
            ),
        },
        TassadarPostArticlePluginCharterDependencyRow {
            dependency_id: String::from("world_mount_compatibility"),
            dependency_class: TassadarPostArticlePluginCharterDependencyClass::CompatibilityDependency,
            satisfied: world_mount.allowed_case_count >= 1
                && world_mount.denied_case_count >= 1
                && world_mount.unresolved_case_count >= 1,
            source_ref: String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
            bound_report_id: Some(world_mount.report_id.clone()),
            bound_report_digest: Some(world_mount.report_digest.clone()),
            detail: format!(
                "capability envelopes stay explicit and task-scoped with allowed={}, denied={}, unresolved={} world-mount cases.",
                world_mount.allowed_case_count,
                world_mount.denied_case_count,
                world_mount.unresolved_case_count,
            ),
        },
        TassadarPostArticlePluginCharterDependencyRow {
            dependency_id: String::from("broad_internal_compute_profile_publication"),
            dependency_class: TassadarPostArticlePluginCharterDependencyClass::PublicationDependency,
            satisfied: internal_only_plugin_posture
                && publication.current_served_profile_id == ARTICLE_CLOSEOUT_PROFILE_ID
                && publication.published_profile_ids == vec![String::from(ARTICLE_CLOSEOUT_PROFILE_ID)],
            source_ref: String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
            bound_report_id: Some(publication.report_id.clone()),
            bound_report_digest: Some(publication.report_digest.clone()),
            detail: String::from(
                "publication posture stays internal-only on the narrower article-closeout profile instead of widening into a plugin platform claim.",
            ),
        },
        TassadarPostArticlePluginCharterDependencyRow {
            dependency_id: String::from("module_promotion_state"),
            dependency_class: TassadarPostArticlePluginCharterDependencyClass::GovernanceDependency,
            satisfied: promotion.active_promoted_count >= 1
                && promotion.challenge_open_count >= 1
                && promotion.quarantined_count >= 1
                && promotion.revoked_count >= 1
                && promotion.superseded_count >= 1,
            source_ref: String::from(MODULE_PROMOTION_STATE_REPORT_REF),
            bound_report_id: Some(promotion.report_id.clone()),
            bound_report_digest: Some(promotion.report_digest.clone()),
            detail: String::from(
                "promotion authority remains receipt-bearing and challengeable rather than implied from a green plugin charter alone.",
            ),
        },
        TassadarPostArticlePluginCharterDependencyRow {
            dependency_id: String::from("module_trust_isolation"),
            dependency_class: TassadarPostArticlePluginCharterDependencyClass::GovernanceDependency,
            satisfied: trust.allowed_case_count >= 1
                && trust.refused_case_count >= 1
                && trust.cross_tier_refusal_count >= 1
                && trust.privilege_escalation_refusal_count >= 1
                && trust.mount_policy_refusal_count >= 1,
            source_ref: String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
            bound_report_id: Some(trust.report_id.clone()),
            bound_report_digest: Some(trust.report_digest.clone()),
            detail: String::from(
                "trust-tier isolation remains explicit and refusal-backed so plugin authority cannot silently widen across tiers.",
            ),
        },
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_law_rows(
    bridge: &BridgeContractFixture,
    plugin_boundary: &TassadarPostArticlePluginCapabilityBoundaryReport,
    closeout: &PostArticleTuringCloseoutFixture,
    internal_only_plugin_posture: bool,
    proof_vs_audit_distinction_frozen: bool,
    observer_model_frozen: bool,
    three_plane_contract_frozen: bool,
    adversarial_host_model_frozen: bool,
    host_executes_but_does_not_decide: bool,
    semantic_preservation_required: bool,
    choice_set_integrity_frozen: bool,
    resource_transparency_frozen: bool,
    scheduling_ownership_frozen: bool,
    no_externalized_learning_frozen: bool,
    plugin_language_boundary_frozen: bool,
    first_plugin_tranche_closed_world: bool,
    anti_interpreter_smuggling_frozen: bool,
    downward_non_influence_frozen: bool,
) -> Vec<TassadarPostArticlePluginCharterLawRow> {
    vec![
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("plugin_non_goals_and_internal_only_posture"),
            current_posture: String::from("operator_internal_only"),
            green: internal_only_plugin_posture,
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
            ],
            detail: String::from(
                "the charter freezes plugin non-goals and keeps the current publication posture internal-only rather than broad public platform language.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("post_tas_186_owned_route_binding"),
            current_posture: String::from("inherited_from_rebased_carrier"),
            green: plugin_boundary.machine_identity_binding.machine_identity_id
                == bridge.bridge_machine_identity.machine_identity_id
                && closeout.machine_identity_binding.machine_identity_id
                    == bridge.bridge_machine_identity.machine_identity_id,
            source_refs: vec![
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            detail: String::from(
                "the plugin charter binds to the rebased owned Transformer route instead of mutating the bridge into a mixed host/controller machine.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("canonical_machine_identity_and_compute_model_inherited"),
            current_posture: String::from("digest_bound_identity_lock"),
            green: bridge.computational_model_statement.canonical_machine_identity_id
                == bridge.bridge_machine_identity.machine_identity_id
                && bridge.computational_model_statement.substrate_model_id == "tcm.v1",
            source_refs: vec![String::from(BRIDGE_CONTRACT_REPORT_REF)],
            detail: String::from(
                "the charter inherits one canonical machine identity and one computational-model statement from the rebased bridge.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("pre_plugin_control_plane_proof_is_hard_dependency"),
            current_posture: String::from("must_remain_green"),
            green: host_executes_but_does_not_decide,
            source_refs: vec![
                String::from(CONTROL_PLANE_PROOF_REPORT_REF),
                String::from(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            detail: String::from(
                "the charter does not reopen control ownership; it inherits the pre-plugin decision-provenance proof as a hard dependency.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("proof_vs_audit_distinction_and_observer_model_frozen"),
            current_posture: String::from("proof_classes_explicit"),
            green: proof_vs_audit_distinction_frozen && observer_model_frozen,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(CONTROL_PLANE_PROOF_REPORT_REF),
                String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            ],
            detail: String::from(
                "proof-bearing artifacts remain distinct from observational audits, and verifier roles stay machine-readable instead of implicit in publication language.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("three_plane_contract_frozen"),
            current_posture: String::from("data_control_capability_planes_explicit"),
            green: three_plane_contract_frozen,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(CONTROL_PLANE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
            ],
            detail: String::from(
                "data plane, control plane, and capability plane remain explicit and may not absorb each other's responsibilities.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("adversarial_host_model_frozen"),
            current_posture: String::from("host_channels_treated_as_potentially_adversarial"),
            green: adversarial_host_model_frozen,
            source_refs: vec![
                String::from(CONTROL_PLANE_PROOF_REPORT_REF),
                String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            ],
            detail: String::from(
                "scheduler, helper-selection, cache, latency, and queue signals stay outside the proof-bearing control surface unless declared explicitly.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("host_executes_capability_but_may_not_decide_workflow"),
            current_posture: String::from("execution_without_orchestration_authority"),
            green: host_executes_but_does_not_decide,
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(CONTROL_PLANE_PROOF_REPORT_REF),
            ],
            detail: String::from(
                "host may load, execute, and receipt plugins, but workflow choice remains on the weighted control path.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("semantic_preservation_rule_frozen"),
            current_posture: String::from("marshall_or_fail_closed"),
            green: semantic_preservation_required,
            source_refs: vec![
                String::from(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            detail: String::from(
                "marshalling, schema adaptation, and result reinjection must preserve declared meaning or fail closed.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("choice_set_integrity_frozen"),
            current_posture: String::from("no_hidden_filtering"),
            green: choice_set_integrity_frozen,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
            ],
            detail: String::from(
                "admissible plugin choices may not be hidden, pre-ranked, or filtered off-trace.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("resource_transparency_frozen"),
            current_posture: String::from("resource_signals_explicit_or_fixed"),
            green: resource_transparency_frozen,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
            ],
            detail: String::from(
                "latency, quota, cost, and availability that could steer branching stay explicit or fixed by contract.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("scheduling_ownership_frozen"),
            current_posture: String::from("ordering_and_visibility_not_host_steered"),
            green: scheduling_ownership_frozen,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
            ],
            detail: String::from(
                "ordering, concurrency, and result-visibility timing remain model-owned or fixed as a declared runtime contract.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("no_externalized_learning_rule_frozen"),
            current_posture: String::from("runtime_learning_forbidden"),
            green: no_externalized_learning_frozen,
            source_refs: vec![
                String::from(CONTROL_PLANE_PROOF_REPORT_REF),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            detail: String::from(
                "runtime adaptation, telemetry, logging, and cache behavior may not become hidden learning channels across executions.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("plugin_language_boundary_is_bounded_effectful_procedure"),
            current_posture: String::from("not_a_second_compute_substrate"),
            green: plugin_language_boundary_frozen,
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
                String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
            ],
            detail: String::from(
                "the charter freezes the plugin language boundary as bounded effectful procedures above the machine, not a hidden second compute substrate.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("first_plugin_tranche_closed_world_and_operator_curated"),
            current_posture: String::from("closed_world_until_later_discovery_audit"),
            green: first_plugin_tranche_closed_world,
            source_refs: vec![
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            detail: String::from(
                "the first plugin tranche remains closed-world and operator-curated until later manifest/runtime/discovery work is audited separately.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("anti_interpreter_smuggling_frozen"),
            current_posture: String::from("plugin_lane_cannot_backfill_interpreter_ownership"),
            green: anti_interpreter_smuggling_frozen,
            source_refs: vec![
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
                String::from(CONTROL_PLANE_PROOF_REPORT_REF),
                String::from(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            detail: String::from(
                "plugin artifacts cannot smuggle a fresh interpreter/control substrate underneath the already-closed canonical route.",
            ),
        },
        TassadarPostArticlePluginCharterLawRow {
            law_id: String::from("downward_non_influence_frozen"),
            current_posture: String::from("plugin_ergonomics_cannot_rewrite_lower_planes"),
            green: downward_non_influence_frozen,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
            ],
            detail: String::from(
                "plugin ergonomics may not redefine continuation semantics, proof assumptions, or carrier identity below the capability plane.",
            ),
        },
    ]
}

fn build_state_class_rows(
    control_plane: &ControlPlaneProofFixture,
    plugin_boundary: &TassadarPostArticlePluginCapabilityBoundaryReport,
    world_mount: &WorldMountCompatibilityFixture,
) -> Vec<TassadarPostArticlePluginStateClassRow> {
    let packet_local_green = plugin_boundary.state_receipt_rows.iter().any(|row| {
        row.row_id == "plugin_packet_local_state"
            && row.green
            && row.current_posture == "reserved_not_implemented"
    });
    let instance_local_green = plugin_boundary.state_receipt_rows.iter().any(|row| {
        row.row_id == "plugin_instance_local_ephemeral_state"
            && row.green
            && row.current_posture == "reserved_not_implemented"
    });
    let host_backed_green = control_plane
        .hidden_state_channel_closure
        .allowed_state_class_ids
        .iter()
        .any(|id| id == "durable_receipt_backed_state")
        && control_plane.observer_model.replay_receipt_required
        && world_mount
            .world_mount_dependency_marker
            .contains("outside standalone psionic");
    let weights_owned_green = control_plane
        .hidden_state_channel_closure
        .allowed_state_class_ids
        .iter()
        .any(|id| id == "weights_owned_control_state")
        && control_plane.decision_provenance_proof_complete;

    vec![
        TassadarPostArticlePluginStateClassRow {
            state_class_id: String::from("packet_local_state"),
            owner_plane: String::from("capability"),
            current_posture: String::from("reserved_packet_local_only"),
            green: packet_local_green,
            source_refs: vec![
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            detail: String::from(
                "packet-local plugin state stays a bounded capability-plane class instead of leaking into compute or control truth.",
            ),
        },
        TassadarPostArticlePluginStateClassRow {
            state_class_id: String::from("instance_local_ephemeral_state"),
            owner_plane: String::from("capability"),
            current_posture: String::from("reserved_ephemeral_only"),
            green: instance_local_green,
            source_refs: vec![
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            detail: String::from(
                "instance-local plugin state stays explicitly ephemeral and may not become undeclared workflow state.",
            ),
        },
        TassadarPostArticlePluginStateClassRow {
            state_class_id: String::from("host_backed_durable_state"),
            owner_plane: String::from("capability"),
            current_posture: String::from("receipt_backed_only"),
            green: host_backed_green,
            source_refs: vec![
                String::from(CONTROL_PLANE_PROOF_REPORT_REF),
                String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
            ],
            detail: String::from(
                "durable host-backed state must be receipt-backed and external-authority-scoped rather than silently owning workflow semantics.",
            ),
        },
        TassadarPostArticlePluginStateClassRow {
            state_class_id: String::from("weights_owned_control_state"),
            owner_plane: String::from("control"),
            current_posture: String::from("canonical_workflow_owner"),
            green: weights_owned_green,
            source_refs: vec![
                String::from(CONTROL_PLANE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
            ],
            detail: String::from(
                "branching, retries, stop conditions, and composition remain weights-owned control state on the canonical route.",
            ),
        },
    ]
}

fn build_governance_rows(
    plugin_boundary: &TassadarPostArticlePluginCapabilityBoundaryReport,
    publication: &BroadInternalComputeProfilePublicationFixture,
    promotion: &ModulePromotionStateFixture,
    trust: &ModuleTrustIsolationFixture,
    world_mount: &WorldMountCompatibilityFixture,
    internal_only_plugin_posture: bool,
) -> Vec<TassadarPostArticlePluginGovernanceRow> {
    vec![
        TassadarPostArticlePluginGovernanceRow {
            governance_id: String::from("canonical_plugin_declaration_authority"),
            current_posture: String::from("operator_curated_internal_only_pending_later_receipts"),
            green: plugin_boundary.closed_world_operator_curated_first_tranche_required
                && internal_only_plugin_posture
                && publication.current_served_profile_id == ARTICLE_CLOSEOUT_PROFILE_ID,
            source_refs: vec![
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
            ],
            detail: String::from(
                "standalone psionic may stage bounded operator-curated plugins, but this charter does not grant a public canonical plugin declaration right.",
            ),
        },
        TassadarPostArticlePluginGovernanceRow {
            governance_id: String::from("capability_envelope_widening_authority"),
            current_posture: String::from("requires_world_mount_trust_and_promotion_receipts"),
            green: world_mount.unresolved_case_count >= 1
                && trust.mount_policy_refusal_count >= 1
                && promotion.challenge_open_count >= 1,
            source_refs: vec![
                String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
            ],
            detail: String::from(
                "widening envelopes requires mount, trust, and promotion receipts instead of a silent policy toggle inside the host runtime.",
            ),
        },
        TassadarPostArticlePluginGovernanceRow {
            governance_id: String::from("publication_and_trust_tier_widening_authority"),
            current_posture: String::from("suppressed_until_publication_and_promotion_gates_green"),
            green: !publication.suppressed_profile_ids.is_empty()
                && promotion.quarantined_count >= 1
                && trust.privilege_escalation_refusal_count >= 1,
            source_refs: vec![
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
            ],
            detail: String::from(
                "publication and trust-tier widening remain suppressed until later promotion, publication, and trust gates carry their own receipts.",
            ),
        },
        TassadarPostArticlePluginGovernanceRow {
            governance_id: String::from("typed_posture_change_receipts_required"),
            current_posture: String::from("refusal_suppression_quarantine_downgrade_blocked"),
            green: promotion.quarantined_count >= 1
                && trust.refused_case_count >= 1
                && !publication.suppressed_profile_ids.is_empty(),
            source_refs: vec![
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
                String::from(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            detail: String::from(
                "posture changes must stay typed through refusal, suppression, quarantine, downgrade, or blocked receipts when prerequisites are missing.",
            ),
        },
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    bridge: &BridgeContractFixture,
    control_plane: &ControlPlaneProofFixture,
    plugin_boundary: &TassadarPostArticlePluginCapabilityBoundaryReport,
    closeout: &PostArticleTuringCloseoutFixture,
    world_mount: &WorldMountCompatibilityFixture,
    publication: &BroadInternalComputeProfilePublicationFixture,
    promotion: &ModulePromotionStateFixture,
    trust: &ModuleTrustIsolationFixture,
    no_externalized_learning_frozen: bool,
    resource_transparency_frozen: bool,
    scheduling_ownership_frozen: bool,
    host_executes_but_does_not_decide: bool,
    semantic_preservation_required: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
) -> Vec<TassadarPostArticlePluginCharterValidationRow> {
    vec![
        TassadarPostArticlePluginCharterValidationRow {
            validation_id: String::from("canonical_identity_digest_binding_green"),
            green: plugin_boundary.machine_identity_binding.machine_identity_id
                == bridge.bridge_machine_identity.machine_identity_id
                && control_plane.machine_identity_id == bridge.bridge_machine_identity.machine_identity_id
                && closeout.machine_identity_binding.machine_identity_id
                    == bridge.bridge_machine_identity.machine_identity_id
                && plugin_boundary.machine_identity_binding.canonical_route_descriptor_digest
                    == bridge.bridge_machine_identity.canonical_route_descriptor_digest,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(CONTROL_PLANE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
                String::from(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            detail: String::from(
                "the plugin charter remains digest-bound to the same bridge machine identity, model, and canonical route as the rebased truth carrier.",
            ),
        },
        TassadarPostArticlePluginCharterValidationRow {
            validation_id: String::from("computational_model_statement_inherited_without_tcm_mutation"),
            green: bridge.computational_model_statement.substrate_model_id == "tcm.v1"
                && bridge.computational_model_statement.runtime_contract_id
                    == bridge.bridge_machine_identity.continuation_contract_id
                && plugin_boundary.tcm_v1_substrate_retained,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
            ],
            detail: String::from(
                "the charter inherits the declared computational model and continuation contract without widening the bounded substrate.",
            ),
        },
        TassadarPostArticlePluginCharterValidationRow {
            validation_id: String::from("hidden_host_orchestration_blocked"),
            green: control_plane.hidden_control_channel_rows.len() >= 6
                && host_executes_but_does_not_decide,
            source_refs: vec![String::from(CONTROL_PLANE_PROOF_REPORT_REF)],
            detail: String::from(
                "hidden host orchestration remains blocked by explicit hidden-control-channel review plus the host-executes-only rule.",
            ),
        },
        TassadarPostArticlePluginCharterValidationRow {
            validation_id: String::from("schema_drift_posture_blocked"),
            green: semantic_preservation_required
                && plugin_boundary.plugin_execution_layer_separate
                && plugin_boundary.plugin_state_identity_separated,
            source_refs: vec![
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
                String::from(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            detail: String::from(
                "schema and marshalling drift remain fail-closed rather than becoming a silent control-layer widening.",
            ),
        },
        TassadarPostArticlePluginCharterValidationRow {
            validation_id: String::from("envelope_leakage_posture_blocked"),
            green: world_mount.allowed_case_count >= 1
                && world_mount.denied_case_count >= 1
                && world_mount.unresolved_case_count >= 1
                && world_mount
                    .world_mount_dependency_marker
                    .contains("outside standalone psionic"),
            source_refs: vec![String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF)],
            detail: String::from(
                "capability envelopes remain explicit and externally owned where necessary instead of leaking into ambient host privilege.",
            ),
        },
        TassadarPostArticlePluginCharterValidationRow {
            validation_id: String::from("side_channel_posture_blocked"),
            green: resource_transparency_frozen
                && scheduling_ownership_frozen
                && control_plane
                    .information_boundary
                    .model_hidden_signal_ids
                    .iter()
                    .any(|signal| signal == "latency")
                && control_plane
                    .information_boundary
                    .model_hidden_signal_ids
                    .iter()
                    .any(|signal| signal == "queue_pressure")
                && control_plane
                    .information_boundary
                    .model_hidden_signal_ids
                    .iter()
                    .any(|signal| signal == "helper_selection"),
            source_refs: vec![
                String::from(CONTROL_PLANE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
            ],
            detail: String::from(
                "latency, queue pressure, cache-hit, scheduler order, and helper-selection signals remain outside model-visible control unless explicitly declared.",
            ),
        },
        TassadarPostArticlePluginCharterValidationRow {
            validation_id: String::from("overclaim_posture_blocked"),
            green: rebase_claim_allowed
                && !plugin_capability_claim_allowed
                && !weighted_plugin_control_allowed
                && !plugin_publication_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            source_refs: vec![
                String::from(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF),
            ],
            detail: String::from(
                "the green rebased claim does not widen into weighted plugin control, plugin publication, served/public universality, or arbitrary software capability.",
            ),
        },
        TassadarPostArticlePluginCharterValidationRow {
            validation_id: String::from("typed_fail_closed_posture_explicit"),
            green: !publication.suppressed_profile_ids.is_empty()
                && promotion.quarantined_count >= 1
                && trust.refused_case_count >= 1
                && !closeout.plugin_capability_claim_allowed,
            source_refs: vec![
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
                String::from(POST_ARTICLE_TURING_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            detail: String::from(
                "suppression, quarantine, refusal, downgrade, and blocked posture remain explicit whenever plugin prerequisites are missing.",
            ),
        },
        TassadarPostArticlePluginCharterValidationRow {
            validation_id: String::from("external_authority_dependencies_explicit"),
            green: world_mount
                .kernel_policy_dependency_marker
                .contains("outside standalone psionic")
                && publication
                    .compute_market_dependency_marker
                    .contains("outside standalone psionic")
                && promotion
                    .kernel_policy_dependency_marker
                    .contains("outside standalone psionic")
                && promotion
                    .nexus_dependency_marker
                    .contains("outside standalone psionic")
                && trust
                    .cluster_trust_dependency_marker
                    .contains("outside standalone psionic"),
            source_refs: vec![
                String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
            ],
            detail: String::from(
                "world-mount, kernel-policy, cluster-trust, compute-market, and nexus ownership stays explicit instead of being backfilled into standalone psionic.",
            ),
        },
        TassadarPostArticlePluginCharterValidationRow {
            validation_id: String::from("no_externalized_learning_rule_enforced"),
            green: no_externalized_learning_frozen
                && control_plane.hidden_state_channel_closure.hidden_state_channel_closed,
            source_refs: vec![String::from(CONTROL_PLANE_PROOF_REPORT_REF)],
            detail: String::from(
                "runtime adaptation, telemetry, logs, and cache behavior remain outside decision authority, and hidden state channels stay closed.",
            ),
        },
    ]
}

#[must_use]
pub fn tassadar_post_article_plugin_charter_authority_boundary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF)
}

pub fn write_tassadar_post_article_plugin_charter_authority_boundary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginCharterAuthorityBoundaryReport,
    TassadarPostArticlePluginCharterAuthorityBoundaryReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginCharterAuthorityBoundaryReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_plugin_charter_authority_boundary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginCharterAuthorityBoundaryReportError::Write {
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
        .expect("repo root should resolve from psionic-sandbox crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarPostArticlePluginCharterAuthorityBoundaryReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginCharterAuthorityBoundaryReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginCharterAuthorityBoundaryReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BridgeContractFixture {
    report_id: String,
    report_digest: String,
    bridge_machine_identity: BridgeMachineIdentityFixture,
    computational_model_statement: ComputationalModelStatementFixture,
    reserved_later_invariant_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BridgeMachineIdentityFixture {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ComputationalModelStatementFixture {
    statement_id: String,
    #[serde(alias = "machine_identity_id")]
    canonical_machine_identity_id: String,
    substrate_model_id: String,
    substrate_model_digest: String,
    runtime_contract_id: String,
    runtime_contract_digest: String,
    proof_class_statement: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ControlPlaneProofFixture {
    report_id: String,
    report_digest: String,
    machine_identity_id: String,
    control_plane_ownership_green: bool,
    decision_provenance_proof_complete: bool,
    hidden_control_channel_rows: Vec<serde_json::Value>,
    observer_model: ControlPlaneObserverModelFixture,
    information_boundary: ControlPlaneInformationBoundaryFixture,
    training_inference_boundary: ControlPlaneTrainingInferenceBoundaryFixture,
    hidden_state_channel_closure: ControlPlaneHiddenStateChannelClosureFixture,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ControlPlaneObserverModelFixture {
    replay_receipt_required: bool,
    gate_verdict_bound_to_machine_identity: bool,
    green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ControlPlaneInformationBoundaryFixture {
    model_hidden_signal_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ControlPlaneTrainingInferenceBoundaryFixture {
    runtime_adaptation_allowed: bool,
    telemetry_decision_authority: bool,
    logging_decision_authority: bool,
    cache_behavior_decision_authority: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ControlPlaneHiddenStateChannelClosureFixture {
    allowed_state_class_ids: Vec<String>,
    hidden_state_channel_closed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct PostArticleTuringCloseoutFixture {
    report_id: String,
    report_digest: String,
    machine_identity_binding: PostArticleMachineIdentityBindingFixture,
    closeout_green: bool,
    historical_tas_156_still_stands: bool,
    canonical_route_truth_carrier: bool,
    control_plane_proof_part_of_truth_carrier: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct PostArticleMachineIdentityBindingFixture {
    machine_identity_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct WorldMountCompatibilityFixture {
    report_id: String,
    report_digest: String,
    allowed_case_count: u32,
    denied_case_count: u32,
    unresolved_case_count: u32,
    world_mount_dependency_marker: String,
    kernel_policy_dependency_marker: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BroadInternalComputeProfilePublicationFixture {
    report_id: String,
    report_digest: String,
    current_served_profile_id: String,
    published_profile_ids: Vec<String>,
    suppressed_profile_ids: Vec<String>,
    compute_market_dependency_marker: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ModulePromotionStateFixture {
    report_id: String,
    report_digest: String,
    active_promoted_count: u32,
    challenge_open_count: u32,
    quarantined_count: u32,
    revoked_count: u32,
    superseded_count: u32,
    kernel_policy_dependency_marker: String,
    nexus_dependency_marker: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ModuleTrustIsolationFixture {
    report_id: String,
    report_digest: String,
    allowed_case_count: u32,
    refused_case_count: u32,
    cross_tier_refusal_count: u32,
    privilege_escalation_refusal_count: u32,
    mount_policy_refusal_count: u32,
    cluster_trust_dependency_marker: String,
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticlePluginCharterAuthorityBoundaryReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticlePluginCharterAuthorityBoundaryReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginCharterAuthorityBoundaryReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_plugin_charter_authority_boundary_report, read_json,
        tassadar_post_article_plugin_charter_authority_boundary_report_path,
        write_tassadar_post_article_plugin_charter_authority_boundary_report,
        TassadarPostArticlePluginCharterAuthorityBoundaryReport,
        TassadarPostArticlePluginCharterAuthorityBoundaryStatus,
        TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn post_article_plugin_charter_freezes_laws_without_widening_claims() {
        let report =
            build_tassadar_post_article_plugin_charter_authority_boundary_report().expect("report");

        assert_eq!(
            report.charter_status,
            TassadarPostArticlePluginCharterAuthorityBoundaryStatus::Green
        );
        assert!(report.charter_green);
        assert!(report.internal_only_plugin_posture);
        assert!(report.proof_vs_audit_distinction_frozen);
        assert!(report.observer_model_frozen);
        assert!(report.three_plane_contract_frozen);
        assert!(report.adversarial_host_model_frozen);
        assert!(report.state_class_split_frozen);
        assert!(report.host_executes_but_does_not_decide);
        assert!(report.semantic_preservation_required);
        assert!(report.choice_set_integrity_frozen);
        assert!(report.resource_transparency_frozen);
        assert!(report.scheduling_ownership_frozen);
        assert!(report.no_externalized_learning_frozen);
        assert!(report.plugin_language_boundary_frozen);
        assert!(report.first_plugin_tranche_closed_world);
        assert!(report.anti_interpreter_smuggling_frozen);
        assert!(report.downward_non_influence_frozen);
        assert!(report.governance_receipts_required);
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
        assert_eq!(report.dependency_rows.len(), 11);
        assert_eq!(report.law_rows.len(), 17);
        assert_eq!(report.state_class_rows.len(), 4);
        assert_eq!(report.governance_rows.len(), 4);
        assert_eq!(report.validation_rows.len(), 10);
        assert!(report.deferred_issue_ids.is_empty());
    }

    #[test]
    fn post_article_plugin_charter_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_charter_authority_boundary_report().expect("report");
        let committed: TassadarPostArticlePluginCharterAuthorityBoundaryReport =
            read_json(tassadar_post_article_plugin_charter_authority_boundary_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_plugin_charter_authority_boundary_report.json"
        );
    }

    #[test]
    fn write_post_article_plugin_charter_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_plugin_charter_authority_boundary_report.json");
        let written =
            write_tassadar_post_article_plugin_charter_authority_boundary_report(&output_path)
                .expect("write report");
        let persisted: TassadarPostArticlePluginCharterAuthorityBoundaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
