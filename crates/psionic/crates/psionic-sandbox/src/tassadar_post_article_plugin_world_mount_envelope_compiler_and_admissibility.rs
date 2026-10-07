use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle,
    TassadarPostArticlePluginAdmissibilityCaseStatus,
    TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundle,
    TASSADAR_POST_ARTICLE_PLUGIN_ADMISSIBILITY_CONTRACT_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
    TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_ID,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_CHECKER_REF:
    &str =
    "scripts/check-tassadar-post-article-plugin-world-mount-envelope-compiler-and-admissibility.sh";

const INVOCATION_RECEIPTS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report.json";
const WORLD_MOUNT_COMPATIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_world_mount_compatibility_report.json";
const IMPORT_POLICY_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_import_policy_matrix_report.json";
const BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_internal_compute_route_policy_report.json";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyClass {
    ReceiptPrecedent,
    WorldMountPrecedent,
    ImportPolicyPrecedent,
    RoutePolicyPrecedent,
    DesignInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginWorldMountEnvelopeCompilerMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub invocation_receipts_report_id: String,
    pub invocation_receipts_report_digest: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub world_mount_envelope_compiler_id: String,
    pub admissibility_contract_id: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub runtime_bundle_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAdmissibilityRuleReportRow {
    pub rule_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCandidateSetReportRow {
    pub candidate_set_id: String,
    pub route_policy_id: String,
    pub world_mount_id: String,
    pub candidate_plugin_ids: Vec<String>,
    pub version_constraints: Vec<String>,
    pub closed_world_enumerated: bool,
    pub explicit_enumeration_required: bool,
    pub hidden_ranking_allowed: bool,
    pub receipt_visible_filter_required: bool,
    pub equivalent_choice_class_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginEquivalentChoiceReportRow {
    pub equivalent_choice_class_id: String,
    pub bounded_candidate_count: u32,
    pub neutral_choice_auditable: bool,
    pub hidden_ranking_allowed: bool,
    pub receipt_visible_transforms_required: bool,
    pub violation_reason_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginWorldMountEnvelopeReportRow {
    pub envelope_id: String,
    pub route_policy_id: String,
    pub world_mount_id: String,
    pub selected_plugin_id: String,
    pub selected_plugin_version: String,
    pub mounted_capability_namespace_ids: Vec<String>,
    pub network_rule_ids: Vec<String>,
    pub artifact_mount_ids: Vec<String>,
    pub timeout_ceiling_millis: u32,
    pub memory_ceiling_bytes: u64,
    pub concurrency_ceiling: u16,
    pub provenance_requirement_ids: Vec<String>,
    pub replay_requirement_id: String,
    pub trust_posture_id: String,
    pub publication_posture_id: String,
    pub closed_world_discovery_assumption: bool,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAdmissibilityCaseReportRow {
    pub case_id: String,
    pub status: TassadarPostArticlePluginAdmissibilityCaseStatus,
    pub route_policy_id: String,
    pub world_mount_id: String,
    pub candidate_set_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_plugin_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_plugin_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_install_id: Option<String>,
    pub trust_posture_id: String,
    pub publication_posture_id: String,
    pub equivalent_choice_class_id: String,
    pub filter_transform_receipt_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denial_reason_id: Option<String>,
    pub receipt_digest: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginWorldMountEnvelopeCompilerValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub invocation_receipts_report_ref: String,
    pub world_mount_compatibility_report_ref: String,
    pub import_policy_matrix_report_ref: String,
    pub broad_internal_compute_route_policy_report_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding:
        TassadarPostArticlePluginWorldMountEnvelopeCompilerMachineIdentityBinding,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundle,
    pub dependency_rows: Vec<TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyRow>,
    pub admissibility_rule_rows: Vec<TassadarPostArticlePluginAdmissibilityRuleReportRow>,
    pub candidate_set_rows: Vec<TassadarPostArticlePluginCandidateSetReportRow>,
    pub equivalent_choice_rows: Vec<TassadarPostArticlePluginEquivalentChoiceReportRow>,
    pub envelope_rows: Vec<TassadarPostArticlePluginWorldMountEnvelopeReportRow>,
    pub case_rows: Vec<TassadarPostArticlePluginAdmissibilityCaseReportRow>,
    pub validation_rows: Vec<TassadarPostArticlePluginWorldMountEnvelopeCompilerValidationRow>,
    pub contract_status: TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityStatus,
    pub contract_green: bool,
    pub operator_internal_only_posture: bool,
    pub admissibility_frozen: bool,
    pub candidate_set_enumeration_frozen: bool,
    pub equivalent_choice_model_frozen: bool,
    pub world_mount_envelope_compiler_frozen: bool,
    pub receipt_visible_filtering_required: bool,
    pub version_constraint_binding_required: bool,
    pub trust_posture_binding_required: bool,
    pub publication_posture_binding_required: bool,
    pub denial_behavior_frozen: bool,
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
pub enum TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError {
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

pub fn build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report(
) -> Result<
    TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReport,
    TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError,
> {
    let invocation_receipts: InvocationReceiptsFixture =
        read_repo_json(INVOCATION_RECEIPTS_REPORT_REF)?;
    let world_mount: WorldMountCompatibilityFixture =
        read_repo_json(WORLD_MOUNT_COMPATIBILITY_REPORT_REF)?;
    let import_policy: ImportPolicyMatrixFixture = read_repo_json(IMPORT_POLICY_MATRIX_REPORT_REF)?;
    let route_policy: BroadInternalComputeRoutePolicyFixture =
        read_repo_json(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF)?;
    let runtime_bundle =
        build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle();

    let invocation_receipts_dependency_closed =
        invocation_receipts.contract_green && invocation_receipts.deferred_issue_ids.is_empty();
    let world_mount_precedent_bound = world_mount.allowed_case_count == 2
        && world_mount.denied_case_count == 1
        && world_mount.unresolved_case_count == 1
        && has_world_mount_case(
            &world_mount,
            "mount.benchmark_graph",
            WorldMountOutcomeFixture::Allowed,
            None,
        )
        && has_world_mount_case(
            &world_mount,
            "mount.validator_search",
            WorldMountOutcomeFixture::Allowed,
            None,
        )
        && has_world_mount_case(
            &world_mount,
            "mount.strict_no_imports",
            WorldMountOutcomeFixture::Denied,
            Some(WorldMountRefusalReasonFixture::ImportPostureIncompatible),
        )
        && has_world_mount_case(
            &world_mount,
            "mount.missing_dependency",
            WorldMountOutcomeFixture::Unresolved,
            Some(WorldMountRefusalReasonFixture::ModuleDependencyMissing),
        );
    let import_policy_boundary_bound = has_import_policy_entry(
        &import_policy,
        "env.clock_stub",
        ImportExecutionBoundaryFixture::InternalOnly,
    ) && has_import_policy_entry(
        &import_policy,
        "sandbox.math_eval",
        ImportExecutionBoundaryFixture::SandboxDelegationOnly,
    ) && has_import_policy_entry(
        &import_policy,
        "host.fs_write",
        ImportExecutionBoundaryFixture::Refused,
    );
    let route_policy_precedent_bound = route_policy.selected_route_count == 1
        && route_policy.promoted_profile_specific_route_count >= 2
        && route_policy.suppressed_route_count >= 3
        && route_policy.refused_route_count >= 1
        && has_route_policy_row(
            &route_policy,
            "route.article_closeout.served_exact",
            RouteDecisionStatusFixture::Selected,
        )
        && has_route_policy_row(
            &route_policy,
            "route.deterministic_import.subset",
            RouteDecisionStatusFixture::PromotedProfileSpecific,
        )
        && has_route_policy_row(
            &route_policy,
            "route.public_broad_family.publication",
            RouteDecisionStatusFixture::Suppressed,
        );

    let machine_identity_binding =
        TassadarPostArticlePluginWorldMountEnvelopeCompilerMachineIdentityBinding {
            machine_identity_id: invocation_receipts
                .machine_identity_binding
                .machine_identity_id
                .clone(),
            canonical_model_id: invocation_receipts
                .machine_identity_binding
                .canonical_model_id
                .clone(),
            canonical_route_id: invocation_receipts
                .machine_identity_binding
                .canonical_route_id
                .clone(),
            canonical_route_descriptor_digest: invocation_receipts
                .machine_identity_binding
                .canonical_route_descriptor_digest
                .clone(),
            canonical_weight_bundle_digest: invocation_receipts
                .machine_identity_binding
                .canonical_weight_bundle_digest
                .clone(),
            canonical_weight_primary_artifact_sha256: invocation_receipts
                .machine_identity_binding
                .canonical_weight_primary_artifact_sha256
                .clone(),
            continuation_contract_id: invocation_receipts
                .machine_identity_binding
                .continuation_contract_id
                .clone(),
            continuation_contract_digest: invocation_receipts
                .machine_identity_binding
                .continuation_contract_digest
                .clone(),
            computational_model_statement_id: invocation_receipts
                .machine_identity_binding
                .computational_model_statement_id
                .clone(),
            invocation_receipts_report_id: invocation_receipts.report_id.clone(),
            invocation_receipts_report_digest: invocation_receipts.report_digest.clone(),
            packet_abi_version: invocation_receipts
                .machine_identity_binding
                .packet_abi_version
                .clone(),
            host_owned_runtime_api_id: invocation_receipts
                .machine_identity_binding
                .host_owned_runtime_api_id
                .clone(),
            engine_abstraction_id: invocation_receipts
                .machine_identity_binding
                .engine_abstraction_id
                .clone(),
            invocation_receipt_profile_id: invocation_receipts
                .machine_identity_binding
                .invocation_receipt_profile_id
                .clone(),
            world_mount_envelope_compiler_id: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_ID,
            ),
            admissibility_contract_id: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_ADMISSIBILITY_CONTRACT_ID,
            ),
            runtime_bundle_id: runtime_bundle.bundle_id.clone(),
            runtime_bundle_digest: runtime_bundle.bundle_digest.clone(),
            runtime_bundle_ref: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
            ),
            detail: format!(
                "machine_identity_id=`{}` canonical_route_id=`{}` invocation_receipts_report_id=`{}` world_mount_envelope_compiler_id=`{}` and runtime_bundle_id=`{}` remain bound together.",
                invocation_receipts.machine_identity_binding.machine_identity_id,
                invocation_receipts.machine_identity_binding.canonical_route_id,
                invocation_receipts.report_id,
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_ID,
                runtime_bundle.bundle_id,
            ),
        };

    let dependency_rows = vec![
        dependency_row(
            "invocation_receipts_contract_closed",
            TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyClass::ReceiptPrecedent,
            invocation_receipts_dependency_closed,
            INVOCATION_RECEIPTS_REPORT_REF,
            Some(invocation_receipts.report_id.clone()),
            Some(invocation_receipts.report_digest.clone()),
            "the earlier invocation-receipt contract is green and no longer defers TAS-202.",
        ),
        dependency_row(
            "world_mount_precedent_bound",
            TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyClass::WorldMountPrecedent,
            world_mount_precedent_bound,
            WORLD_MOUNT_COMPATIBILITY_REPORT_REF,
            Some(world_mount.report_id.clone()),
            Some(world_mount.report_digest.clone()),
            "world-mount compatibility precedent keeps allowed, denied, and unresolved mount outcomes explicit before admissibility can compile runtime envelopes.",
        ),
        dependency_row(
            "import_policy_precedent_bound",
            TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyClass::ImportPolicyPrecedent,
            import_policy_boundary_bound,
            IMPORT_POLICY_MATRIX_REPORT_REF,
            Some(import_policy.report_id.clone()),
            Some(import_policy.report_digest.clone()),
            "import-policy precedent keeps deterministic stubs, sandbox delegation, and refused host effects explicit for envelope compilation.",
        ),
        dependency_row(
            "route_policy_precedent_bound",
            TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyClass::RoutePolicyPrecedent,
            route_policy_precedent_bound,
            BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF,
            Some(route_policy.report_id.clone()),
            Some(route_policy.report_digest.clone()),
            "route-policy precedent keeps selected, promoted, suppressed, and refused internal-compute routes explicit before plugin admissibility can bind them.",
        ),
        dependency_row(
            "plugin_system_admissibility_shape_cited",
            TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyClass::DesignInput,
            true,
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the local plugin-system note remains the cited design input for admissibility checks, closed-world candidate sets, equivalent-choice classes, and compiled runtime envelopes.",
        ),
    ];

    let admissibility_rule_rows = runtime_bundle
        .admissibility_rule_rows
        .iter()
        .map(|row| TassadarPostArticlePluginAdmissibilityRuleReportRow {
            rule_id: row.rule_id.clone(),
            green: row.green,
            source_refs: vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
                ),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            detail: row.detail.clone(),
        })
        .collect::<Vec<_>>();
    let candidate_set_rows = runtime_bundle
        .candidate_set_rows
        .iter()
        .map(|row| TassadarPostArticlePluginCandidateSetReportRow {
            candidate_set_id: row.candidate_set_id.clone(),
            route_policy_id: row.route_policy_id.clone(),
            world_mount_id: row.world_mount_id.clone(),
            candidate_plugin_ids: row.candidate_plugin_ids.clone(),
            version_constraints: row.version_constraints.clone(),
            closed_world_enumerated: row.closed_world_enumerated,
            explicit_enumeration_required: row.explicit_enumeration_required,
            hidden_ranking_allowed: row.hidden_ranking_allowed,
            receipt_visible_filter_required: row.receipt_visible_filter_required,
            equivalent_choice_class_id: row.equivalent_choice_class_id.clone(),
            green: row.closed_world_enumerated
                && row.explicit_enumeration_required
                && !row.hidden_ranking_allowed
                && row.receipt_visible_filter_required,
            source_refs: vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
                ),
                String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            detail: row.detail.clone(),
        })
        .collect::<Vec<_>>();
    let equivalent_choice_rows = runtime_bundle
        .equivalent_choice_rows
        .iter()
        .map(|row| TassadarPostArticlePluginEquivalentChoiceReportRow {
            equivalent_choice_class_id: row.equivalent_choice_class_id.clone(),
            bounded_candidate_count: row.bounded_candidate_count,
            neutral_choice_auditable: row.neutral_choice_auditable,
            hidden_ranking_allowed: row.hidden_ranking_allowed,
            receipt_visible_transforms_required: row.receipt_visible_transforms_required,
            violation_reason_id: row.violation_reason_id.clone(),
            green: row.neutral_choice_auditable
                && !row.hidden_ranking_allowed
                && row.receipt_visible_transforms_required,
            source_refs: vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
                ),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            detail: row.detail.clone(),
        })
        .collect::<Vec<_>>();
    let envelope_rows = runtime_bundle
        .envelope_rows
        .iter()
        .map(|row| TassadarPostArticlePluginWorldMountEnvelopeReportRow {
            envelope_id: row.envelope_id.clone(),
            route_policy_id: row.route_policy_id.clone(),
            world_mount_id: row.world_mount_id.clone(),
            selected_plugin_id: row.selected_plugin_id.clone(),
            selected_plugin_version: row.selected_plugin_version.clone(),
            mounted_capability_namespace_ids: row.mounted_capability_namespace_ids.clone(),
            network_rule_ids: row.network_rule_ids.clone(),
            artifact_mount_ids: row.artifact_mount_ids.clone(),
            timeout_ceiling_millis: row.timeout_ceiling_millis,
            memory_ceiling_bytes: row.memory_ceiling_bytes,
            concurrency_ceiling: row.concurrency_ceiling,
            provenance_requirement_ids: row.provenance_requirement_ids.clone(),
            replay_requirement_id: row.replay_requirement_id.clone(),
            trust_posture_id: row.trust_posture_id.clone(),
            publication_posture_id: row.publication_posture_id.clone(),
            closed_world_discovery_assumption: row.closed_world_discovery_assumption,
            green: row.closed_world_discovery_assumption
                && !row.mounted_capability_namespace_ids.is_empty()
                && !row.provenance_requirement_ids.is_empty(),
            source_refs: vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
                ),
                String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
                String::from(IMPORT_POLICY_MATRIX_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            detail: row.detail.clone(),
        })
        .collect::<Vec<_>>();
    let case_rows = runtime_bundle
        .case_receipts
        .iter()
        .map(|row| TassadarPostArticlePluginAdmissibilityCaseReportRow {
            case_id: row.case_id.clone(),
            status: row.status,
            route_policy_id: row.route_policy_id.clone(),
            world_mount_id: row.world_mount_id.clone(),
            candidate_set_id: row.candidate_set_id.clone(),
            selected_plugin_id: row.selected_plugin_id.clone(),
            selected_plugin_version: row.selected_plugin_version.clone(),
            selected_install_id: row.selected_install_id.clone(),
            trust_posture_id: row.trust_posture_id.clone(),
            publication_posture_id: row.publication_posture_id.clone(),
            equivalent_choice_class_id: row.equivalent_choice_class_id.clone(),
            filter_transform_receipt_ids: row.filter_transform_receipt_ids.clone(),
            envelope_id: row.envelope_id.clone(),
            denial_reason_id: row.denial_reason_id.clone(),
            receipt_digest: row.receipt_digest.clone(),
            green: true,
            source_refs: vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
                ),
                String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
                String::from(IMPORT_POLICY_MATRIX_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            detail: row.note.clone(),
        })
        .collect::<Vec<_>>();

    let admissibility_frozen =
        admissibility_rule_rows.len() == 9 && admissibility_rule_rows.iter().all(|row| row.green);
    let candidate_set_enumeration_frozen = candidate_set_rows.len() == 5
        && candidate_set_rows.iter().all(|row| row.green)
        && candidate_set_rows
            .iter()
            .all(|row| !row.candidate_plugin_ids.is_empty() && !row.version_constraints.is_empty());
    let equivalent_choice_model_frozen = equivalent_choice_rows.len() == 5
        && equivalent_choice_rows.iter().all(|row| row.green)
        && equivalent_choice_rows
            .iter()
            .all(|row| !row.violation_reason_id.is_empty());
    let world_mount_envelope_compiler_frozen = envelope_rows.len() == 2
        && envelope_rows.iter().all(|row| row.green)
        && envelope_rows
            .iter()
            .all(|row| row.timeout_ceiling_millis > 0 && row.memory_ceiling_bytes > 0);
    let receipt_visible_filtering_required = candidate_set_rows
        .iter()
        .all(|row| row.receipt_visible_filter_required)
        && case_rows
            .iter()
            .filter(|row| {
                row.status
                    == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactAdmittedEnvelope
            })
            .all(|row| !row.filter_transform_receipt_ids.is_empty());
    let version_constraint_binding_required = candidate_set_rows
        .iter()
        .all(|row| !row.version_constraints.is_empty())
        && case_rows
            .iter()
            .any(|row| row.denial_reason_id.as_deref() == Some("version_constraint_unsatisfied"));
    let trust_posture_binding_required =
        case_rows.iter().all(|row| !row.trust_posture_id.is_empty())
            && case_rows
                .iter()
                .any(|row| row.trust_posture_id == "challenge_gated_install");
    let publication_posture_binding_required = case_rows
        .iter()
        .all(|row| !row.publication_posture_id.is_empty())
        && case_rows
            .iter()
            .any(|row| row.publication_posture_id == "served_disallowed");
    let denial_behavior_frozen = case_rows.iter().any(|row| {
        row.denial_reason_id.as_deref() == Some("import_posture_incompatible")
            && row.status == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactDeniedEnvelope
    }) && case_rows.iter().any(|row| {
        row.denial_reason_id.as_deref() == Some("version_constraint_unsatisfied")
            && row.status == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactDeniedEnvelope
    }) && case_rows.iter().any(|row| {
        row.denial_reason_id.as_deref() == Some("module_dependency_missing")
            && row.status
                == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactQuarantinedEnvelope
    }) && case_rows.iter().any(|row| {
        row.denial_reason_id.as_deref() == Some("equivalent_choice_neutrality_violation")
            && row.status
                == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactSuppressedEnvelope
    }) && case_rows.iter().any(|row| {
        row.denial_reason_id.as_deref() == Some("route_policy_suppressed")
            && row.status
                == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactSuppressedEnvelope
    });
    let operator_internal_only_posture = invocation_receipts.operator_internal_only_posture
        && invocation_receipts.rebase_claim_allowed
        && !invocation_receipts.plugin_capability_claim_allowed
        && !invocation_receipts.plugin_publication_allowed;

    let validation_rows = vec![
        validation_row(
            "invocation_receipts_dependency_closed",
            invocation_receipts_dependency_closed,
            &[INVOCATION_RECEIPTS_REPORT_REF],
            "the earlier invocation-receipt contract is green and no longer defers TAS-202.",
        ),
        validation_row(
            "world_mount_precedent_bound",
            world_mount_precedent_bound,
            &[WORLD_MOUNT_COMPATIBILITY_REPORT_REF],
            "world-mount compatibility precedent still carries allowed, denied, and unresolved mount posture explicitly.",
        ),
        validation_row(
            "import_policy_precedent_bound",
            import_policy_boundary_bound,
            &[IMPORT_POLICY_MATRIX_REPORT_REF],
            "import-policy precedent still separates deterministic stubs, sandbox delegation, and refused host side effects.",
        ),
        validation_row(
            "route_policy_precedent_bound",
            route_policy_precedent_bound,
            &[BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF],
            "route-policy precedent still keeps selected, promoted, suppressed, and refused routes explicit.",
        ),
        validation_row(
            "candidate_set_enumeration_frozen",
            candidate_set_enumeration_frozen && receipt_visible_filtering_required,
            &[TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF],
            "candidate sets remain closed-world, explicitly enumerated, and receipt-visible.",
        ),
        validation_row(
            "equivalent_choice_model_frozen",
            equivalent_choice_model_frozen,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
                LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            ],
            "equivalent-choice classes stay bounded, auditable, and fail closed on hidden ranking.",
        ),
        validation_row(
            "world_mount_envelopes_compiled",
            world_mount_envelope_compiler_frozen
                && trust_posture_binding_required
                && publication_posture_binding_required
                && version_constraint_binding_required,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
                WORLD_MOUNT_COMPATIBILITY_REPORT_REF,
                IMPORT_POLICY_MATRIX_REPORT_REF,
                BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF,
            ],
            "runtime envelopes stay compiled with explicit capability, network, artifact, timeout, memory, replay, trust, and publication posture.",
        ),
        validation_row(
            "typed_denial_and_quarantine_behavior",
            denial_behavior_frozen,
            &[TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF],
            "denied, suppressed, and quarantined posture remain typed instead of collapsing into one generic failure.",
        ),
        validation_row(
            "overclaim_posture_blocked",
            operator_internal_only_posture,
            &[
                INVOCATION_RECEIPTS_REPORT_REF,
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
            ],
            "the admissibility contract stays operator/internal-only and does not imply weighted plugin control, plugin publication, served/public universality, or arbitrary software capability.",
        ),
    ];

    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && admissibility_frozen
        && candidate_set_enumeration_frozen
        && equivalent_choice_model_frozen
        && world_mount_envelope_compiler_frozen
        && receipt_visible_filtering_required
        && version_constraint_binding_required
        && trust_posture_binding_required
        && publication_posture_binding_required
        && denial_behavior_frozen
        && validation_rows.iter().all(|row| row.green)
        && operator_internal_only_posture;
    let contract_status = if contract_green {
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityStatus::Green
    } else {
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityStatus::Incomplete
    };
    let rebase_claim_allowed = contract_green;

    let mut report =
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReport {
            schema_version: 1,
            report_id: String::from(
                "tassadar.post_article_plugin_world_mount_envelope_compiler_and_admissibility.report.v1",
            ),
            checker_script_ref: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_CHECKER_REF,
            ),
            invocation_receipts_report_ref: String::from(INVOCATION_RECEIPTS_REPORT_REF),
            world_mount_compatibility_report_ref: String::from(
                WORLD_MOUNT_COMPATIBILITY_REPORT_REF,
            ),
            import_policy_matrix_report_ref: String::from(IMPORT_POLICY_MATRIX_REPORT_REF),
            broad_internal_compute_route_policy_report_ref: String::from(
                BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF,
            ),
            local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            supporting_material_refs: vec![
                String::from(INVOCATION_RECEIPTS_REPORT_REF),
                String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
                String::from(IMPORT_POLICY_MATRIX_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            machine_identity_binding,
            runtime_bundle_ref: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
            ),
            runtime_bundle,
            dependency_rows,
            admissibility_rule_rows,
            candidate_set_rows,
            equivalent_choice_rows,
            envelope_rows,
            case_rows,
            validation_rows,
            contract_status,
            contract_green,
            operator_internal_only_posture,
            admissibility_frozen,
            candidate_set_enumeration_frozen,
            equivalent_choice_model_frozen,
            world_mount_envelope_compiler_frozen,
            receipt_visible_filtering_required,
            version_constraint_binding_required,
            trust_posture_binding_required,
            publication_posture_binding_required,
            denial_behavior_frozen,
            rebase_claim_allowed,
            plugin_capability_claim_allowed: false,
            weighted_plugin_control_allowed: false,
            plugin_publication_allowed: false,
            served_public_universality_allowed: false,
            arbitrary_software_capability_allowed: false,
            deferred_issue_ids: Vec::new(),
            claim_boundary: String::from(
                "this sandbox report freezes the canonical post-article plugin admissibility contract above the invocation-receipt layer and the earlier world-mount, import-policy, and route-policy precedents. It keeps closed-world candidate sets, equivalent-choice classes, route and mount binding, version constraints, trust posture, publication posture, compiled runtime envelopes, and typed denied or quarantined outcomes machine-readable while keeping weighted plugin control, plugin publication, served/public universality, and arbitrary software capability blocked.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
    report.summary = format!(
        "Post-article plugin world-mount admissibility report keeps contract_status={:?}, dependency_rows={}, candidate_set_rows={}, equivalent_choice_rows={}, envelope_rows={}, case_rows={}, validation_rows={}, and deferred_issue_ids={}.",
        report.contract_status,
        report.dependency_rows.len(),
        report.candidate_set_rows.len(),
        report.equivalent_choice_rows.len(),
        report.envelope_rows.len(),
        report.case_rows.len(),
        report.validation_rows.len(),
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
    )
}

pub fn write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReport,
    TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report =
        build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report(
        )?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn dependency_row(
    dependency_id: &str,
    dependency_class: TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyClass,
    satisfied: bool,
    source_ref: &str,
    bound_report_id: Option<String>,
    bound_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyRow {
    TassadarPostArticlePluginWorldMountEnvelopeCompilerDependencyRow {
        dependency_id: String::from(dependency_id),
        dependency_class,
        satisfied,
        source_ref: String::from(source_ref),
        bound_report_id,
        bound_report_digest,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticlePluginWorldMountEnvelopeCompilerValidationRow {
    TassadarPostArticlePluginWorldMountEnvelopeCompilerValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn has_world_mount_case(
    report: &WorldMountCompatibilityFixture,
    mount_id: &str,
    outcome: WorldMountOutcomeFixture,
    refusal_reason: Option<WorldMountRefusalReasonFixture>,
) -> bool {
    report.case_reports.iter().any(|case| {
        case.descriptor.mount_id == mount_id
            && case.outcome == outcome
            && case.refusal_reason == refusal_reason
    })
}

fn has_import_policy_entry(
    report: &ImportPolicyMatrixFixture,
    import_ref: &str,
    execution_boundary: ImportExecutionBoundaryFixture,
) -> bool {
    report.policy_matrix.entries.iter().any(|entry| {
        entry.import_ref == import_ref && entry.execution_boundary == execution_boundary
    })
}

fn has_route_policy_row(
    report: &BroadInternalComputeRoutePolicyFixture,
    route_policy_id: &str,
    decision_status: RouteDecisionStatusFixture,
) -> bool {
    report
        .rows
        .iter()
        .any(|row| row.route_policy_id == route_policy_id && row.decision_status == decision_status)
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

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[derive(Clone, Debug, Deserialize)]
struct InvocationReceiptsFixture {
    report_id: String,
    report_digest: String,
    contract_green: bool,
    operator_internal_only_posture: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    plugin_publication_allowed: bool,
    deferred_issue_ids: Vec<String>,
    machine_identity_binding: InvocationReceiptsMachineIdentityBindingFixture,
}

#[derive(Clone, Debug, Deserialize)]
struct InvocationReceiptsMachineIdentityBindingFixture {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
    computational_model_statement_id: String,
    packet_abi_version: String,
    host_owned_runtime_api_id: String,
    engine_abstraction_id: String,
    invocation_receipt_profile_id: String,
}

#[derive(Clone, Debug, Deserialize)]
struct WorldMountCompatibilityFixture {
    report_id: String,
    report_digest: String,
    allowed_case_count: u32,
    denied_case_count: u32,
    unresolved_case_count: u32,
    case_reports: Vec<WorldMountCaseFixture>,
}

#[derive(Clone, Debug, Deserialize)]
struct WorldMountCaseFixture {
    descriptor: WorldMountDescriptorFixture,
    outcome: WorldMountOutcomeFixture,
    #[serde(default)]
    refusal_reason: Option<WorldMountRefusalReasonFixture>,
}

#[derive(Clone, Debug, Deserialize)]
struct WorldMountDescriptorFixture {
    mount_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WorldMountOutcomeFixture {
    Allowed,
    Denied,
    Unresolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WorldMountRefusalReasonFixture {
    ImportPostureIncompatible,
    ModuleDependencyMissing,
}

#[derive(Clone, Debug, Deserialize)]
struct ImportPolicyMatrixFixture {
    report_id: String,
    report_digest: String,
    policy_matrix: ImportPolicyMatrixBodyFixture,
}

#[derive(Clone, Debug, Deserialize)]
struct ImportPolicyMatrixBodyFixture {
    entries: Vec<ImportPolicyEntryFixture>,
}

#[derive(Clone, Debug, Deserialize)]
struct ImportPolicyEntryFixture {
    import_ref: String,
    execution_boundary: ImportExecutionBoundaryFixture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportExecutionBoundaryFixture {
    InternalOnly,
    SandboxDelegationOnly,
    Refused,
}

#[derive(Clone, Debug, Deserialize)]
struct BroadInternalComputeRoutePolicyFixture {
    report_id: String,
    report_digest: String,
    rows: Vec<BroadInternalComputeRoutePolicyRowFixture>,
    selected_route_count: u32,
    promoted_profile_specific_route_count: u32,
    suppressed_route_count: u32,
    refused_route_count: u32,
}

#[derive(Clone, Debug, Deserialize)]
struct BroadInternalComputeRoutePolicyRowFixture {
    route_policy_id: String,
    decision_status: RouteDecisionStatusFixture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RouteDecisionStatusFixture {
    Selected,
    PromotedProfileSpecific,
    Suppressed,
    Refused,
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report,
        read_repo_json,
        tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report_path,
        write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report,
        TassadarPostArticlePluginAdmissibilityCaseStatus,
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReport,
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityStatus,
        TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
    };

    #[test]
    fn post_article_plugin_world_mount_admissibility_report_keeps_frontier_explicit() {
        let report =
            build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report()
                .expect("report");

        assert_eq!(
            report.contract_status,
            TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityStatus::Green
        );
        assert_eq!(
            report
                .machine_identity_binding
                .world_mount_envelope_compiler_id,
            "tassadar.plugin_runtime.world_mount_envelope_compiler.v1"
        );
        assert_eq!(
            report.machine_identity_binding.admissibility_contract_id,
            "tassadar.plugin_runtime.admissibility.v1"
        );
        assert_eq!(report.dependency_rows.len(), 5);
        assert_eq!(report.admissibility_rule_rows.len(), 9);
        assert_eq!(report.candidate_set_rows.len(), 5);
        assert_eq!(report.equivalent_choice_rows.len(), 5);
        assert_eq!(report.envelope_rows.len(), 2);
        assert_eq!(report.case_rows.len(), 7);
        assert_eq!(report.validation_rows.len(), 9);
        assert!(report.deferred_issue_ids.is_empty());
        assert!(report.operator_internal_only_posture);
        assert!(report.admissibility_frozen);
        assert!(report.candidate_set_enumeration_frozen);
        assert!(report.equivalent_choice_model_frozen);
        assert!(report.world_mount_envelope_compiler_frozen);
        assert!(report.receipt_visible_filtering_required);
        assert!(report.version_constraint_binding_required);
        assert!(report.trust_posture_binding_required);
        assert!(report.publication_posture_binding_required);
        assert!(report.denial_behavior_frozen);
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
        assert!(report.case_rows.iter().any(|row| {
            row.status == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactSuppressedEnvelope
                && row.denial_reason_id.as_deref() == Some("route_policy_suppressed")
        }));
    }

    #[test]
    fn post_article_plugin_world_mount_admissibility_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report()
                .expect("report");
        let committed: TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReport =
            read_repo_json(
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report.json"
            )
        );
    }

    #[test]
    fn write_post_article_plugin_world_mount_admissibility_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report.json",
        );
        let written =
            write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
