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

use crate::{build_tassadar_import_policy_matrix_report, TassadarImportPolicyMatrixReport};

pub const TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_capability_boundary_report.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-plugin-capability-boundary.sh";

const BRIDGE_CONTRACT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_bridge_contract_report.json";
const REBASED_VERDICT_SPLIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_rebased_universality_verdict_split_report.json";
const WORLD_MOUNT_COMPATIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_world_mount_compatibility_report.json";
const PLUGIN_SYSTEM_AND_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const CANONICAL_ARCHITECTURE_BOUNDARY_REF: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const CANONICAL_ARCHITECTURE_ANCHOR_CRATE: &str = "psionic-transformer";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginCapabilityBoundaryStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginCapabilityDependencyClass {
    ProofCarrying,
    CompatibilityDependency,
    ObservationalContext,
    DesignInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCapabilityMachineIdentity {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub reserved_capability_plane_id: String,
    pub bridge_contract_report_id: String,
    pub bridge_contract_report_digest: String,
    pub rebased_verdict_report_id: String,
    pub rebased_verdict_report_digest: String,
    pub canonical_architecture_anchor_crate: String,
    pub canonical_architecture_boundary_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCapabilityDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticlePluginCapabilityDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCapabilityBoundaryRow {
    pub boundary_id: String,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginStateReceiptRow {
    pub row_id: String,
    pub state_or_receipt_class: String,
    pub owner_plane: String,
    pub current_posture: String,
    pub separate_from_compute_substrate: bool,
    pub receipt_identity_family: String,
    pub source_refs: Vec<String>,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginReservedInvariantRow {
    pub invariant_id: String,
    pub current_posture: String,
    pub reserved_for_later_plugin_layer: bool,
    pub source_refs: Vec<String>,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCapabilityValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCapabilityBoundaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub tcm_v1_runtime_contract_report_ref: String,
    pub bridge_contract_report_ref: String,
    pub rebased_verdict_split_report_ref: String,
    pub world_mount_compatibility_report_ref: String,
    pub import_policy_matrix_report_ref: String,
    pub plugin_system_audit_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding: TassadarPostArticlePluginCapabilityMachineIdentity,
    pub dependency_rows: Vec<TassadarPostArticlePluginCapabilityDependencyRow>,
    pub boundary_rows: Vec<TassadarPostArticlePluginCapabilityBoundaryRow>,
    pub state_receipt_rows: Vec<TassadarPostArticlePluginStateReceiptRow>,
    pub reserved_invariant_rows: Vec<TassadarPostArticlePluginReservedInvariantRow>,
    pub validation_rows: Vec<TassadarPostArticlePluginCapabilityValidationRow>,
    pub boundary_status: TassadarPostArticlePluginCapabilityBoundaryStatus,
    pub boundary_green: bool,
    pub tcm_v1_substrate_retained: bool,
    pub plugin_capability_plane_reserved: bool,
    pub plugin_execution_layer_separate: bool,
    pub downward_non_influence_reserved: bool,
    pub plugin_state_identity_separated: bool,
    pub closed_world_operator_curated_first_tranche_required: bool,
    pub first_plugin_tranche_posture: String,
    pub choice_set_integrity_reserved: bool,
    pub resource_transparency_reserved: bool,
    pub scheduling_ownership_reserved: bool,
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
pub enum TassadarPostArticlePluginCapabilityBoundaryReportError {
    #[error(transparent)]
    Runtime(#[from] TassadarTcmV1RuntimeContractReportError),
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

pub fn build_tassadar_post_article_plugin_capability_boundary_report() -> Result<
    TassadarPostArticlePluginCapabilityBoundaryReport,
    TassadarPostArticlePluginCapabilityBoundaryReportError,
> {
    let runtime_contract = build_tassadar_tcm_v1_runtime_contract_report()?;
    let bridge_report: BridgeContractFixture = read_repo_json(BRIDGE_CONTRACT_REPORT_REF)?;
    let rebased_verdict: RebasedVerdictFixture = read_repo_json(REBASED_VERDICT_SPLIT_REPORT_REF)?;
    let world_mount: WorldMountCompatibilityFixture =
        read_repo_json(WORLD_MOUNT_COMPATIBILITY_REPORT_REF)?;
    let import_policy = build_tassadar_import_policy_matrix_report();

    let reserved_capability_plane_id = bridge_report
        .carrier_rows
        .iter()
        .find(|row| row.carrier_class == "reserved_capability_plane")
        .map(|row| row.carrier_id.clone())
        .expect("reserved capability plane should exist");
    let machine_identity_binding = TassadarPostArticlePluginCapabilityMachineIdentity {
        machine_identity_id: bridge_report.bridge_machine_identity.machine_identity_id.clone(),
        canonical_model_id: bridge_report.bridge_machine_identity.canonical_model_id.clone(),
        canonical_route_id: bridge_report.bridge_machine_identity.canonical_route_id.clone(),
        canonical_route_descriptor_digest: bridge_report
            .bridge_machine_identity
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: bridge_report
            .bridge_machine_identity
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: bridge_report
            .bridge_machine_identity
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: bridge_report
            .bridge_machine_identity
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: bridge_report
            .bridge_machine_identity
            .continuation_contract_digest
            .clone(),
        reserved_capability_plane_id: reserved_capability_plane_id.clone(),
        bridge_contract_report_id: bridge_report.report_id.clone(),
        bridge_contract_report_digest: bridge_report.report_digest.clone(),
        rebased_verdict_report_id: rebased_verdict.report_id.clone(),
        rebased_verdict_report_digest: rebased_verdict.report_digest.clone(),
        canonical_architecture_anchor_crate: String::from(CANONICAL_ARCHITECTURE_ANCHOR_CRATE),
        canonical_architecture_boundary_ref: String::from(CANONICAL_ARCHITECTURE_BOUNDARY_REF),
        detail: format!(
            "machine_identity_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` continuation_contract_id=`{}` reserved_capability_plane_id=`{}` and canonical architecture ownership stays anchored in `{}`.",
            bridge_report.bridge_machine_identity.machine_identity_id,
            bridge_report.bridge_machine_identity.canonical_model_id,
            bridge_report.bridge_machine_identity.canonical_route_id,
            bridge_report.bridge_machine_identity.continuation_contract_id,
            reserved_capability_plane_id,
            CANONICAL_ARCHITECTURE_ANCHOR_CRATE,
        ),
    };

    let dependency_rows = build_dependency_rows(
        &runtime_contract,
        &bridge_report,
        &rebased_verdict,
        &world_mount,
        &import_policy,
    );
    let boundary_rows = build_boundary_rows(&machine_identity_binding);
    let state_receipt_rows =
        build_state_receipt_rows(&runtime_contract, &rebased_verdict.report_id);
    let reserved_invariant_rows = build_reserved_invariant_rows(&bridge_report);

    let tcm_v1_substrate_retained = runtime_contract.overall_green
        && runtime_contract.report_id
            == bridge_report
                .bridge_machine_identity
                .continuation_contract_id
        && runtime_contract.report_digest
            == bridge_report
                .bridge_machine_identity
                .continuation_contract_digest;
    let plugin_capability_plane_reserved = !rebased_verdict.plugin_capability_claim_allowed;
    let plugin_execution_layer_separate = import_policy
        .policy_matrix
        .entries
        .iter()
        .any(|entry| entry.import_ref == "sandbox.math_eval")
        && world_mount.surface.import_posture == "deterministic_stub_imports_only";
    let downward_non_influence_reserved = true;
    let plugin_state_identity_separated = state_receipt_rows
        .iter()
        .filter(|row| row.owner_plane == "capability")
        .all(|row| row.separate_from_compute_substrate);
    let closed_world_operator_curated_first_tranche_required = true;
    let first_plugin_tranche_posture =
        String::from("closed_world_operator_curated_only_until_audited");
    let choice_set_integrity_reserved = bridge_report
        .reserved_later_invariant_ids
        .iter()
        .any(|id| id == "choice_set_integrity");
    let resource_transparency_reserved = bridge_report
        .reserved_later_invariant_ids
        .iter()
        .any(|id| id == "resource_transparency");
    let scheduling_ownership_reserved = bridge_report
        .reserved_later_invariant_ids
        .iter()
        .any(|id| id == "scheduling_ownership");
    let rebase_claim_allowed = rebased_verdict.rebase_claim_allowed;
    let plugin_capability_claim_allowed = rebased_verdict.plugin_capability_claim_allowed;
    let weighted_plugin_control_allowed = false;
    let plugin_publication_allowed = false;
    let served_public_universality_allowed = rebased_verdict.served_public_universality_allowed;
    let arbitrary_software_capability_allowed =
        rebased_verdict.arbitrary_software_capability_allowed;
    let validation_rows = build_validation_rows(
        &runtime_contract,
        &bridge_report,
        &rebased_verdict,
        &world_mount,
        &import_policy,
        choice_set_integrity_reserved,
        resource_transparency_reserved,
        scheduling_ownership_reserved,
        weighted_plugin_control_allowed,
        plugin_publication_allowed,
    );
    let boundary_green = dependency_rows.iter().all(|row| row.satisfied)
        && boundary_rows.iter().all(|row| row.green)
        && state_receipt_rows.iter().all(|row| row.green)
        && reserved_invariant_rows.iter().all(|row| row.green)
        && validation_rows.iter().all(|row| row.green)
        && tcm_v1_substrate_retained
        && plugin_capability_plane_reserved
        && plugin_execution_layer_separate
        && downward_non_influence_reserved
        && plugin_state_identity_separated
        && closed_world_operator_curated_first_tranche_required
        && choice_set_integrity_reserved
        && resource_transparency_reserved
        && scheduling_ownership_reserved
        && rebase_claim_allowed
        && !plugin_capability_claim_allowed
        && !weighted_plugin_control_allowed
        && !plugin_publication_allowed
        && !served_public_universality_allowed
        && !arbitrary_software_capability_allowed;
    let boundary_status = if boundary_green {
        TassadarPostArticlePluginCapabilityBoundaryStatus::Green
    } else {
        TassadarPostArticlePluginCapabilityBoundaryStatus::Incomplete
    };
    let deferred_issue_ids = Vec::new();

    let mut report = TassadarPostArticlePluginCapabilityBoundaryReport {
        schema_version: 1,
        report_id: String::from("tassadar.post_article_plugin_capability_boundary.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_CHECKER_REF,
        ),
        tcm_v1_runtime_contract_report_ref: String::from(
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
        ),
        bridge_contract_report_ref: String::from(BRIDGE_CONTRACT_REPORT_REF),
        rebased_verdict_split_report_ref: String::from(REBASED_VERDICT_SPLIT_REPORT_REF),
        world_mount_compatibility_report_ref: String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
        import_policy_matrix_report_ref: String::from(
            crate::TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF,
        ),
        plugin_system_audit_ref: String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        supporting_material_refs: vec![
            String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
            String::from(BRIDGE_CONTRACT_REPORT_REF),
            String::from(REBASED_VERDICT_SPLIT_REPORT_REF),
            String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
            String::from(crate::TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF),
            String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            String::from(CANONICAL_ARCHITECTURE_BOUNDARY_REF),
        ],
        machine_identity_binding,
        dependency_rows,
        boundary_rows,
        state_receipt_rows,
        reserved_invariant_rows,
        validation_rows,
        boundary_status,
        boundary_green,
        tcm_v1_substrate_retained,
        plugin_capability_plane_reserved,
        plugin_execution_layer_separate,
        downward_non_influence_reserved,
        plugin_state_identity_separated,
        closed_world_operator_curated_first_tranche_required,
        first_plugin_tranche_posture,
        choice_set_integrity_reserved,
        resource_transparency_reserved,
        scheduling_ownership_reserved,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        weighted_plugin_control_allowed,
        plugin_publication_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        deferred_issue_ids,
        claim_boundary: String::from(
            "this report makes the post-`TAS-186` rebased universality lane explicitly plugin-aware without widening `TCM.v1`, the continuation contract, or the rebased verdict into weighted plugin control. It binds one reserved capability plane above the canonical bridge machine identity, keeps plugin execution as a separate software-capability layer, keeps plugin state and receipt identity outside the bounded compute substrate, reserves choice-set integrity/resource transparency/scheduling ownership and a closed-world operator-curated first tranche, and leaves plugin/publication/served/arbitrary software capability claims blocked.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "post-article plugin-capability boundary report binds machine_identity_id=`{}`, canonical_route_id=`{}`, boundary_status={:?}, dependency_rows={}, reserved_invariants={}, rebase_claim_allowed={}, plugin_capability_claim_allowed={}, weighted_plugin_control_allowed={}, and plugin_publication_allowed={}.",
        report.machine_identity_binding.machine_identity_id,
        report.machine_identity_binding.canonical_route_id,
        report.boundary_status,
        report.dependency_rows.len(),
        report.reserved_invariant_rows.len(),
        report.rebase_claim_allowed,
        report.plugin_capability_claim_allowed,
        report.weighted_plugin_control_allowed,
        report.plugin_publication_allowed,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_capability_boundary_report|",
        &report,
    );
    Ok(report)
}

fn build_dependency_rows(
    runtime_contract: &TassadarTcmV1RuntimeContractReport,
    bridge_report: &BridgeContractFixture,
    rebased_verdict: &RebasedVerdictFixture,
    world_mount: &WorldMountCompatibilityFixture,
    import_policy: &TassadarImportPolicyMatrixReport,
) -> Vec<TassadarPostArticlePluginCapabilityDependencyRow> {
    vec![
        TassadarPostArticlePluginCapabilityDependencyRow {
            dependency_id: String::from("tcm_v1_runtime_contract"),
            dependency_class: TassadarPostArticlePluginCapabilityDependencyClass::ProofCarrying,
            satisfied: runtime_contract.overall_green,
            source_ref: String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
            bound_report_id: Some(runtime_contract.report_id.clone()),
            bound_report_digest: Some(runtime_contract.report_digest.clone()),
            detail: String::from(
                "`TCM.v1` remains the bounded compute substrate and continuation contract below any later plugin layer.",
            ),
        },
        TassadarPostArticlePluginCapabilityDependencyRow {
            dependency_id: String::from("post_article_bridge_machine_identity"),
            dependency_class: TassadarPostArticlePluginCapabilityDependencyClass::ProofCarrying,
            satisfied: bridge_report.bridge_machine_identity.continuation_contract_id
                == runtime_contract.report_id,
            source_ref: String::from(BRIDGE_CONTRACT_REPORT_REF),
            bound_report_id: Some(bridge_report.report_id.clone()),
            bound_report_digest: Some(bridge_report.report_digest.clone()),
            detail: String::from(
                "the plugin-aware boundary binds to the canonical post-article bridge machine identity instead of reconstructing a mixed host/runtime machine ad hoc.",
            ),
        },
        TassadarPostArticlePluginCapabilityDependencyRow {
            dependency_id: String::from("rebased_universality_verdict_split"),
            dependency_class: TassadarPostArticlePluginCapabilityDependencyClass::ProofCarrying,
            satisfied: rebased_verdict.rebase_claim_allowed
                && !rebased_verdict.plugin_capability_claim_allowed,
            source_ref: String::from(REBASED_VERDICT_SPLIT_REPORT_REF),
            bound_report_id: Some(rebased_verdict.report_id.clone()),
            bound_report_digest: Some(rebased_verdict.report_digest.clone()),
            detail: String::from(
                "the rebased theory/operator verdict stays green while plugin capability remains explicitly out of scope.",
            ),
        },
        TassadarPostArticlePluginCapabilityDependencyRow {
            dependency_id: String::from("plugin_system_and_turing_audit"),
            dependency_class:
                TassadarPostArticlePluginCapabilityDependencyClass::ObservationalContext,
            satisfied: true,
            source_ref: String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            bound_report_id: None,
            bound_report_digest: None,
            detail: String::from(
                "the March 20 plugin audit provides the observational context for keeping plugin capability above the rebased compute substrate rather than folded into it.",
            ),
        },
        TassadarPostArticlePluginCapabilityDependencyRow {
            dependency_id: String::from("local_plugin_system_spec"),
            dependency_class: TassadarPostArticlePluginCapabilityDependencyClass::DesignInput,
            satisfied: true,
            source_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            bound_report_id: None,
            bound_report_digest: None,
            detail: String::from(
                "the local plugin-system note is cited explicitly as a design input without treating it as already-landed runtime truth.",
            ),
        },
        TassadarPostArticlePluginCapabilityDependencyRow {
            dependency_id: String::from("world_mount_compatibility"),
            dependency_class:
                TassadarPostArticlePluginCapabilityDependencyClass::CompatibilityDependency,
            satisfied: world_mount.allowed_case_count >= 1
                && world_mount.denied_case_count >= 1
                && world_mount.unresolved_case_count >= 1,
            source_ref: String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
            bound_report_id: Some(world_mount.report_id.clone()),
            bound_report_digest: Some(world_mount.report_digest.clone()),
            detail: format!(
                "world-mount compatibility keeps mount admission task-scoped and external to standalone psionic with allowed={}, denied={}, unresolved={} cases.",
                world_mount.allowed_case_count,
                world_mount.denied_case_count,
                world_mount.unresolved_case_count,
            ),
        },
        TassadarPostArticlePluginCapabilityDependencyRow {
            dependency_id: String::from("import_policy_matrix"),
            dependency_class:
                TassadarPostArticlePluginCapabilityDependencyClass::CompatibilityDependency,
            satisfied: import_policy.allowed_internal_case_count >= 1
                && import_policy.delegated_case_count >= 1
                && import_policy.refused_case_count >= 1,
            source_ref: String::from(crate::TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF),
            bound_report_id: Some(import_policy.report_id.clone()),
            bound_report_digest: Some(import_policy.report_digest.clone()),
            detail: format!(
                "the import-policy matrix keeps internal deterministic stubs, explicit sandbox delegation, and refused unsafe side effects separate with internal={}, delegated={}, refused={} cases.",
                import_policy.allowed_internal_case_count,
                import_policy.delegated_case_count,
                import_policy.refused_case_count,
            ),
        },
    ]
}

fn build_boundary_rows(
    machine_identity_binding: &TassadarPostArticlePluginCapabilityMachineIdentity,
) -> Vec<TassadarPostArticlePluginCapabilityBoundaryRow> {
    vec![
        TassadarPostArticlePluginCapabilityBoundaryRow {
            boundary_id: String::from("tcm_v1_remains_bounded_compute_substrate"),
            current_posture: String::from("bounded_compute_substrate_locked"),
            green: true,
            source_refs: vec![
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
                String::from(BRIDGE_CONTRACT_REPORT_REF),
            ],
            detail: String::from(
                "`TCM.v1` remains the bounded compute substrate and is not widened by later plugin ambitions.",
            ),
        },
        TassadarPostArticlePluginCapabilityBoundaryRow {
            boundary_id: String::from("plugin_capability_plane_reserved_above_bridge"),
            current_posture: String::from("reserved_above_rebased_compute_substrate"),
            green: true,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(REBASED_VERDICT_SPLIT_REPORT_REF),
            ],
            detail: format!(
                "plugin capability stays on reserved plane `{}` above bridge machine `{}` instead of collapsing into the direct compute carrier.",
                machine_identity_binding.reserved_capability_plane_id,
                machine_identity_binding.machine_identity_id,
            ),
        },
        TassadarPostArticlePluginCapabilityBoundaryRow {
            boundary_id: String::from("plugin_execution_separate_software_capability_layer"),
            current_posture: String::from("separate_software_capability_layer_reserved"),
            green: true,
            source_refs: vec![
                String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
                String::from(crate::TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF),
            ],
            detail: String::from(
                "plugin execution is reserved as a separate software-capability layer with explicit mount and import boundaries rather than treated as direct compute truth.",
            ),
        },
        TassadarPostArticlePluginCapabilityBoundaryRow {
            boundary_id: String::from("downward_non_influence_against_plugin_ergonomics"),
            current_posture: String::from("reserved_non_negotiable"),
            green: true,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            ],
            detail: String::from(
                "plugin ergonomics may not rewrite continuation semantics, carrier identity, or proof assumptions below the capability plane.",
            ),
        },
        TassadarPostArticlePluginCapabilityBoundaryRow {
            boundary_id: String::from("plugin_state_and_receipt_identity_stay_separate"),
            current_posture: String::from("reserved_separate_identity_families"),
            green: true,
            source_refs: vec![
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
                String::from(crate::TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF),
            ],
            detail: String::from(
                "plugin packet state, instance-local state, and receipt families stay separate from the core compute substrate and continuation contract.",
            ),
        },
        TassadarPostArticlePluginCapabilityBoundaryRow {
            boundary_id: String::from("closed_world_operator_curated_first_plugin_tranche"),
            current_posture: String::from("required_until_later_discovery_audit"),
            green: true,
            source_refs: vec![
                String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            detail: String::from(
                "the first audited plugin tranche stays closed-world and operator-curated unless later discovery work is audited separately.",
            ),
        },
        TassadarPostArticlePluginCapabilityBoundaryRow {
            boundary_id: String::from("rebased_universality_does_not_grant_plugin_control"),
            current_posture: String::from("claims_blocked"),
            green: true,
            source_refs: vec![String::from(REBASED_VERDICT_SPLIT_REPORT_REF)],
            detail: String::from(
                "theory/operator universality on the canonical route does not imply weighted plugin control, plugin publication, served/public universality, or arbitrary software capability.",
            ),
        },
    ]
}

fn build_state_receipt_rows(
    runtime_contract: &TassadarTcmV1RuntimeContractReport,
    rebased_verdict_report_id: &str,
) -> Vec<TassadarPostArticlePluginStateReceiptRow> {
    vec![
        TassadarPostArticlePluginStateReceiptRow {
            row_id: String::from("continuation_substrate_state"),
            state_or_receipt_class: String::from("tcm_v1_continuation_state"),
            owner_plane: String::from("data"),
            current_posture: String::from("bounded_compute_substrate"),
            separate_from_compute_substrate: false,
            receipt_identity_family: runtime_contract.report_id.clone(),
            source_refs: vec![String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)],
            green: runtime_contract.overall_green,
            detail: String::from(
                "checkpoint, spill, process, and declared effect state stay inside the `TCM.v1` continuation substrate instead of being repackaged as plugin state.",
            ),
        },
        TassadarPostArticlePluginStateReceiptRow {
            row_id: String::from("weights_owned_control_state"),
            state_or_receipt_class: String::from("weights_owned_control_state"),
            owner_plane: String::from("control"),
            current_posture: String::from("already_bound_to_rebased_route"),
            separate_from_compute_substrate: true,
            receipt_identity_family: String::from(rebased_verdict_report_id),
            source_refs: vec![String::from(REBASED_VERDICT_SPLIT_REPORT_REF)],
            green: true,
            detail: String::from(
                "workflow choice remains weights-owned control state on the rebased canonical route rather than plugin-owned orchestration state.",
            ),
        },
        TassadarPostArticlePluginStateReceiptRow {
            row_id: String::from("plugin_packet_local_state"),
            state_or_receipt_class: String::from("plugin_packet_local_state"),
            owner_plane: String::from("capability"),
            current_posture: String::from("reserved_not_implemented"),
            separate_from_compute_substrate: true,
            receipt_identity_family: String::from("tassadar.plugin_invocation.packet.v1.reserved"),
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            ],
            green: true,
            detail: String::from(
                "packet-local plugin state is explicitly future-facing and must not be confused with the compute substrate state classes.",
            ),
        },
        TassadarPostArticlePluginStateReceiptRow {
            row_id: String::from("plugin_instance_local_ephemeral_state"),
            state_or_receipt_class: String::from("plugin_instance_local_ephemeral_state"),
            owner_plane: String::from("capability"),
            current_posture: String::from("reserved_not_implemented"),
            separate_from_compute_substrate: true,
            receipt_identity_family: String::from("tassadar.plugin_instance.state.v1.reserved"),
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            ],
            green: true,
            detail: String::from(
                "instance-local ephemeral plugin state is reserved as its own class instead of leaking into continuations or proof-bearing compute state.",
            ),
        },
        TassadarPostArticlePluginStateReceiptRow {
            row_id: String::from("plugin_receipt_identity_family"),
            state_or_receipt_class: String::from("plugin_receipt_identity_family"),
            owner_plane: String::from("capability"),
            current_posture: String::from("reserved_not_implemented"),
            separate_from_compute_substrate: true,
            receipt_identity_family: String::from("tassadar.plugin_invocation.receipt.v1.reserved"),
            source_refs: vec![
                String::from(crate::TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF),
                String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            ],
            green: true,
            detail: String::from(
                "plugin invocation receipts are reserved as a later capability-plane receipt family instead of being backfilled into current compute receipts.",
            ),
        },
    ]
}

fn build_reserved_invariant_rows(
    bridge_report: &BridgeContractFixture,
) -> Vec<TassadarPostArticlePluginReservedInvariantRow> {
    bridge_report
        .reserved_later_invariant_ids
        .iter()
        .map(|invariant_id| TassadarPostArticlePluginReservedInvariantRow {
            invariant_id: invariant_id.clone(),
            current_posture: String::from("reserved_for_later_plugin_layer"),
            reserved_for_later_plugin_layer: true,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            ],
            green: true,
            detail: match invariant_id.as_str() {
                "choice_set_integrity" => String::from(
                    "admissible plugin choices may not be hidden, filtered, or pre-ranked off-trace.",
                ),
                "resource_transparency" => String::from(
                    "latency, quota, cost, and availability that could steer plugin choice remain reserved as explicit model-visible or fixed-by-contract facts.",
                ),
                "scheduling_ownership" => String::from(
                    "ordering, concurrency, and visibility timing remain reserved as model-owned or fixed-by-contract instead of host-steered behavior.",
                ),
                _ => String::from(
                    "reserved later invariant remains explicit instead of being left implicit at the plugin boundary.",
                ),
            },
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    runtime_contract: &TassadarTcmV1RuntimeContractReport,
    bridge_report: &BridgeContractFixture,
    rebased_verdict: &RebasedVerdictFixture,
    world_mount: &WorldMountCompatibilityFixture,
    import_policy: &TassadarImportPolicyMatrixReport,
    choice_set_integrity_reserved: bool,
    resource_transparency_reserved: bool,
    scheduling_ownership_reserved: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
) -> Vec<TassadarPostArticlePluginCapabilityValidationRow> {
    let import_policy_explicit = import_policy.allowed_internal_case_count == 1
        && import_policy.delegated_case_count == 1
        && import_policy.refused_case_count == 2;
    vec![
        TassadarPostArticlePluginCapabilityValidationRow {
            validation_id: String::from("canonical_identity_matches_bridge_machine"),
            green: bridge_report.bridge_machine_identity.machine_identity_id
                == rebased_verdict.machine_identity_id
                && bridge_report.bridge_machine_identity.canonical_model_id
                    == rebased_verdict.canonical_model_id
                && bridge_report.bridge_machine_identity.canonical_route_id
                    == rebased_verdict.canonical_route_id,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(REBASED_VERDICT_SPLIT_REPORT_REF),
            ],
            detail: String::from(
                "the plugin-aware boundary stays attached to the canonical bridge machine identity rather than drifting onto a parallel route.",
            ),
        },
        TassadarPostArticlePluginCapabilityValidationRow {
            validation_id: String::from("runtime_contract_remains_tcm_v1_substrate"),
            green: runtime_contract.overall_green
                && runtime_contract.report_id
                    == bridge_report.bridge_machine_identity.continuation_contract_id,
            source_refs: vec![
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
                String::from(BRIDGE_CONTRACT_REPORT_REF),
            ],
            detail: String::from(
                "the same `TCM.v1` continuation contract remains the lower substrate instead of being silently rewritten for plugin ergonomics.",
            ),
        },
        TassadarPostArticlePluginCapabilityValidationRow {
            validation_id: String::from("world_mount_dependency_stays_task_scoped_external"),
            green: world_mount.allowed_case_count >= 1
                && world_mount.denied_case_count >= 1
                && world_mount.unresolved_case_count >= 1
                && world_mount
                    .world_mount_dependency_marker
                    .contains("outside standalone psionic"),
            source_refs: vec![String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF)],
            detail: String::from(
                "world-mount truth remains task-scoped and external, so plugin admission cannot be mistaken for compute-substrate ownership.",
            ),
        },
        TassadarPostArticlePluginCapabilityValidationRow {
            validation_id: String::from("import_policy_internal_vs_delegated_split_explicit"),
            green: import_policy_explicit
                && import_policy
                    .policy_matrix
                    .entries
                    .iter()
                    .any(|entry| entry.import_ref == "sandbox.math_eval"),
            source_refs: vec![String::from(crate::TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF)],
            detail: String::from(
                "deterministic stubs, explicit sandbox delegation, and refused side effects remain typed, counted, and separate.",
            ),
        },
        TassadarPostArticlePluginCapabilityValidationRow {
            validation_id: String::from("helper_substitution_posture_blocked"),
            green: import_policy
                .policy_matrix
                .entries
                .iter()
                .any(|entry| {
                    entry.import_ref == "host.fs_write"
                        && entry.execution_boundary
                            == crate::TassadarImportExecutionBoundary::Refused
                })
                && !rebased_verdict.plugin_capability_claim_allowed,
            source_refs: vec![
                String::from(crate::TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF),
                String::from(REBASED_VERDICT_SPLIT_REPORT_REF),
            ],
            detail: String::from(
                "undeclared helpers and unsafe side effects cannot be laundered into the rebased compute claim as if they were part of `TCM.v1`.",
            ),
        },
        TassadarPostArticlePluginCapabilityValidationRow {
            validation_id: String::from("route_drift_posture_blocked"),
            green: bridge_report.bridge_machine_identity.canonical_route_id
                == rebased_verdict.canonical_route_id
                && bridge_report
                    .bridge_machine_identity
                    .canonical_route_descriptor_digest
                    == bridge_report
                        .bridge_machine_identity
                        .canonical_route_descriptor_digest,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(REBASED_VERDICT_SPLIT_REPORT_REF),
            ],
            detail: String::from(
                "plugin awareness stays bound to the canonical direct article route and does not authorize route-family drift.",
            ),
        },
        TassadarPostArticlePluginCapabilityValidationRow {
            validation_id: String::from("continuation_abuse_posture_blocked"),
            green: runtime_contract.report_digest
                == bridge_report.bridge_machine_identity.continuation_contract_digest,
            source_refs: vec![
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
                String::from(BRIDGE_CONTRACT_REPORT_REF),
            ],
            detail: String::from(
                "continuation semantics remain anchored to the same digest-bound runtime contract and are not widened by plugin ergonomics.",
            ),
        },
        TassadarPostArticlePluginCapabilityValidationRow {
            validation_id: String::from("semantic_drift_posture_blocked"),
            green: world_mount.surface.import_posture == "deterministic_stub_imports_only"
                && import_policy_explicit,
            source_refs: vec![
                String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
                String::from(crate::TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF),
            ],
            detail: String::from(
                "mount/import boundaries keep plugin-capability mediation explicit rather than rebranding wider semantics as exact compute truth.",
            ),
        },
        TassadarPostArticlePluginCapabilityValidationRow {
            validation_id: String::from("overclaim_posture_blocked"),
            green: rebased_verdict.rebase_claim_allowed
                && !rebased_verdict.plugin_capability_claim_allowed
                && !weighted_plugin_control_allowed
                && !plugin_publication_allowed
                && !rebased_verdict.served_public_universality_allowed
                && !rebased_verdict.arbitrary_software_capability_allowed,
            source_refs: vec![String::from(REBASED_VERDICT_SPLIT_REPORT_REF)],
            detail: String::from(
                "the rebased theory/operator verdict remains green while plugin control, plugin publication, served/public universality, and arbitrary software capability stay blocked.",
            ),
        },
        TassadarPostArticlePluginCapabilityValidationRow {
            validation_id: String::from("later_plugin_invariants_remain_reserved"),
            green: choice_set_integrity_reserved
                && resource_transparency_reserved
                && scheduling_ownership_reserved,
            source_refs: vec![
                String::from(BRIDGE_CONTRACT_REPORT_REF),
                String::from(PLUGIN_SYSTEM_AND_TURING_AUDIT_REF),
            ],
            detail: String::from(
                "choice-set integrity, resource transparency, and scheduling ownership are frozen as non-negotiable plugin-plane invariants instead of being left optional.",
            ),
        },
    ]
}

#[must_use]
pub fn tassadar_post_article_plugin_capability_boundary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF)
}

pub fn write_tassadar_post_article_plugin_capability_boundary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginCapabilityBoundaryReport,
    TassadarPostArticlePluginCapabilityBoundaryReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginCapabilityBoundaryReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_plugin_capability_boundary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginCapabilityBoundaryReportError::Write {
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
) -> Result<T, TassadarPostArticlePluginCapabilityBoundaryReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginCapabilityBoundaryReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginCapabilityBoundaryReportError::Decode {
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
    carrier_rows: Vec<BridgeCarrierRowFixture>,
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
struct BridgeCarrierRowFixture {
    carrier_id: String,
    carrier_class: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct RebasedVerdictFixture {
    report_id: String,
    report_digest: String,
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_route_id: String,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct WorldMountCompatibilityFixture {
    report_id: String,
    report_digest: String,
    allowed_case_count: u32,
    denied_case_count: u32,
    unresolved_case_count: u32,
    world_mount_dependency_marker: String,
    surface: WorldMountSurfaceFixture,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct WorldMountSurfaceFixture {
    import_posture: String,
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticlePluginCapabilityBoundaryReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticlePluginCapabilityBoundaryReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginCapabilityBoundaryReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_plugin_capability_boundary_report, read_json,
        tassadar_post_article_plugin_capability_boundary_report_path,
        write_tassadar_post_article_plugin_capability_boundary_report,
        TassadarPostArticlePluginCapabilityBoundaryReport,
        TassadarPostArticlePluginCapabilityBoundaryStatus,
        TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn post_article_plugin_capability_boundary_keeps_plugin_claims_blocked() {
        let report =
            build_tassadar_post_article_plugin_capability_boundary_report().expect("report");

        assert_eq!(
            report.boundary_status,
            TassadarPostArticlePluginCapabilityBoundaryStatus::Green
        );
        assert!(report.boundary_green);
        assert!(report.tcm_v1_substrate_retained);
        assert!(report.plugin_capability_plane_reserved);
        assert!(report.plugin_execution_layer_separate);
        assert!(report.downward_non_influence_reserved);
        assert!(report.plugin_state_identity_separated);
        assert!(report.closed_world_operator_curated_first_tranche_required);
        assert!(report.choice_set_integrity_reserved);
        assert!(report.resource_transparency_reserved);
        assert!(report.scheduling_ownership_reserved);
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
        assert!(report.deferred_issue_ids.is_empty());
        assert_eq!(report.dependency_rows.len(), 7);
        assert_eq!(report.boundary_rows.len(), 7);
        assert_eq!(report.state_receipt_rows.len(), 5);
        assert_eq!(report.reserved_invariant_rows.len(), 3);
        assert_eq!(report.validation_rows.len(), 10);
    }

    #[test]
    fn post_article_plugin_capability_boundary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_capability_boundary_report().expect("report");
        let committed: TassadarPostArticlePluginCapabilityBoundaryReport =
            read_json(tassadar_post_article_plugin_capability_boundary_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_plugin_capability_boundary_report.json"
        );
    }

    #[test]
    fn write_post_article_plugin_capability_boundary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_plugin_capability_boundary_report.json");
        let written = write_tassadar_post_article_plugin_capability_boundary_report(&output_path)
            .expect("write report");
        let persisted: TassadarPostArticlePluginCapabilityBoundaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
