use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_CHECKER_REF:
    &str =
    "scripts/check-tassadar-post-article-plugin-authority-promotion-publication-and-trust-tier-gate.sh";

const CONTROLLER_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report.json";
const MANIFEST_CONTRACT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_manifest_identity_contract_report.json";
const MODULE_PROMOTION_STATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_promotion_state_report.json";
const MODULE_TRUST_ISOLATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_trust_isolation_report.json";
const BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_internal_compute_profile_publication_report.json";
const BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_internal_compute_route_policy_report.json";
const CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_closure_bundle_report.json";
const STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/tassadar_post_article_starter_plugin_catalog_bundle.json";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const PLUGIN_SYSTEM_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const CANONICAL_ARCHITECTURE_BOUNDARY_REF: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const CANONICAL_ARCHITECTURE_ANCHOR_CRATE: &str = "psionic-transformer";

const ARTICLE_CLOSEOUT_PROFILE_ID: &str = "tassadar.internal_compute.article_closeout.v1";
const DETERMINISTIC_IMPORT_PROFILE_ID: &str =
    "tassadar.internal_compute.deterministic_import_subset.v1";
const RUNTIME_SUPPORT_PROFILE_ID: &str = "tassadar.internal_compute.runtime_support_subset.v1";
const PORTABLE_BROAD_PROFILE_ID: &str = "tassadar.internal_compute.portable_broad_family.v1";
const PUBLIC_BROAD_PROFILE_ID: &str = "tassadar.internal_compute.public_broad_family.v1";

const ROUTE_DETERMINISTIC_IMPORT: &str = "route.deterministic_import.subset";
const ROUTE_RUNTIME_SUPPORT: &str = "route.runtime_support.linked_bundle";
const ROUTE_PORTABLE_BROAD: &str = "route.portable_broad_family.declared_matrix";
const ROUTE_PUBLIC_BROAD: &str = "route.public_broad_family.publication";
const GUEST_ARTIFACT_PLUGIN_ID: &str = "plugin.example.echo_guest";
const GUEST_ARTIFACT_REPLAY_CLASS_ID: &str = "guest_artifact_digest_replay_only.v1";
const GUEST_ARTIFACT_TRUST_TIER_ID: &str =
    "operator_reviewed_guest_artifact_digest_bound_internal_only";
const GUEST_ARTIFACT_EVIDENCE_POSTURE_ID: &str =
    "evidence.manifest_digest_invocation_receipt_bound.v1";
const GUEST_ARTIFACT_PUBLICATION_NOT_CLAIMED_NEGATIVE_CLAIM_ID: &str =
    "guest_artifact_publication_not_claimed";
const GUEST_ARTIFACT_ARBITRARY_WASM_NOT_CLAIMED_NEGATIVE_CLAIM_ID: &str =
    "arbitrary_wasm_plugin_support_not_claimed";
const GUEST_ARTIFACT_SECRET_OR_STATEFUL_NOT_CLAIMED_NEGATIVE_CLAIM_ID: &str =
    "secret_or_stateful_guest_capability_not_claimed";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginAuthorityDependencyClass {
    ControllerPrecedent,
    CatalogPrecedent,
    PromotionDependency,
    TrustDependency,
    PublicationDependency,
    RoutePolicyDependency,
    ClosureBundle,
    DesignInput,
    AuditInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAuthorityMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub controller_eval_report_id: String,
    pub controller_eval_report_digest: String,
    pub control_trace_contract_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub manifest_contract_report_id: String,
    pub manifest_contract_report_digest: String,
    pub canonical_architecture_anchor_crate: String,
    pub canonical_architecture_boundary_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAuthorityDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticlePluginAuthorityDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAuthorityTrustTierRow {
    pub tier_id: String,
    pub current_posture: String,
    pub benchmark_bar: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAuthorityPromotionRow {
    pub authority_id: String,
    pub lifecycle_state: String,
    pub receipt_required: bool,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAuthorityPublicationPostureRow {
    pub posture_id: String,
    pub publication_posture: String,
    pub publication_status: String,
    pub validator_hook_status: String,
    pub accepted_outcome_hook_status: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAuthorityObserverRow {
    pub observer_id: String,
    pub permitted_actions: Vec<String>,
    pub publication_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAuthorityValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub controller_eval_report_ref: String,
    pub manifest_contract_report_ref: String,
    pub module_promotion_state_report_ref: String,
    pub module_trust_isolation_report_ref: String,
    pub broad_internal_compute_profile_publication_report_ref: String,
    pub broad_internal_compute_route_policy_report_ref: String,
    pub canonical_machine_closure_bundle_report_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding: TassadarPostArticlePluginAuthorityMachineIdentityBinding,
    pub dependency_rows: Vec<TassadarPostArticlePluginAuthorityDependencyRow>,
    pub trust_tier_rows: Vec<TassadarPostArticlePluginAuthorityTrustTierRow>,
    pub promotion_rows: Vec<TassadarPostArticlePluginAuthorityPromotionRow>,
    pub publication_posture_rows: Vec<TassadarPostArticlePluginAuthorityPublicationPostureRow>,
    pub observer_rows: Vec<TassadarPostArticlePluginAuthorityObserverRow>,
    pub validation_rows: Vec<TassadarPostArticlePluginAuthorityValidationRow>,
    pub contract_status:
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateStatus,
    pub contract_green: bool,
    pub trust_tier_gate_green: bool,
    pub promotion_receipts_explicit: bool,
    pub publication_posture_explicit: bool,
    pub observer_rights_explicit: bool,
    pub validator_hooks_explicit: bool,
    pub accepted_outcome_hooks_explicit: bool,
    pub operator_internal_only_posture: bool,
    pub profile_specific_named_routes_explicit: bool,
    pub broader_publication_refused: bool,
    pub closure_bundle_bound_by_digest: bool,
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
pub enum TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError {
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

pub fn build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report(
) -> Result<
    TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReport,
    TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError,
> {
    let controller: ControllerEvalFixture = read_repo_json(CONTROLLER_EVAL_REPORT_REF)?;
    let manifest: ManifestContractFixture = read_repo_json(MANIFEST_CONTRACT_REPORT_REF)?;
    let promotion: ModulePromotionStateFixture = read_repo_json(MODULE_PROMOTION_STATE_REPORT_REF)?;
    let trust: ModuleTrustIsolationFixture = read_repo_json(MODULE_TRUST_ISOLATION_REPORT_REF)?;
    let publication: BroadInternalComputeProfilePublicationFixture =
        read_repo_json(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF)?;
    let route_policy: BroadInternalComputeRoutePolicyFixture =
        read_repo_json(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF)?;
    let closure_bundle: CanonicalMachineClosureBundleFixture =
        read_repo_json(CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF)?;
    let starter_catalog_runtime: StarterPluginCatalogRuntimeBundleFixture =
        read_repo_json(STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF)?;

    let operator_internal_only_posture = controller.operator_internal_only_posture
        && manifest.operator_internal_only_posture
        && publication.current_served_profile_id == ARTICLE_CLOSEOUT_PROFILE_ID
        && publication.published_profile_ids == [String::from(ARTICLE_CLOSEOUT_PROFILE_ID)]
        && !controller.plugin_capability_claim_allowed
        && !controller.plugin_publication_allowed
        && starter_catalog_runtime.operator_only_posture
        && !starter_catalog_runtime.public_marketplace_implication_allowed;
    let rebase_claim_allowed = controller.rebase_claim_allowed && manifest.rebase_claim_allowed;
    let weighted_plugin_control_allowed = controller.weighted_plugin_control_allowed;
    let plugin_capability_claim_allowed = false;
    let plugin_publication_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;

    let deterministic_import_publication =
        publication_profile_row(&publication, DETERMINISTIC_IMPORT_PROFILE_ID);
    let runtime_support_publication =
        publication_profile_row(&publication, RUNTIME_SUPPORT_PROFILE_ID);
    let portable_broad_publication =
        publication_profile_row(&publication, PORTABLE_BROAD_PROFILE_ID);
    let public_broad_publication = publication_profile_row(&publication, PUBLIC_BROAD_PROFILE_ID);
    let deterministic_import_route = route_policy_row(&route_policy, ROUTE_DETERMINISTIC_IMPORT);
    let runtime_support_route = route_policy_row(&route_policy, ROUTE_RUNTIME_SUPPORT);
    let portable_broad_route = route_policy_row(&route_policy, ROUTE_PORTABLE_BROAD);
    let public_broad_route = route_policy_row(&route_policy, ROUTE_PUBLIC_BROAD);

    let profile_specific_named_routes_explicit =
        profile_specific_route_green(deterministic_import_publication, deterministic_import_route)
            && profile_specific_route_green(runtime_support_publication, runtime_support_route)
            && route_policy.promoted_profile_specific_route_count >= 2;
    let broader_publication_refused =
        suppressed_public_broad_green(public_broad_publication, public_broad_route)
            && refused_portable_broad_green(portable_broad_publication, portable_broad_route)
            && publication
                .suppressed_profile_ids
                .contains(&String::from(PUBLIC_BROAD_PROFILE_ID))
            && publication
                .failed_profile_ids
                .contains(&String::from(PORTABLE_BROAD_PROFILE_ID))
            && route_policy.suppressed_route_count >= 1
            && route_policy.refused_route_count >= 1;
    let validator_hooks_explicit = profile_specific_named_routes_explicit;
    let accepted_outcome_hooks_explicit = profile_specific_named_routes_explicit;
    let guest_artifact_trust_tier_green =
        guest_artifact_trust_tier_green(&starter_catalog_runtime);
    let guest_artifact_publication_blocked =
        guest_artifact_publication_blocked(&starter_catalog_runtime)
            && !plugin_publication_allowed
            && broader_publication_refused;

    let machine_identity_binding = TassadarPostArticlePluginAuthorityMachineIdentityBinding {
        machine_identity_id: controller
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: controller.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: controller.machine_identity_binding.canonical_route_id.clone(),
        canonical_route_descriptor_digest: controller
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: controller
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: controller
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: controller
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: controller
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: controller
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        controller_eval_report_id: controller.report_id.clone(),
        controller_eval_report_digest: controller.report_digest.clone(),
        control_trace_contract_id: controller
            .machine_identity_binding
            .control_trace_contract_id
            .clone(),
        closure_bundle_report_id: closure_bundle.report_id.clone(),
        closure_bundle_report_digest: closure_bundle.report_digest.clone(),
        closure_bundle_digest: closure_bundle.closure_bundle_digest.clone(),
        manifest_contract_report_id: manifest.report_id.clone(),
        manifest_contract_report_digest: manifest.report_digest.clone(),
        canonical_architecture_anchor_crate: String::from(CANONICAL_ARCHITECTURE_ANCHOR_CRATE),
        canonical_architecture_boundary_ref: String::from(CANONICAL_ARCHITECTURE_BOUNDARY_REF),
        detail: format!(
            "machine_identity_id=`{}` canonical_route_id=`{}` controller_eval_report_id=`{}` manifest_contract_report_id=`{}` and closure_bundle_digest=`{}` remain the authority-gate anchor in `{}`.",
            controller.machine_identity_binding.machine_identity_id,
            controller.machine_identity_binding.canonical_route_id,
            controller.report_id,
            manifest.report_id,
            closure_bundle.closure_bundle_digest,
            CANONICAL_ARCHITECTURE_ANCHOR_CRATE,
        ),
    };
    let closure_bundle_bound_by_digest = closure_bundle.bundle_green
        && closure_bundle.closure_subject.machine_identity_id
            == machine_identity_binding.machine_identity_id
        && closure_bundle.closure_subject.canonical_route_id
            == machine_identity_binding.canonical_route_id
        && machine_identity_binding.closure_bundle_report_id == closure_bundle.report_id
        && machine_identity_binding.closure_bundle_report_digest == closure_bundle.report_digest
        && machine_identity_binding.closure_bundle_digest == closure_bundle.closure_bundle_digest;

    let dependency_rows = vec![
        dependency_row(
            "controller_eval_green",
            TassadarPostArticlePluginAuthorityDependencyClass::ControllerPrecedent,
            controller.contract_green
                && controller.control_trace_contract_green
                && controller.determinism_profile_explicit
                && controller.typed_refusal_loop_closed
                && controller.host_not_planner_green
                && controller.adversarial_negative_rows_green
                && controller.closure_bundle_bound_by_digest
                && controller.weighted_plugin_control_allowed
                && controller.deferred_issue_ids.is_empty(),
            CONTROLLER_EVAL_REPORT_REF,
            Some(controller.report_id.clone()),
            Some(controller.report_digest.clone()),
            "the inherited controller proof must already be green, keep weighted plugin control explicit, and clear its TAS-204 frontier before the authority gate can rely on it.",
        ),
        dependency_row(
            "manifest_contract_green",
            TassadarPostArticlePluginAuthorityDependencyClass::CatalogPrecedent,
            manifest.contract_green
                && manifest.manifest_fields_frozen
                && manifest.canonical_invocation_identity_frozen
                && manifest.hot_swap_rules_frozen
                && manifest.multi_module_packaging_explicit
                && manifest.linked_bundle_identity_explicit
                && manifest.deferred_issue_ids.is_empty(),
            MANIFEST_CONTRACT_REPORT_REF,
            Some(manifest.report_id.clone()),
            Some(manifest.report_digest.clone()),
            "the manifest and invocation identity contract must already be green so authority widening cannot bypass canonical declaration, packaging, or hot-swap rules.",
        ),
        dependency_row(
            "canonical_machine_closure_bundle_published",
            TassadarPostArticlePluginAuthorityDependencyClass::ClosureBundle,
            closure_bundle_bound_by_digest,
            CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
            Some(closure_bundle.report_id.clone()),
            Some(closure_bundle.report_digest.clone()),
            "the authority and publication claim must inherit the canonical machine closure bundle by report id and digest instead of reconstructing machine identity from the controller and manifest pair alone.",
        ),
        dependency_row(
            "promotion_receipts_present",
            TassadarPostArticlePluginAuthorityDependencyClass::PromotionDependency,
            promotion.active_promoted_count >= 1
                && promotion.challenge_open_count >= 1
                && promotion.quarantined_count >= 1
                && promotion.revoked_count >= 1
                && promotion.superseded_count >= 1,
            MODULE_PROMOTION_STATE_REPORT_REF,
            Some(promotion.report_id.clone()),
            Some(promotion.report_digest.clone()),
            "promotion, challenge, quarantine, revocation, and supersession must all remain explicit instead of collapsing promotion into one irreversible success flag.",
        ),
        dependency_row(
            "trust_tiers_present",
            TassadarPostArticlePluginAuthorityDependencyClass::TrustDependency,
            has_trust_bundle(
                &trust,
                "scratchpad_probe_research@0.1.0",
                "research_only",
                "research_contained",
                1,
            ) && has_trust_bundle(
                &trust,
                "frontier_relax_core@1.0.0",
                "benchmark_gated_internal",
                "benchmark_internal_only",
                2,
            ) && has_trust_bundle(
                &trust,
                "candidate_select_core@1.1.0",
                "challenge_gated_install",
                "challenge_gated_install",
                2,
            ) && guest_artifact_trust_tier_green
                && trust.cross_tier_refusal_count >= 1
                && trust.privilege_escalation_refusal_count >= 1
                && trust.mount_policy_refusal_count >= 1,
            MODULE_TRUST_ISOLATION_REPORT_REF,
            Some(trust.report_id.clone()),
            Some(trust.report_digest.clone()),
            "the authority gate must keep research-only, benchmark-gated, challenge-gated, and the one admitted digest-bound guest-artifact tier distinct while still refusing cross-tier escalation and mount-policy leakage.",
        ),
        dependency_row(
            "publication_report_present",
            TassadarPostArticlePluginAuthorityDependencyClass::PublicationDependency,
            publication.current_served_profile_id == ARTICLE_CLOSEOUT_PROFILE_ID
                && publication
                    .published_profile_ids
                    .contains(&String::from(ARTICLE_CLOSEOUT_PROFILE_ID))
                && publication
                    .suppressed_profile_ids
                    .contains(&String::from(DETERMINISTIC_IMPORT_PROFILE_ID))
                && publication
                    .suppressed_profile_ids
                    .contains(&String::from(RUNTIME_SUPPORT_PROFILE_ID)),
            BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
            Some(publication.report_id.clone()),
            Some(publication.report_digest.clone()),
            "the broader publication report must keep the current served lane article-only while exposing explicit suppressed profile-specific routes for bounded widening.",
        ),
        dependency_row(
            "route_policy_present",
            TassadarPostArticlePluginAuthorityDependencyClass::RoutePolicyDependency,
            route_policy.current_served_profile_id == ARTICLE_CLOSEOUT_PROFILE_ID
                && route_policy.selected_route_count >= 1
                && route_policy.promoted_profile_specific_route_count >= 2
                && profile_specific_named_routes_explicit
                && broader_publication_refused,
            BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF,
            Some(route_policy.report_id.clone()),
            Some(route_policy.report_digest.clone()),
            "the broader route-policy report must keep profile-specific widening and broader public refusal machine-readable instead of implying one default public plugin lane.",
        ),
        dependency_row(
            "plugin_system_spec_cited",
            TassadarPostArticlePluginAuthorityDependencyClass::DesignInput,
            !LOCAL_PLUGIN_SYSTEM_SPEC_REF.trim().is_empty(),
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the local plugin-system spec remains a cited design input for the authority and trust-tier gate.",
        ),
        dependency_row(
            "plugin_system_audit_cited",
            TassadarPostArticlePluginAuthorityDependencyClass::AuditInput,
            !PLUGIN_SYSTEM_AUDIT_REF.trim().is_empty(),
            PLUGIN_SYSTEM_AUDIT_REF,
            None,
            None,
            "the public plugin-system audit remains a cited audit input so proof-carrying artifacts stay distinct from narrative review.",
        ),
    ];

    let trust_tier_rows = vec![
        trust_tier_row(
            "research_only_contained",
            "research_only",
            "1 benchmark-equivalent seeded evidence row and zero authority widening",
            has_trust_bundle(
                &trust,
                "scratchpad_probe_research@0.1.0",
                "research_only",
                "research_contained",
                1,
            ),
            vec![String::from(MODULE_TRUST_ISOLATION_REPORT_REF)],
            "research-only plugin experiments stay contained and may not silently promote into benchmark or install-grade authority.",
        ),
        trust_tier_row(
            "benchmark_gated_internal_requires_two_benchmarks",
            "benchmark_gated_internal",
            "at least 2 benchmark refs and benchmark-only isolation",
            has_trust_bundle(
                &trust,
                "frontier_relax_core@1.0.0",
                "benchmark_gated_internal",
                "benchmark_internal_only",
                2,
            ) && has_trust_bundle(
                &trust,
                "checkpoint_backtrack_core@1.0.0",
                "benchmark_gated_internal",
                "benchmark_internal_only",
                2,
            ),
            vec![String::from(MODULE_TRUST_ISOLATION_REPORT_REF)],
            "benchmark-gated plugins require explicit two-benchmark posture and remain isolated from research-only and challenge-gated mounts unless separately declared.",
        ),
        trust_tier_row(
            "challenge_gated_install_requires_explicit_mount",
            "challenge_gated_install",
            "at least 2 benchmark refs plus challenge-aware mount posture",
            has_trust_bundle(
                &trust,
                "candidate_select_core@1.1.0",
                "challenge_gated_install",
                "challenge_gated_install",
                2,
            ),
            vec![
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
            ],
            "challenge-gated installs remain bounded to explicit mounts and challenge windows rather than inheriting broad publication authority.",
        ),
        trust_tier_row(
            "guest_artifact_digest_bound_internal_only",
            "operator_reviewed_guest_artifact_digest_bound_internal_only",
            "digest-bound manifest, explicit publication-negative claims, operator-reviewed internal-only posture",
            guest_artifact_trust_tier_green,
            vec![
                String::from(STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF),
                String::from(MANIFEST_CONTRACT_REPORT_REF),
            ],
            "the admitted guest-artifact row stays digest-bound, operator-reviewed, and publication-blocked instead of inheriting host-native trust or publication posture.",
        ),
        trust_tier_row(
            "cross_tier_escalation_refusals_explicit",
            "refusal_surface",
            "cross-tier, privilege-escalation, and mount-policy refusals are all present",
            trust.refused_case_count >= 3
                && trust.cross_tier_refusal_count >= 1
                && trust.privilege_escalation_refusal_count >= 1
                && trust.mount_policy_refusal_count >= 1,
            vec![String::from(MODULE_TRUST_ISOLATION_REPORT_REF)],
            "cross-tier composition, privilege escalation, and mount-policy bypass remain typed refusals instead of implicit host-side policy.",
        ),
    ];
    let trust_tier_gate_green = trust_tier_rows.iter().all(|row| row.green);

    let promotion_rows = vec![
        promotion_row(
            "active_promotion_receipt",
            "active_promoted",
            true,
            promotion.active_promoted_count >= 1,
            vec![String::from(MODULE_PROMOTION_STATE_REPORT_REF)],
            "at least one active promoted record remains explicit and challengeable rather than silently becoming irreversible closure.",
        ),
        promotion_row(
            "challenge_window_receipt",
            "challenge_open",
            true,
            promotion.challenge_open_count >= 1,
            vec![String::from(MODULE_PROMOTION_STATE_REPORT_REF)],
            "promotion remains challenge-open where declared instead of hiding the review window behind one benchmark pass.",
        ),
        promotion_row(
            "quarantine_receipt",
            "quarantined",
            true,
            promotion.quarantined_count >= 1,
            vec![String::from(MODULE_PROMOTION_STATE_REPORT_REF)],
            "missing evidence still produces explicit quarantine posture instead of optimistic publication.",
        ),
        promotion_row(
            "revocation_receipt",
            "revoked",
            true,
            promotion.revoked_count >= 1,
            vec![String::from(MODULE_PROMOTION_STATE_REPORT_REF)],
            "stale or invalid evidence still yields a revocation receipt instead of retroactively erasing prior lineage.",
        ),
        promotion_row(
            "supersession_receipt",
            "superseded",
            true,
            promotion.superseded_count >= 1,
            vec![String::from(MODULE_PROMOTION_STATE_REPORT_REF)],
            "reinstall and supersession remain explicit so upgraded plugin posture cannot smuggle authority through ambiguous lineage.",
        ),
    ];
    let promotion_receipts_explicit = promotion_rows.iter().all(|row| row.green);

    let publication_posture_rows = vec![
        publication_posture_row(
            "operator_internal_only_current",
            "operator_internal_only",
            "article_closeout_only",
            "not_applicable",
            "not_applicable",
            operator_internal_only_posture,
            vec![
                String::from(CONTROLLER_EVAL_REPORT_REF),
                String::from(MANIFEST_CONTRACT_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
            ],
            "the current served posture remains article-only and operator/internal for plugin authority; no public plugin lane is implied by weighted control alone.",
        ),
        publication_posture_row(
            "guest_artifact_digest_bound_publication_blocked",
            "guest_artifact_digest_bound_internal_only",
            "blocked",
            if guest_artifact_trust_tier_green {
                GUEST_ARTIFACT_EVIDENCE_POSTURE_ID
            } else {
                "missing"
            },
            if guest_artifact_publication_blocked {
                "depublication_review_required"
            } else {
                "missing"
            },
            guest_artifact_publication_blocked,
            vec![
                String::from(STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF),
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            "the digest-bound guest-artifact class stays explicitly publication-blocked and operator-internal until a later tranche proves any broader lane.",
        ),
        publication_posture_row(
            "deterministic_import_profile_specific_template",
            "profile_specific_named_but_suppressed",
            "suppressed",
            deterministic_import_route
                .map(|row| row.world_mount_binding_status.as_str())
                .unwrap_or("missing"),
            deterministic_import_route
                .map(|row| row.accepted_outcome_binding_status.as_str())
                .unwrap_or("missing"),
            profile_specific_route_green(
                deterministic_import_publication,
                deterministic_import_route,
            ),
            vec![
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            "the deterministic-import subset is explicitly nameable only through a suppressed profile-specific lane with mount and accepted-outcome templates kept explicit.",
        ),
        publication_posture_row(
            "runtime_support_profile_specific_template",
            "profile_specific_named_but_suppressed",
            "suppressed",
            runtime_support_route
                .map(|row| row.world_mount_binding_status.as_str())
                .unwrap_or("missing"),
            runtime_support_route
                .map(|row| row.accepted_outcome_binding_status.as_str())
                .unwrap_or("missing"),
            profile_specific_route_green(runtime_support_publication, runtime_support_route),
            vec![
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            "the runtime-support subset is explicitly nameable only through a suppressed profile-specific lane with validator and accepted-outcome hooks kept machine-readable.",
        ),
        publication_posture_row(
            "public_broad_publication_suppressed",
            "served_public_plugin_lane",
            "suppressed",
            public_broad_route
                .and_then(|row| row.required_runtime_support_id.as_deref())
                .unwrap_or("missing"),
            public_broad_route
                .map(|row| row.accepted_outcome_binding_status.as_str())
                .unwrap_or("missing"),
            suppressed_public_broad_green(public_broad_publication, public_broad_route),
            vec![
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            "any broader served/public plugin publication remains explicitly suppressed until later platform-closeout work lands.",
        ),
        publication_posture_row(
            "portable_broad_family_refused",
            "portable_broad_family",
            "refused",
            portable_broad_route
                .map(|row| row.world_mount_binding_status.as_str())
                .unwrap_or("missing"),
            portable_broad_route
                .map(|row| row.accepted_outcome_binding_status.as_str())
                .unwrap_or("missing"),
            refused_portable_broad_green(portable_broad_publication, portable_broad_route),
            vec![
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            "the portable broad family remains explicitly refused rather than renamed into a public plugin platform claim.",
        ),
    ];
    let publication_posture_explicit = publication_posture_rows.iter().all(|row| row.green);

    let observer_rows = vec![
        observer_row(
            "operator_internal_curator_accepts_internal_results",
            vec![
                String::from("accept_internal_conformance"),
                String::from("promote_with_receipts"),
            ],
            "operator_internal_only",
            operator_internal_only_posture && promotion.active_promoted_count >= 1,
            vec![
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
            ],
            "internal operators may accept bounded plugin conformance only when the promotion and trust receipts remain explicit.",
        ),
        observer_row(
            "challenge_observer_may_quarantine_and_revoke",
            vec![
                String::from("challenge"),
                String::from("quarantine"),
                String::from("revoke"),
            ],
            "challenge_window",
            promotion.challenge_open_count >= 1
                && promotion.quarantined_count >= 1
                && promotion.revoked_count >= 1,
            vec![String::from(MODULE_PROMOTION_STATE_REPORT_REF)],
            "challenge observers may challenge, quarantine, and revoke bounded plugin posture through explicit receipts instead of implicit host trust.",
        ),
        observer_row(
            "served_public_observer_cannot_publish_plugin_lane",
            vec![
                String::from("observe_article_only"),
                String::from("cannot_publish_plugin_conformance"),
            ],
            "served_public_suppressed",
            broader_publication_refused
                && !plugin_publication_allowed
                && publication.current_served_profile_id == ARTICLE_CLOSEOUT_PROFILE_ID,
            vec![
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            "served/public observers may see the article lane but cannot infer a published plugin lane from the bounded weighted-controller proof.",
        ),
        observer_row(
            "external_publication_owners_explicit",
            vec![
                String::from("externalize_publication_authority"),
                String::from("externalize_settlement_accepted_outcomes"),
            ],
            "externalized_outside_psionic",
            !publication.compute_market_dependency_marker.trim().is_empty()
                && !publication.nexus_dependency_marker.trim().is_empty()
                && !publication.kernel_policy_dependency_marker.trim().is_empty()
                && !trust.cluster_trust_dependency_marker.trim().is_empty()
                && !promotion.kernel_policy_dependency_marker.trim().is_empty()
                && !promotion.nexus_dependency_marker.trim().is_empty(),
            vec![
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
            ],
            "public publication, settlement-grade acceptance, and cluster/provider authority remain explicitly externalized outside standalone psionic.",
        ),
    ];
    let observer_rights_explicit = observer_rows.iter().all(|row| row.green);

    let validation_rows = vec![
        validation_row(
            "hidden_host_orchestration_negative_rows_carried",
            controller.host_not_planner_green && controller.adversarial_negative_rows_green,
            vec![String::from(CONTROLLER_EVAL_REPORT_REF)],
            "hidden host orchestration remains refused by the inherited controller proof rather than being reintroduced inside promotion or publication policy.",
        ),
        validation_row(
            "controller_frontier_cleared",
            controller.deferred_issue_ids.is_empty(),
            vec![String::from(CONTROLLER_EVAL_REPORT_REF)],
            "the TAS-204 controller artifact now clears its defer pointer so TAS-205 is the live frontier instead of keeping authority work hidden behind controller status.",
        ),
        validation_row(
            "manifest_frontier_cleared",
            manifest.deferred_issue_ids.is_empty(),
            vec![String::from(MANIFEST_CONTRACT_REPORT_REF)],
            "the earlier manifest contract has no remaining defer pointer, so authority widening cannot rely on implied future declaration work.",
        ),
        validation_row(
            "schema_and_identity_drift_refused",
            manifest.manifest_fields_frozen
                && manifest.canonical_invocation_identity_frozen
                && manifest.hot_swap_rules_frozen,
            vec![String::from(MANIFEST_CONTRACT_REPORT_REF)],
            "schema drift and invocation-identity drift remain refused through the earlier manifest and hot-swap contract.",
        ),
        validation_row(
            "promotion_quarantine_revocation_receipts_explicit",
            promotion_receipts_explicit,
            vec![String::from(MODULE_PROMOTION_STATE_REPORT_REF)],
            "promotion, quarantine, revocation, and supersession receipts remain explicit instead of optimistic widening.",
        ),
        validation_row(
            "trust_tiers_machine_checkable",
            trust_tier_gate_green,
            vec![
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
                String::from(STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF),
            ],
            "trust tiers remain machine-checkable rather than being inferred from benchmark counts or host posture, including the distinct digest-bound guest-artifact class.",
        ),
        validation_row(
            "validator_and_accepted_outcome_hooks_bound",
            validator_hooks_explicit && accepted_outcome_hooks_explicit,
            vec![
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            "profile-specific widening keeps validator and accepted-outcome hooks explicit where required instead of silently reusing the default served lane.",
        ),
        validation_row(
            "overclaim_posture_refused",
            weighted_plugin_control_allowed
                && !plugin_capability_claim_allowed
                && !plugin_publication_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed
                && broader_publication_refused,
            vec![
                String::from(CONTROLLER_EVAL_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            "the gate keeps bounded controller closure distinct from plugin-platform closeout and refuses any claim of arbitrary public Wasm or arbitrary public tool use.",
        ),
        validation_row(
            "guest_artifact_revocation_and_depublication_posture_explicit",
            guest_artifact_publication_blocked
                && promotion.revoked_count >= 1
                && !plugin_publication_allowed
                && broader_publication_refused,
            vec![
                String::from(STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF),
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
            ],
            "the digest-bound guest-artifact lane stays publication-blocked, revocable, and subject to later depublication review instead of inheriting host-native promotion or publication closure.",
        ),
        validation_row(
            "closure_bundle_bound_by_digest",
            closure_bundle_bound_by_digest,
            vec![
                String::from(CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF),
                String::from(CONTROLLER_EVAL_REPORT_REF),
                String::from(MANIFEST_CONTRACT_REPORT_REF),
            ],
            "the authority and publication claim now inherits the canonical machine closure bundle by digest instead of relying on adjacent machine fields only.",
        ),
    ];

    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && trust_tier_gate_green
        && promotion_receipts_explicit
        && publication_posture_explicit
        && observer_rights_explicit
        && validation_rows.iter().all(|row| row.green)
        && validator_hooks_explicit
        && accepted_outcome_hooks_explicit
        && operator_internal_only_posture
        && profile_specific_named_routes_explicit
        && broader_publication_refused
        && rebase_claim_allowed
        && weighted_plugin_control_allowed
        && !plugin_capability_claim_allowed
        && !plugin_publication_allowed
        && !served_public_universality_allowed
        && !arbitrary_software_capability_allowed;
    let contract_status = if contract_green {
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateStatus::Green
    } else {
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateStatus::Incomplete
    };

    let mut report =
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReport {
            schema_version: 1,
            report_id: String::from(
                "tassadar.post_article_plugin_authority_promotion_publication_and_trust_tier_gate.report.v1",
            ),
            checker_script_ref: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_CHECKER_REF,
            ),
            controller_eval_report_ref: String::from(CONTROLLER_EVAL_REPORT_REF),
            manifest_contract_report_ref: String::from(MANIFEST_CONTRACT_REPORT_REF),
            module_promotion_state_report_ref: String::from(MODULE_PROMOTION_STATE_REPORT_REF),
            module_trust_isolation_report_ref: String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
            broad_internal_compute_profile_publication_report_ref: String::from(
                BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
            ),
            broad_internal_compute_route_policy_report_ref: String::from(
                BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF,
            ),
            canonical_machine_closure_bundle_report_ref: String::from(
                CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
            ),
            local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            supporting_material_refs: vec![
                String::from(CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF),
                String::from(CONTROLLER_EVAL_REPORT_REF),
                String::from(MANIFEST_CONTRACT_REPORT_REF),
                String::from(MODULE_PROMOTION_STATE_REPORT_REF),
                String::from(MODULE_TRUST_ISOLATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
                String::from(BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
                String::from(PLUGIN_SYSTEM_AUDIT_REF),
                String::from(STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF),
            ],
            machine_identity_binding,
            dependency_rows,
            trust_tier_rows,
            promotion_rows,
            publication_posture_rows,
            observer_rows,
            validation_rows,
            contract_status,
            contract_green,
            trust_tier_gate_green,
            promotion_receipts_explicit,
            publication_posture_explicit,
            observer_rights_explicit,
            validator_hooks_explicit,
            accepted_outcome_hooks_explicit,
            operator_internal_only_posture,
            profile_specific_named_routes_explicit,
            broader_publication_refused,
            closure_bundle_bound_by_digest,
            rebase_claim_allowed,
            plugin_capability_claim_allowed,
            weighted_plugin_control_allowed,
            plugin_publication_allowed,
            served_public_universality_allowed,
            arbitrary_software_capability_allowed,
            deferred_issue_ids: Vec::new(),
            claim_boundary: String::from(
                "this catalog report freezes plugin authority, promotion, publication posture, and trust tiers above the canonical weighted controller without widening the claim surface. The authority and publication claim now inherits the canonical machine closure bundle by report id and digest instead of reconstructing machine identity from the controller and manifest pair alone. It keeps operator/internal posture, the one admitted digest-bound guest-artifact trust tier and blocked-publication row, profile-specific named-but-suppressed routes, validator and accepted-outcome hook requirements, promotion challengeability, quarantine, revocation, and broader public refusal explicit instead of implying a served/public plugin platform or arbitrary public software execution.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
    report.summary = format!(
        "post-article plugin authority gate binds machine_identity_id=`{}`, canonical_route_id=`{}`, contract_status={:?}, trust_tier_rows={}, promotion_rows={}, publication_posture_rows={}, observer_rows={}, validation_rows={}, closure_bundle_digest=`{}`, weighted_plugin_control_allowed={}, and deferred_issue_ids={}.",
        report.machine_identity_binding.machine_identity_id,
        report.machine_identity_binding.canonical_route_id,
        report.contract_status,
        report.trust_tier_rows.len(),
        report.promotion_rows.len(),
        report.publication_posture_rows.len(),
        report.observer_rows.len(),
        report.validation_rows.len(),
        report.machine_identity_binding.closure_bundle_digest,
        report.weighted_plugin_control_allowed,
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF,
    )
}

pub fn write_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReport,
    TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report =
        build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn dependency_row(
    dependency_id: &str,
    dependency_class: TassadarPostArticlePluginAuthorityDependencyClass,
    satisfied: bool,
    source_ref: &str,
    bound_report_id: Option<String>,
    bound_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticlePluginAuthorityDependencyRow {
    TassadarPostArticlePluginAuthorityDependencyRow {
        dependency_id: String::from(dependency_id),
        dependency_class,
        satisfied,
        source_ref: String::from(source_ref),
        bound_report_id,
        bound_report_digest,
        detail: String::from(detail),
    }
}

fn trust_tier_row(
    tier_id: &str,
    current_posture: &str,
    benchmark_bar: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticlePluginAuthorityTrustTierRow {
    TassadarPostArticlePluginAuthorityTrustTierRow {
        tier_id: String::from(tier_id),
        current_posture: String::from(current_posture),
        benchmark_bar: String::from(benchmark_bar),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn promotion_row(
    authority_id: &str,
    lifecycle_state: &str,
    receipt_required: bool,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticlePluginAuthorityPromotionRow {
    TassadarPostArticlePluginAuthorityPromotionRow {
        authority_id: String::from(authority_id),
        lifecycle_state: String::from(lifecycle_state),
        receipt_required,
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn publication_posture_row(
    posture_id: &str,
    publication_posture: &str,
    publication_status: &str,
    validator_hook_status: &str,
    accepted_outcome_hook_status: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticlePluginAuthorityPublicationPostureRow {
    TassadarPostArticlePluginAuthorityPublicationPostureRow {
        posture_id: String::from(posture_id),
        publication_posture: String::from(publication_posture),
        publication_status: String::from(publication_status),
        validator_hook_status: String::from(validator_hook_status),
        accepted_outcome_hook_status: String::from(accepted_outcome_hook_status),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn observer_row(
    observer_id: &str,
    permitted_actions: Vec<String>,
    publication_posture: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticlePluginAuthorityObserverRow {
    TassadarPostArticlePluginAuthorityObserverRow {
        observer_id: String::from(observer_id),
        permitted_actions,
        publication_posture: String::from(publication_posture),
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
) -> TassadarPostArticlePluginAuthorityValidationRow {
    TassadarPostArticlePluginAuthorityValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn has_trust_bundle(
    trust: &ModuleTrustIsolationFixture,
    module_ref: &str,
    trust_posture: &str,
    isolation_boundary: &str,
    min_benchmark_ref_count: u32,
) -> bool {
    trust.bundles.iter().any(|bundle| {
        bundle.module_ref == module_ref
            && bundle.trust_posture == trust_posture
            && bundle.isolation_boundary == isolation_boundary
            && bundle.benchmark_ref_count >= min_benchmark_ref_count
    })
}

fn publication_profile_row<'a>(
    publication: &'a BroadInternalComputeProfilePublicationFixture,
    profile_id: &str,
) -> Option<&'a BroadInternalComputeProfileRowFixture> {
    publication
        .profile_rows
        .iter()
        .find(|row| row.profile_id == profile_id)
}

fn route_policy_row<'a>(
    route_policy: &'a BroadInternalComputeRoutePolicyFixture,
    route_policy_id: &str,
) -> Option<&'a BroadInternalComputeRoutePolicyRowFixture> {
    route_policy
        .rows
        .iter()
        .find(|row| row.route_policy_id == route_policy_id)
}

fn profile_specific_route_green(
    publication_row: Option<&BroadInternalComputeProfileRowFixture>,
    route_row: Option<&BroadInternalComputeRoutePolicyRowFixture>,
) -> bool {
    matches!(
        (publication_row, route_row),
        (Some(publication_row), Some(route_row))
            if publication_row.publication_status == "suppressed"
                && publication_row.world_mount_binding_status
                    == "profile_specific_mount_template_available"
                && publication_row.accepted_outcome_binding_status
                    == "profile_specific_accepted_outcome_template_available"
                && route_row.publication_status == "suppressed"
                && route_row.decision_status == "promoted_profile_specific"
                && route_row.world_mount_binding_status
                    == "profile_specific_mount_template_available"
                && route_row.accepted_outcome_binding_status
                    == "profile_specific_accepted_outcome_template_available"
    )
}

fn suppressed_public_broad_green(
    publication_row: Option<&BroadInternalComputeProfileRowFixture>,
    route_row: Option<&BroadInternalComputeRoutePolicyRowFixture>,
) -> bool {
    matches!(
        (publication_row, route_row),
        (Some(publication_row), Some(route_row))
            if publication_row.publication_status == "suppressed"
                && route_row.publication_status == "suppressed"
                && route_row.decision_status == "suppressed"
                && route_row.required_runtime_support_id.as_deref()
                    == Some("broad_profile_publication_and_route_policy")
    )
}

fn refused_portable_broad_green(
    publication_row: Option<&BroadInternalComputeProfileRowFixture>,
    route_row: Option<&BroadInternalComputeRoutePolicyRowFixture>,
) -> bool {
    matches!(
        (publication_row, route_row),
        (Some(publication_row), Some(route_row))
            if publication_row.publication_status == "failed"
                && route_row.publication_status == "failed"
                && route_row.decision_status == "refused"
                && route_row.world_mount_binding_status == "refused_pending_profile_evidence"
                && route_row.accepted_outcome_binding_status
                    == "refused_pending_profile_evidence"
    )
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
) -> Result<T, TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ControllerEvalFixture {
    report_id: String,
    report_digest: String,
    machine_identity_binding: ControllerMachineIdentityFixture,
    contract_green: bool,
    control_trace_contract_green: bool,
    determinism_profile_explicit: bool,
    typed_refusal_loop_closed: bool,
    host_not_planner_green: bool,
    adversarial_negative_rows_green: bool,
    closure_bundle_bound_by_digest: bool,
    operator_internal_only_posture: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
    deferred_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ControllerMachineIdentityFixture {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
    computational_model_statement_id: String,
    control_trace_contract_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CanonicalMachineClosureBundleFixture {
    report_id: String,
    report_digest: String,
    closure_bundle_digest: String,
    bundle_green: bool,
    closure_subject: CanonicalMachineClosureBundleSubjectFixture,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CanonicalMachineClosureBundleSubjectFixture {
    machine_identity_id: String,
    canonical_route_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ManifestContractFixture {
    report_id: String,
    report_digest: String,
    contract_green: bool,
    operator_internal_only_posture: bool,
    manifest_fields_frozen: bool,
    canonical_invocation_identity_frozen: bool,
    hot_swap_rules_frozen: bool,
    multi_module_packaging_explicit: bool,
    linked_bundle_identity_explicit: bool,
    rebase_claim_allowed: bool,
    deferred_issue_ids: Vec<String>,
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
    nexus_dependency_marker: String,
    kernel_policy_dependency_marker: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ModuleTrustIsolationFixture {
    report_id: String,
    report_digest: String,
    bundles: Vec<ModuleTrustBundleFixture>,
    allowed_case_count: u32,
    refused_case_count: u32,
    cross_tier_refusal_count: u32,
    privilege_escalation_refusal_count: u32,
    mount_policy_refusal_count: u32,
    cluster_trust_dependency_marker: String,
    kernel_policy_dependency_marker: String,
    world_mount_dependency_marker: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ModuleTrustBundleFixture {
    module_ref: String,
    trust_posture: String,
    isolation_boundary: String,
    benchmark_ref_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BroadInternalComputeProfilePublicationFixture {
    report_id: String,
    report_digest: String,
    current_served_profile_id: String,
    profile_rows: Vec<BroadInternalComputeProfileRowFixture>,
    published_profile_ids: Vec<String>,
    suppressed_profile_ids: Vec<String>,
    failed_profile_ids: Vec<String>,
    world_mount_dependency_marker: String,
    nexus_dependency_marker: String,
    kernel_policy_dependency_marker: String,
    compute_market_dependency_marker: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BroadInternalComputeProfileRowFixture {
    profile_id: String,
    publication_status: String,
    world_mount_binding_status: String,
    accepted_outcome_binding_status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BroadInternalComputeRoutePolicyFixture {
    report_id: String,
    report_digest: String,
    current_served_profile_id: String,
    rows: Vec<BroadInternalComputeRoutePolicyRowFixture>,
    selected_route_count: u32,
    promoted_profile_specific_route_count: u32,
    suppressed_route_count: u32,
    refused_route_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BroadInternalComputeRoutePolicyRowFixture {
    route_policy_id: String,
    publication_status: String,
    decision_status: String,
    world_mount_binding_status: String,
    accepted_outcome_binding_status: String,
    #[serde(default)]
    required_runtime_support_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct StarterPluginCatalogRuntimeBundleFixture {
    plugin_count: u32,
    operator_only_posture: bool,
    public_marketplace_implication_allowed: bool,
    descriptor_rows: Vec<StarterPluginCatalogRuntimeDescriptorFixture>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct StarterPluginCatalogRuntimeDescriptorFixture {
    plugin_id: String,
    capability_class: String,
    replay_class_id: String,
    trust_tier_id: String,
    evidence_posture_id: String,
    negative_claim_ids: Vec<String>,
}

fn guest_artifact_trust_tier_green(
    runtime: &StarterPluginCatalogRuntimeBundleFixture,
) -> bool {
    runtime.plugin_count >= 6
        && runtime.operator_only_posture
        && !runtime.public_marketplace_implication_allowed
        && guest_artifact_descriptor(runtime).is_some_and(|descriptor| {
            descriptor.capability_class == "local_deterministic"
                && descriptor.replay_class_id == GUEST_ARTIFACT_REPLAY_CLASS_ID
                && descriptor.trust_tier_id == GUEST_ARTIFACT_TRUST_TIER_ID
                && descriptor.evidence_posture_id == GUEST_ARTIFACT_EVIDENCE_POSTURE_ID
                && descriptor
                    .negative_claim_ids
                    .iter()
                    .any(|claim| {
                        claim == GUEST_ARTIFACT_PUBLICATION_NOT_CLAIMED_NEGATIVE_CLAIM_ID
                    })
                && descriptor
                    .negative_claim_ids
                    .iter()
                    .any(|claim| {
                        claim == GUEST_ARTIFACT_ARBITRARY_WASM_NOT_CLAIMED_NEGATIVE_CLAIM_ID
                    })
                && descriptor
                    .negative_claim_ids
                    .iter()
                    .any(|claim| {
                        claim == GUEST_ARTIFACT_SECRET_OR_STATEFUL_NOT_CLAIMED_NEGATIVE_CLAIM_ID
                    })
        })
}

fn guest_artifact_publication_blocked(
    runtime: &StarterPluginCatalogRuntimeBundleFixture,
) -> bool {
    guest_artifact_trust_tier_green(runtime)
}

fn guest_artifact_descriptor(
    runtime: &StarterPluginCatalogRuntimeBundleFixture,
) -> Option<&StarterPluginCatalogRuntimeDescriptorFixture> {
    runtime
        .descriptor_rows
        .iter()
        .find(|descriptor| descriptor.plugin_id == GUEST_ARTIFACT_PLUGIN_ID)
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report,
        read_json,
        tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report_path,
        write_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report,
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReport,
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateStatus,
        TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn post_article_plugin_authority_gate_freezes_trust_promotion_and_publication_posture() {
        let report =
            build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report()
                .expect("report");

        assert_eq!(
            report.contract_status,
            TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateStatus::Green
        );
        assert!(report.contract_green);
        assert!(report.trust_tier_gate_green);
        assert!(report.promotion_receipts_explicit);
        assert!(report.publication_posture_explicit);
        assert!(report.observer_rights_explicit);
        assert!(report.validator_hooks_explicit);
        assert!(report.accepted_outcome_hooks_explicit);
        assert!(report.operator_internal_only_posture);
        assert!(report.profile_specific_named_routes_explicit);
        assert!(report.broader_publication_refused);
        assert!(report.closure_bundle_bound_by_digest);
        assert!(report.rebase_claim_allowed);
        assert!(report.weighted_plugin_control_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
        assert_eq!(report.dependency_rows.len(), 9);
        assert_eq!(report.trust_tier_rows.len(), 5);
        assert_eq!(report.promotion_rows.len(), 5);
        assert_eq!(report.publication_posture_rows.len(), 6);
        assert_eq!(report.observer_rows.len(), 4);
        assert_eq!(report.validation_rows.len(), 10);
        assert!(report.deferred_issue_ids.is_empty());
    }

    #[test]
    fn post_article_plugin_authority_gate_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report()
                .expect("report");
        let committed: TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReport =
            read_json(
                tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report_path(),
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report.json"
            )
        );
    }

    #[test]
    fn write_post_article_plugin_authority_gate_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report.json",
        );
        let written =
            write_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
        assert_eq!(
            TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report.json"
        );
    }
}
