use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_manifest_identity_contract_report.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-plugin-manifest-identity-contract.sh";

const PLUGIN_CHARTER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_charter_authority_boundary_report.json";
const MODULE_TRUST_ISOLATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_trust_isolation_report.json";
const MODULE_PROMOTION_STATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_promotion_state_report.json";
const INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_compute_package_manager_report.json";
const INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_compute_package_route_policy_report.json";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const CANONICAL_ARCHITECTURE_BOUNDARY_REF: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const CANONICAL_ARCHITECTURE_ANCHOR_CRATE: &str = "psionic-transformer";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginManifestIdentityContractStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginManifestDependencyClass {
    ProofCarrying,
    GovernanceDependency,
    CatalogDependency,
    RoutePolicyDependency,
    DesignInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginManifestMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub plugin_charter_report_id: String,
    pub plugin_charter_report_digest: String,
    pub canonical_architecture_anchor_crate: String,
    pub canonical_architecture_boundary_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginManifestDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticlePluginManifestDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginManifestFieldRow {
    pub field_id: String,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationIdentityRow {
    pub identity_id: String,
    pub required_fields: Vec<String>,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginHotSwapRuleRow {
    pub rule_id: String,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPackagingRow {
    pub packaging_id: String,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginManifestValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginManifestIdentityContractReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub plugin_charter_report_ref: String,
    pub module_trust_isolation_report_ref: String,
    pub module_promotion_state_report_ref: String,
    pub internal_compute_package_manager_report_ref: String,
    pub internal_compute_package_route_policy_report_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding: TassadarPostArticlePluginManifestMachineIdentityBinding,
    pub dependency_rows: Vec<TassadarPostArticlePluginManifestDependencyRow>,
    pub manifest_field_rows: Vec<TassadarPostArticlePluginManifestFieldRow>,
    pub invocation_identity_rows: Vec<TassadarPostArticlePluginInvocationIdentityRow>,
    pub hot_swap_rule_rows: Vec<TassadarPostArticlePluginHotSwapRuleRow>,
    pub packaging_rows: Vec<TassadarPostArticlePluginPackagingRow>,
    pub validation_rows: Vec<TassadarPostArticlePluginManifestValidationRow>,
    pub contract_status: TassadarPostArticlePluginManifestIdentityContractStatus,
    pub contract_green: bool,
    pub operator_internal_only_posture: bool,
    pub manifest_fields_frozen: bool,
    pub canonical_invocation_identity_frozen: bool,
    pub hot_swap_rules_frozen: bool,
    pub multi_module_packaging_explicit: bool,
    pub linked_bundle_identity_explicit: bool,
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
pub enum TassadarPostArticlePluginManifestIdentityContractReportError {
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

pub fn build_tassadar_post_article_plugin_manifest_identity_contract_report(
) -> Result<
    TassadarPostArticlePluginManifestIdentityContractReport,
    TassadarPostArticlePluginManifestIdentityContractReportError,
> {
    let charter: PluginCharterFixture = read_repo_json(PLUGIN_CHARTER_REPORT_REF)?;
    let trust: ModuleTrustIsolationFixture = read_repo_json(MODULE_TRUST_ISOLATION_REPORT_REF)?;
    let promotion: ModulePromotionStateFixture =
        read_repo_json(MODULE_PROMOTION_STATE_REPORT_REF)?;
    let package_manager: InternalComputePackageManagerFixture =
        read_repo_json(INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF)?;
    let route_policy: InternalComputePackageRoutePolicyFixture =
        read_repo_json(INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF)?;

    let operator_internal_only_posture = charter.internal_only_plugin_posture
        && charter.current_publication_posture == "internal_only_until_later_plugin_platform_gates"
        && !charter.plugin_publication_allowed;
    let single_module_packages_explicit = package_manager
        .package_entries
        .iter()
        .filter(|entry| entry.module_refs.len() == 1)
        .count()
        >= 2;
    let linked_bundle_identity_explicit = package_manager
        .package_entries
        .iter()
        .any(|entry| entry.package_id == "package.verifier_search_stack.v1" && entry.module_refs.len() == 2);
    let multi_module_packaging_explicit = single_module_packages_explicit
        && linked_bundle_identity_explicit
        && route_policy.overall_green;
    let manifest_fields_frozen = true;
    let canonical_invocation_identity_frozen = charter.host_executes_but_does_not_decide
        && route_policy.overall_green
        && !package_manager.portability_envelope_ids.is_empty();
    let hot_swap_rules_frozen = charter.governance_receipts_required
        && trust.refused_case_count >= 1
        && promotion.challenge_open_count >= 1
        && route_policy.refused_case_count >= 1;
    let rebase_claim_allowed = charter.rebase_claim_allowed;
    let plugin_capability_claim_allowed = false;
    let weighted_plugin_control_allowed = false;
    let plugin_publication_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;

    let machine_identity_binding = TassadarPostArticlePluginManifestMachineIdentityBinding {
        machine_identity_id: charter.machine_identity_binding.machine_identity_id.clone(),
        canonical_model_id: charter.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: charter.machine_identity_binding.canonical_route_id.clone(),
        canonical_route_descriptor_digest: charter
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: charter
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: charter
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: charter
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: charter
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: charter
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        plugin_charter_report_id: charter.report_id.clone(),
        plugin_charter_report_digest: charter.report_digest.clone(),
        canonical_architecture_anchor_crate: String::from(CANONICAL_ARCHITECTURE_ANCHOR_CRATE),
        canonical_architecture_boundary_ref: String::from(CANONICAL_ARCHITECTURE_BOUNDARY_REF),
        detail: format!(
            "machine_identity_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` computational_model_statement_id=`{}` and plugin_charter_report_id=`{}` remain the manifest contract anchor in `{}`.",
            charter.machine_identity_binding.machine_identity_id,
            charter.machine_identity_binding.canonical_model_id,
            charter.machine_identity_binding.canonical_route_id,
            charter.machine_identity_binding.computational_model_statement_id,
            charter.report_id,
            CANONICAL_ARCHITECTURE_ANCHOR_CRATE,
        ),
    };

    let dependency_rows = build_dependency_rows(
        &charter,
        &trust,
        &promotion,
        &package_manager,
        &route_policy,
        operator_internal_only_posture,
    );
    let manifest_field_rows = build_manifest_field_rows(
        &charter,
        &package_manager,
        &route_policy,
        operator_internal_only_posture,
    );
    let invocation_identity_rows =
        build_invocation_identity_rows(&charter, &package_manager, &route_policy);
    let hot_swap_rule_rows = build_hot_swap_rule_rows(&charter, &trust, &promotion, &route_policy);
    let packaging_rows = build_packaging_rows(
        &package_manager,
        &route_policy,
        single_module_packages_explicit,
        linked_bundle_identity_explicit,
    );
    let validation_rows = build_validation_rows(
        &charter,
        &trust,
        &promotion,
        &package_manager,
        &route_policy,
        multi_module_packaging_explicit,
        linked_bundle_identity_explicit,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        weighted_plugin_control_allowed,
        plugin_publication_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
    );

    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && manifest_field_rows.iter().all(|row| row.green)
        && invocation_identity_rows.iter().all(|row| row.green)
        && hot_swap_rule_rows.iter().all(|row| row.green)
        && packaging_rows.iter().all(|row| row.green)
        && validation_rows.iter().all(|row| row.green)
        && operator_internal_only_posture
        && manifest_fields_frozen
        && canonical_invocation_identity_frozen
        && hot_swap_rules_frozen
        && multi_module_packaging_explicit
        && linked_bundle_identity_explicit
        && rebase_claim_allowed
        && !plugin_capability_claim_allowed
        && !weighted_plugin_control_allowed
        && !plugin_publication_allowed
        && !served_public_universality_allowed
        && !arbitrary_software_capability_allowed;
    let contract_status = if contract_green {
        TassadarPostArticlePluginManifestIdentityContractStatus::Green
    } else {
        TassadarPostArticlePluginManifestIdentityContractStatus::Incomplete
    };

    let mut report = TassadarPostArticlePluginManifestIdentityContractReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_plugin_manifest_identity_contract.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_CHECKER_REF,
        ),
        plugin_charter_report_ref: String::from(PLUGIN_CHARTER_REPORT_REF),
        module_trust_isolation_report_ref: String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
        module_promotion_state_report_ref: String::from(MODULE_PROMOTION_STATE_REPORT_REF),
        internal_compute_package_manager_report_ref: String::from(
            INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF,
        ),
        internal_compute_package_route_policy_report_ref: String::from(
            INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF,
        ),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        supporting_material_refs: vec![
            String::from(PLUGIN_CHARTER_REPORT_REF),
            String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
            String::from(MODULE_PROMOTION_STATE_REPORT_REF),
            String::from(INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF),
            String::from(INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF),
            String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            String::from(CANONICAL_ARCHITECTURE_BOUNDARY_REF),
        ],
        machine_identity_binding,
        dependency_rows,
        manifest_field_rows,
        invocation_identity_rows,
        hot_swap_rule_rows,
        packaging_rows,
        validation_rows,
        contract_status,
        contract_green,
        operator_internal_only_posture,
        manifest_fields_frozen,
        canonical_invocation_identity_frozen,
        hot_swap_rules_frozen,
        multi_module_packaging_explicit,
        linked_bundle_identity_explicit,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        weighted_plugin_control_allowed,
        plugin_publication_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        deferred_issue_ids: Vec::new(),
        claim_boundary: String::from(
            "this report freezes the canonical plugin manifest, identity, and hot-swap contract above the rebased post-article machine without widening the current claim surface. It defines the required manifest fields, canonical invocation identity, compatibility and hot-swap rules, and explicit linked-bundle packaging posture for named plugin artifacts while preserving operator/internal-only release posture and leaving weighted plugin capability, plugin publication, served/public universality, and arbitrary software capability blocked until later ABI/runtime/controller/platform issues land.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "post-article plugin manifest contract binds machine_identity_id=`{}`, canonical_route_id=`{}`, contract_status={:?}, dependency_rows={}, manifest_field_rows={}, hot_swap_rule_rows={}, and deferred_issue_ids={}.",
        report.machine_identity_binding.machine_identity_id,
        report.machine_identity_binding.canonical_route_id,
        report.contract_status,
        report.dependency_rows.len(),
        report.manifest_field_rows.len(),
        report.hot_swap_rule_rows.len(),
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_manifest_identity_contract_report|",
        &report,
    );
    Ok(report)
}

fn build_dependency_rows(
    charter: &PluginCharterFixture,
    trust: &ModuleTrustIsolationFixture,
    promotion: &ModulePromotionStateFixture,
    package_manager: &InternalComputePackageManagerFixture,
    route_policy: &InternalComputePackageRoutePolicyFixture,
    operator_internal_only_posture: bool,
) -> Vec<TassadarPostArticlePluginManifestDependencyRow> {
    vec![
        TassadarPostArticlePluginManifestDependencyRow {
            dependency_id: String::from("plugin_charter_authority_boundary"),
            dependency_class: TassadarPostArticlePluginManifestDependencyClass::ProofCarrying,
            satisfied: charter.charter_green
                && operator_internal_only_posture
                && charter.governance_receipts_required,
            source_ref: String::from(PLUGIN_CHARTER_REPORT_REF),
            bound_report_id: Some(charter.report_id.clone()),
            bound_report_digest: Some(charter.report_digest.clone()),
            detail: String::from(
                "the manifest contract inherits the green plugin charter instead of inventing a parallel plugin authority story.",
            ),
        },
        TassadarPostArticlePluginManifestDependencyRow {
            dependency_id: String::from("module_trust_isolation"),
            dependency_class: TassadarPostArticlePluginManifestDependencyClass::GovernanceDependency,
            satisfied: trust.allowed_case_count >= 1
                && trust.refused_case_count >= 1
                && trust.cross_tier_refusal_count >= 1
                && trust.privilege_escalation_refusal_count >= 1
                && trust.mount_policy_refusal_count >= 1,
            source_ref: String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
            bound_report_id: Some(trust.report_id.clone()),
            bound_report_digest: Some(trust.report_digest.clone()),
            detail: String::from(
                "trust tiers remain typed and refusal-backed so manifest compatibility cannot silently widen privilege.",
            ),
        },
        TassadarPostArticlePluginManifestDependencyRow {
            dependency_id: String::from("module_promotion_state"),
            dependency_class: TassadarPostArticlePluginManifestDependencyClass::GovernanceDependency,
            satisfied: promotion.active_promoted_count >= 1
                && promotion.challenge_open_count >= 1
                && promotion.quarantined_count >= 1
                && promotion.revoked_count >= 1
                && promotion.superseded_count >= 1,
            source_ref: String::from(MODULE_PROMOTION_STATE_REPORT_REF),
            bound_report_id: Some(promotion.report_id.clone()),
            bound_report_digest: Some(promotion.report_digest.clone()),
            detail: String::from(
                "promotion evidence, quarantine, revocation, and supersession remain explicit prerequisites for later manifest widening.",
            ),
        },
        TassadarPostArticlePluginManifestDependencyRow {
            dependency_id: String::from("internal_compute_package_manager"),
            dependency_class: TassadarPostArticlePluginManifestDependencyClass::CatalogDependency,
            satisfied: package_manager.package_entries.len() == 3
                && package_manager.exact_case_count == 3
                && package_manager.refusal_case_count == 3
                && package_manager.public_package_ids.len() == 3,
            source_ref: String::from(INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF),
            bound_report_id: Some(package_manager.report_id.clone()),
            bound_report_digest: Some(package_manager.report_digest.clone()),
            detail: String::from(
                "named internal packages already make software artifacts explicit, bounded, and refusal-backed instead of arbitrary package discovery.",
            ),
        },
        TassadarPostArticlePluginManifestDependencyRow {
            dependency_id: String::from("internal_compute_package_route_policy"),
            dependency_class: TassadarPostArticlePluginManifestDependencyClass::RoutePolicyDependency,
            satisfied: route_policy.overall_green
                && route_policy.selected_case_count == 3
                && route_policy.refused_case_count == 3
                && route_policy.default_served_package_ids.is_empty(),
            source_ref: String::from(INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF),
            bound_report_id: Some(route_policy.report_id.clone()),
            bound_report_digest: Some(route_policy.report_digest.clone()),
            detail: String::from(
                "route policy keeps named packages explicit, default-served empty, and refusals typed so manifests do not imply arbitrary plugin discovery or serving.",
            ),
        },
        TassadarPostArticlePluginManifestDependencyRow {
            dependency_id: String::from("local_plugin_system_spec"),
            dependency_class: TassadarPostArticlePluginManifestDependencyClass::DesignInput,
            satisfied: true,
            source_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            bound_report_id: None,
            bound_report_digest: None,
            detail: String::from(
                "the draft plugin-system note remains the design input for canonical manifest fields, invocation identity, and hot-swap rules.",
            ),
        },
    ]
}

fn build_manifest_field_rows(
    charter: &PluginCharterFixture,
    package_manager: &InternalComputePackageManagerFixture,
    route_policy: &InternalComputePackageRoutePolicyFixture,
    operator_internal_only_posture: bool,
) -> Vec<TassadarPostArticlePluginManifestFieldRow> {
    let common_sources = vec![
        String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        String::from(PLUGIN_CHARTER_REPORT_REF),
    ];
    vec![
        manifest_field_row(
            "plugin_id",
            "required_named_artifact_key",
            true,
            common_sources.clone(),
            "every plugin must carry one stable `plugin_id`; named internal packages already prove the repo does not need loose anonymous modules as the artifact vocabulary.",
        ),
        manifest_field_row(
            "plugin_version",
            "required_hot_swap_key",
            true,
            common_sources.clone(),
            "every plugin must carry one stable `plugin_version` so artifact replacement is explicit rather than inferred from mutable host state.",
        ),
        manifest_field_row(
            "artifact_digest",
            "required_integrity_key",
            !package_manager.public_package_ids.is_empty(),
            common_sources.clone(),
            "every plugin must carry one `artifact_digest` so identity and integrity stay machine-checkable.",
        ),
        manifest_field_row(
            "declared_exports",
            "required_callable_surface",
            route_policy.overall_green,
            common_sources.clone(),
            "every plugin must declare callable exports explicitly; undeclared exports are outside the contract.",
        ),
        manifest_field_row(
            "packet_abi_version",
            "required_abi_selector",
            charter.plugin_language_boundary_frozen,
            common_sources.clone(),
            "every plugin manifest must declare one packet ABI version instead of hiding ABI drift in host adapters.",
        ),
        manifest_field_row(
            "input_schema_id",
            "required_schema_key",
            true,
            common_sources.clone(),
            "every plugin manifest must carry one explicit input schema id for fail-closed invocation.",
        ),
        manifest_field_row(
            "output_schema_id",
            "required_schema_key",
            true,
            common_sources.clone(),
            "every plugin manifest must carry one explicit output schema id for fail-closed reinjection.",
        ),
        manifest_field_row(
            "limits",
            "required_timeout_memory_fuel_envelope",
            charter.governance_receipts_required,
            common_sources.clone(),
            "every plugin manifest must declare time, memory, and fuel ceilings inside one bounded envelope.",
        ),
        manifest_field_row(
            "trust_tier",
            "required_trust_posture_key",
            operator_internal_only_posture,
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
            ],
            "every plugin manifest must declare one trust tier so widening privilege is explicit and challengeable.",
        ),
        manifest_field_row(
            "replay_class",
            "required_replay_posture_key",
            charter.governance_receipts_required,
            common_sources.clone(),
            "every plugin manifest must declare one replay class so hot-swap and retry posture stay typed.",
        ),
        manifest_field_row(
            "evidence_settings",
            "required_receipt_emission_key",
            charter.governance_receipts_required,
            common_sources.clone(),
            "every plugin manifest must declare evidence settings for input digest, output digest, and receipt emission.",
        ),
        manifest_field_row(
            "publication_posture",
            "required_release_scope_key",
            operator_internal_only_posture,
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(PLUGIN_CHARTER_REPORT_REF),
                String::from(INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF),
            ],
            "every plugin manifest must declare publication posture explicitly; the current posture remains operator/internal-only with no default served lane.",
        ),
    ]
}

fn manifest_field_row(
    field_id: &str,
    current_posture: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticlePluginManifestFieldRow {
    TassadarPostArticlePluginManifestFieldRow {
        field_id: String::from(field_id),
        current_posture: String::from(current_posture),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn build_invocation_identity_rows(
    charter: &PluginCharterFixture,
    package_manager: &InternalComputePackageManagerFixture,
    route_policy: &InternalComputePackageRoutePolicyFixture,
) -> Vec<TassadarPostArticlePluginInvocationIdentityRow> {
    vec![
        TassadarPostArticlePluginInvocationIdentityRow {
            identity_id: String::from("canonical_plugin_identity_fields"),
            required_fields: vec![
                String::from("plugin_id"),
                String::from("plugin_version"),
                String::from("artifact_digest"),
            ],
            current_posture: String::from("required_for_all_plugin_artifacts"),
            green: !package_manager.public_package_ids.is_empty(),
            source_refs: vec![String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF)],
            detail: String::from(
                "a plugin artifact is not canonically attributable without `plugin_id`, `plugin_version`, and `artifact_digest`.",
            ),
        },
        TassadarPostArticlePluginInvocationIdentityRow {
            identity_id: String::from("canonical_invocation_identity_fields"),
            required_fields: vec![
                String::from("plugin_id"),
                String::from("plugin_version"),
                String::from("artifact_digest"),
                String::from("export_name"),
                String::from("packet_abi_version"),
                String::from("mount_envelope_identity"),
            ],
            current_posture: String::from("required_for_every_invocation"),
            green: charter.host_executes_but_does_not_decide
                && route_policy.overall_green
                && !package_manager.portability_envelope_ids.is_empty(),
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(PLUGIN_CHARTER_REPORT_REF),
                String::from(INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF),
            ],
            detail: String::from(
                "a plugin invocation is not validly attributable without plugin id/version/digest plus export name, packet ABI version, and mount-envelope identity.",
            ),
        },
        TassadarPostArticlePluginInvocationIdentityRow {
            identity_id: String::from("linked_bundle_member_identity_fields"),
            required_fields: vec![
                String::from("plugin_id"),
                String::from("plugin_version"),
                String::from("artifact_digest"),
                String::from("linked_bundle_member_refs"),
            ],
            current_posture: String::from("required_when_packaging_kind_is_linked_bundle"),
            green: package_manager
                .package_entries
                .iter()
                .any(|entry| entry.package_id == "package.verifier_search_stack.v1" && entry.module_refs.len() == 2),
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF),
            ],
            detail: String::from(
                "linked multi-module plugins must keep member refs explicit rather than collapsing bundle composition behind one opaque module label.",
            ),
        },
    ]
}

fn build_hot_swap_rule_rows(
    charter: &PluginCharterFixture,
    trust: &ModuleTrustIsolationFixture,
    promotion: &ModulePromotionStateFixture,
    route_policy: &InternalComputePackageRoutePolicyFixture,
) -> Vec<TassadarPostArticlePluginHotSwapRuleRow> {
    vec![
        TassadarPostArticlePluginHotSwapRuleRow {
            rule_id: String::from("same_plugin_id_versioned_replacement_only"),
            current_posture: String::from("version_key_required_for_hot_swap"),
            green: charter.governance_receipts_required,
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(PLUGIN_CHARTER_REPORT_REF),
            ],
            detail: String::from(
                "hot-swap is explicit only when one stable `plugin_id` carries an explicit `plugin_version` transition; silent in-place replacement is out of contract.",
            ),
        },
        TassadarPostArticlePluginHotSwapRuleRow {
            rule_id: String::from("abi_and_schema_shape_compatibility_required"),
            current_posture: String::from("typed_compatibility_or_blocked"),
            green: route_policy.refused_case_count >= 1,
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF),
            ],
            detail: String::from(
                "ABI-version and schema-shape drift must resolve through typed compatibility checks, downgrade, or blocked posture rather than silent host translation.",
            ),
        },
        TassadarPostArticlePluginHotSwapRuleRow {
            rule_id: String::from("trust_posture_widening_requires_receipts"),
            current_posture: String::from("challenge_and_trust_receipts_required"),
            green: trust.refused_case_count >= 1
                && trust.privilege_escalation_refusal_count >= 1
                && promotion.challenge_open_count >= 1,
            source_refs: vec![
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
            ],
            detail: String::from(
                "trust posture may not widen through hot-swap without explicit trust and promotion receipts.",
            ),
        },
        TassadarPostArticlePluginHotSwapRuleRow {
            rule_id: String::from("replay_and_evidence_posture_compatibility_required"),
            current_posture: String::from("receipt_family_must_stay_compatible"),
            green: charter.governance_receipts_required,
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(PLUGIN_CHARTER_REPORT_REF),
            ],
            detail: String::from(
                "replay class and evidence settings are part of hot-swap compatibility; a host may not change them without explicit manifest transition and typed receipts.",
            ),
        },
    ]
}

fn build_packaging_rows(
    _package_manager: &InternalComputePackageManagerFixture,
    route_policy: &InternalComputePackageRoutePolicyFixture,
    single_module_packages_explicit: bool,
    linked_bundle_identity_explicit: bool,
) -> Vec<TassadarPostArticlePluginPackagingRow> {
    vec![
        TassadarPostArticlePluginPackagingRow {
            packaging_id: String::from("single_wasm_module_packaging_explicit"),
            current_posture: String::from("single_module_plugins_allowed_when_manifest_complete"),
            green: single_module_packages_explicit,
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF),
            ],
            detail: String::from(
                "single-module packaging remains explicit and named rather than anonymous loose binaries.",
            ),
        },
        TassadarPostArticlePluginPackagingRow {
            packaging_id: String::from("linked_multi_module_packaging_explicit"),
            current_posture: String::from("linked_bundle_members_must_be_named"),
            green: linked_bundle_identity_explicit,
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF),
            ],
            detail: String::from(
                "when one plugin is a linked bundle, member modules remain explicit instead of being hidden inside one opaque artifact label.",
            ),
        },
        TassadarPostArticlePluginPackagingRow {
            packaging_id: String::from("future_component_model_bundle_requires_explicit_profile"),
            current_posture: String::from("blocked_until_later_profile_issue"),
            green: route_policy.default_served_package_ids.is_empty(),
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF),
            ],
            detail: String::from(
                "future component-model bundles require an explicit admitted profile; they are not silently admitted by this manifest contract.",
            ),
        },
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    charter: &PluginCharterFixture,
    trust: &ModuleTrustIsolationFixture,
    promotion: &ModulePromotionStateFixture,
    _package_manager: &InternalComputePackageManagerFixture,
    route_policy: &InternalComputePackageRoutePolicyFixture,
    multi_module_packaging_explicit: bool,
    linked_bundle_identity_explicit: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
) -> Vec<TassadarPostArticlePluginManifestValidationRow> {
    vec![
        TassadarPostArticlePluginManifestValidationRow {
            validation_id: String::from("hidden_host_orchestration_blocked"),
            green: charter.host_executes_but_does_not_decide,
            source_refs: vec![String::from(PLUGIN_CHARTER_REPORT_REF)],
            detail: String::from(
                "the host may project manifest selection and hot-swap mechanics, but it may not secretly decide workflow.",
            ),
        },
        TassadarPostArticlePluginManifestValidationRow {
            validation_id: String::from("schema_drift_posture_blocked"),
            green: route_policy.refused_case_count >= 1,
            source_refs: vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF),
            ],
            detail: String::from(
                "schema or ABI drift remains typed and fail-closed rather than silently mediated by the host runtime.",
            ),
        },
        TassadarPostArticlePluginManifestValidationRow {
            validation_id: String::from("envelope_leakage_posture_blocked"),
            green: trust.mount_policy_refusal_count >= 1
                && trust
                    .world_mount_dependency_marker
                    .contains("outside standalone psionic"),
            source_refs: vec![String::from(MODULE_TRUST_ISOLATION_REPORT_REF)],
            detail: String::from(
                "manifest identity does not leak envelope authority into the host; task-scoped module authority remains explicit and external where required.",
            ),
        },
        TassadarPostArticlePluginManifestValidationRow {
            validation_id: String::from("side_channel_posture_blocked"),
            green: charter.resource_transparency_frozen && charter.scheduling_ownership_frozen,
            source_refs: vec![String::from(PLUGIN_CHARTER_REPORT_REF)],
            detail: String::from(
                "cost, latency, and scheduling signals remain explicit or fixed by contract rather than hidden hot-swap steering channels.",
            ),
        },
        TassadarPostArticlePluginManifestValidationRow {
            validation_id: String::from("overclaim_posture_blocked"),
            green: rebase_claim_allowed
                && !plugin_capability_claim_allowed
                && !weighted_plugin_control_allowed
                && !plugin_publication_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            source_refs: vec![String::from(PLUGIN_CHARTER_REPORT_REF)],
            detail: String::from(
                "the manifest contract does not itself imply weighted plugin capability, public plugin publication, served/public universality, or arbitrary software capability.",
            ),
        },
        TassadarPostArticlePluginManifestValidationRow {
            validation_id: String::from("typed_fail_closed_posture_explicit"),
            green: route_policy.refused_case_count == 3
                && promotion.quarantined_count >= 1
                && trust.refused_case_count >= 1,
            source_refs: vec![
                String::from(INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF),
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
            ],
            detail: String::from(
                "suppression, quarantine, downgrade, and blocked posture remain explicit whenever manifest, trust, or promotion prerequisites are missing.",
            ),
        },
        TassadarPostArticlePluginManifestValidationRow {
            validation_id: String::from("linked_bundle_members_explicit"),
            green: multi_module_packaging_explicit && linked_bundle_identity_explicit,
            source_refs: vec![String::from(INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF)],
            detail: String::from(
                "linked bundle members remain named and machine-readable instead of being hidden inside one opaque plugin package.",
            ),
        },
        TassadarPostArticlePluginManifestValidationRow {
            validation_id: String::from("default_served_lane_remains_empty"),
            green: route_policy.default_served_package_ids.is_empty(),
            source_refs: vec![String::from(INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF)],
            detail: String::from(
                "the current manifest contract keeps the default served plugin lane empty rather than widening operator/internal artifacts into a public platform.",
            ),
        },
    ]
}

#[must_use]
pub fn tassadar_post_article_plugin_manifest_identity_contract_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF)
}

pub fn write_tassadar_post_article_plugin_manifest_identity_contract_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginManifestIdentityContractReport,
    TassadarPostArticlePluginManifestIdentityContractReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginManifestIdentityContractReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_plugin_manifest_identity_contract_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginManifestIdentityContractReportError::Write {
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
        .expect("repo root should resolve from psionic-catalog crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarPostArticlePluginManifestIdentityContractReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginManifestIdentityContractReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginManifestIdentityContractReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct PluginCharterFixture {
    report_id: String,
    report_digest: String,
    machine_identity_binding: PluginCharterMachineIdentityFixture,
    current_publication_posture: String,
    internal_only_plugin_posture: bool,
    host_executes_but_does_not_decide: bool,
    resource_transparency_frozen: bool,
    scheduling_ownership_frozen: bool,
    plugin_language_boundary_frozen: bool,
    governance_receipts_required: bool,
    charter_green: bool,
    rebase_claim_allowed: bool,
    plugin_publication_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct PluginCharterMachineIdentityFixture {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
    computational_model_statement_id: String,
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
    world_mount_dependency_marker: String,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct InternalComputePackageManagerFixture {
    report_id: String,
    report_digest: String,
    package_entries: Vec<InternalComputePackageEntryFixture>,
    exact_case_count: u32,
    refusal_case_count: u32,
    public_package_ids: Vec<String>,
    portability_envelope_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct InternalComputePackageEntryFixture {
    package_id: String,
    module_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct InternalComputePackageRoutePolicyFixture {
    report_id: String,
    report_digest: String,
    overall_green: bool,
    selected_case_count: u32,
    refused_case_count: u32,
    default_served_package_ids: Vec<String>,
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticlePluginManifestIdentityContractReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticlePluginManifestIdentityContractReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginManifestIdentityContractReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_plugin_manifest_identity_contract_report, read_json,
        tassadar_post_article_plugin_manifest_identity_contract_report_path,
        write_tassadar_post_article_plugin_manifest_identity_contract_report,
        TassadarPostArticlePluginManifestIdentityContractReport,
        TassadarPostArticlePluginManifestIdentityContractStatus,
        TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn post_article_plugin_manifest_contract_freezes_identity_and_hot_swap_rules() {
        let report =
            build_tassadar_post_article_plugin_manifest_identity_contract_report().expect("report");

        assert_eq!(
            report.contract_status,
            TassadarPostArticlePluginManifestIdentityContractStatus::Green
        );
        assert!(report.contract_green);
        assert!(report.operator_internal_only_posture);
        assert!(report.manifest_fields_frozen);
        assert!(report.canonical_invocation_identity_frozen);
        assert!(report.hot_swap_rules_frozen);
        assert!(report.multi_module_packaging_explicit);
        assert!(report.linked_bundle_identity_explicit);
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
        assert_eq!(report.dependency_rows.len(), 6);
        assert_eq!(report.manifest_field_rows.len(), 12);
        assert_eq!(report.invocation_identity_rows.len(), 3);
        assert_eq!(report.hot_swap_rule_rows.len(), 4);
        assert_eq!(report.packaging_rows.len(), 3);
        assert_eq!(report.validation_rows.len(), 8);
        assert!(report.deferred_issue_ids.is_empty());
    }

    #[test]
    fn post_article_plugin_manifest_contract_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_manifest_identity_contract_report().expect("report");
        let committed: TassadarPostArticlePluginManifestIdentityContractReport = read_json(
            tassadar_post_article_plugin_manifest_identity_contract_report_path(),
        )
        .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_plugin_manifest_identity_contract_report.json"
        );
    }

    #[test]
    fn write_post_article_plugin_manifest_contract_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_plugin_manifest_identity_contract_report.json");
        let written = write_tassadar_post_article_plugin_manifest_identity_contract_report(
            &output_path,
        )
        .expect("write report");
        let persisted: TassadarPostArticlePluginManifestIdentityContractReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
