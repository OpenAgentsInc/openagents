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
    TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_v1/tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_ID: &str =
    "tassadar.plugin_runtime.world_mount_envelope_compiler.v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_ADMISSIBILITY_CONTRACT_ID: &str =
    "tassadar.plugin_runtime.admissibility.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginAdmissibilityCaseStatus {
    ExactAdmittedEnvelope,
    ExactDeniedEnvelope,
    ExactSuppressedEnvelope,
    ExactQuarantinedEnvelope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAdmissibilityRuleRow {
    pub rule_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCandidateSetRow {
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
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginEquivalentChoiceRow {
    pub equivalent_choice_class_id: String,
    pub bounded_candidate_count: u32,
    pub neutral_choice_auditable: bool,
    pub hidden_ranking_allowed: bool,
    pub receipt_visible_transforms_required: bool,
    pub violation_reason_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginWorldMountEnvelopeRow {
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
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAdmissibilityCaseReceipt {
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
    pub note: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub host_owned_runtime_api_id: String,
    pub invocation_receipt_profile_id: String,
    pub world_mount_envelope_compiler_id: String,
    pub admissibility_contract_id: String,
    pub admissibility_rule_rows: Vec<TassadarPostArticlePluginAdmissibilityRuleRow>,
    pub candidate_set_rows: Vec<TassadarPostArticlePluginCandidateSetRow>,
    pub equivalent_choice_rows: Vec<TassadarPostArticlePluginEquivalentChoiceRow>,
    pub envelope_rows: Vec<TassadarPostArticlePluginWorldMountEnvelopeRow>,
    pub case_receipts: Vec<TassadarPostArticlePluginAdmissibilityCaseReceipt>,
    pub exact_admitted_case_count: u32,
    pub exact_denied_case_count: u32,
    pub exact_suppressed_case_count: u32,
    pub exact_quarantined_case_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundleError {
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

#[must_use]
pub fn build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle(
) -> TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundle {
    let admissibility_rule_rows = vec![
        admissibility_rule_row(
            "closed_world_candidate_sets_explicit",
            "candidate sets remain fully enumerated or explicitly bounded instead of discovered ad hoc at invocation time.",
        ),
        admissibility_rule_row(
            "route_policy_binds_plugin_ids",
            "route policy remains bound to named plugin ids and does not admit hidden plugin substitution.",
        ),
        admissibility_rule_row(
            "version_constraints_visible",
            "plugin version constraints remain explicit inside the admissibility contract.",
        ),
        admissibility_rule_row(
            "trust_posture_binds_admission",
            "trust posture remains an explicit admissibility input instead of host-side policy drift.",
        ),
        admissibility_rule_row(
            "publication_posture_binds_admission",
            "publication posture remains explicit so served-disallowed plugins cannot silently widen claims.",
        ),
        admissibility_rule_row(
            "receipt_visible_filtering_required",
            "filtering, ranking, and transformation receipts remain explicit whenever admissibility is narrowed.",
        ),
        admissibility_rule_row(
            "equivalent_choice_neutrality_auditable",
            "equivalent-choice classes remain explicit and fail closed when neutral choice is violated.",
        ),
        admissibility_rule_row(
            "world_mount_envelopes_compiled",
            "world-mount, capability, network, timeout, memory, and provenance posture remain compiled into runtime-owned envelopes.",
        ),
        admissibility_rule_row(
            "explicit_denial_and_quarantine_behavior",
            "unsupported, disallowed, suppressed, and quarantined invocations remain typed instead of collapsing into one generic failure.",
        ),
    ];
    let candidate_set_rows = vec![
        candidate_set_row(
            "candidate_set.benchmark_graph.v1",
            "route.deterministic_import.subset",
            "mount.benchmark_graph",
            &[
                "plugin.frontier_relax_core",
                "plugin.checkpoint_backtrack_core",
            ],
            &[
                "plugin.frontier_relax_core@>=1.0.0,<2.0.0",
                "plugin.checkpoint_backtrack_core@=1.0.0",
            ],
            true,
            true,
            false,
            true,
            "choice.search_core_pair.closed_world_neutral.v1",
            "benchmark-graph admission stays closed-world and receipt-visible across a bounded pair of internal search-core plugins.",
        ),
        candidate_set_row(
            "candidate_set.validator_search.v1",
            "route.deterministic_import.subset",
            "mount.validator_search",
            &["plugin.candidate_select_core"],
            &["plugin.candidate_select_core@=1.1.0"],
            true,
            true,
            false,
            true,
            "choice.validator_search.singleton.v1",
            "validator-search admission stays singleton and exact under the route-plus-mount contract.",
        ),
        candidate_set_row(
            "candidate_set.strict_no_imports.v1",
            "route.article_closeout.served_exact",
            "mount.strict_no_imports",
            &["plugin.frontier_relax_core"],
            &["plugin.frontier_relax_core@=1.0.0"],
            true,
            true,
            false,
            true,
            "choice.strict_no_imports.singleton.v1",
            "strict no-import admission remains singleton and exact so import-posture refusal stays typed.",
        ),
        candidate_set_row(
            "candidate_set.missing_dependency.v1",
            "route.deterministic_import.subset",
            "mount.missing_dependency",
            &["plugin.branch_prune_core"],
            &["plugin.branch_prune_core@=0.1.0"],
            true,
            true,
            false,
            true,
            "choice.missing_dependency.singleton.v1",
            "missing-dependency admission remains closed-world and quarantinable instead of silently widening to substitute plugins.",
        ),
        candidate_set_row(
            "candidate_set.public_broad_family.v1",
            "route.public_broad_family.publication",
            "mount.validator_search",
            &["plugin.candidate_select_core"],
            &["plugin.candidate_select_core@=1.1.0"],
            true,
            true,
            false,
            true,
            "choice.public_broad_family.singleton.v1",
            "served/public publication candidates remain bounded and explicit even when the selected route is still suppressed.",
        ),
    ];
    let equivalent_choice_rows = vec![
        equivalent_choice_row(
            "choice.search_core_pair.closed_world_neutral.v1",
            2,
            true,
            false,
            true,
            "equivalent_choice_neutrality_violation",
            "two bounded internal search-core candidates remain admissible only under neutral closed-world choice with receipt-visible filtering.",
        ),
        equivalent_choice_row(
            "choice.validator_search.singleton.v1",
            1,
            true,
            false,
            true,
            "version_constraint_unsatisfied",
            "singleton validator-search admission still requires an explicit filter receipt and fails closed on version skew.",
        ),
        equivalent_choice_row(
            "choice.strict_no_imports.singleton.v1",
            1,
            true,
            false,
            true,
            "import_posture_incompatible",
            "strict no-import admission remains singleton and fails closed on import-posture mismatch.",
        ),
        equivalent_choice_row(
            "choice.missing_dependency.singleton.v1",
            1,
            true,
            false,
            true,
            "module_dependency_missing",
            "missing-dependency admission remains singleton and explicit so quarantine is typed rather than widened.",
        ),
        equivalent_choice_row(
            "choice.public_broad_family.singleton.v1",
            1,
            true,
            false,
            true,
            "route_policy_suppressed",
            "public-broad-family admission remains singleton and explicit even while the route stays suppressed.",
        ),
    ];
    let envelope_rows = vec![
        envelope_row(
            "envelope.benchmark_graph.internal_search.v1",
            "route.deterministic_import.subset",
            "mount.benchmark_graph",
            "plugin.frontier_relax_core",
            "1.0.0",
            &["env.clock_stub"],
            &["network.none"],
            &["artifact_mount.validation_dictionary"],
            150,
            8_388_608,
            1,
            &[
                "route_evidence_required",
                "closed_world_candidate_set_receipt_required",
            ],
            "deterministic_replayable",
            "benchmark_gated_internal",
            "operator_internal_only",
            "benchmark-graph admission compiles one deterministic internal-only mount envelope with explicit capability, artifact, replay, and provenance posture.",
        ),
        envelope_row(
            "envelope.validator_search.operator_internal.v1",
            "route.deterministic_import.subset",
            "mount.validator_search",
            "plugin.candidate_select_core",
            "1.1.0",
            &["env.clock_stub", "sandbox.math_eval"],
            &["sim.network.loopback_fifo_seed_7.v1"],
            &[
                "artifact_mount.validation_dictionary",
                "artifact_mount.ephemeral_workspace",
            ],
            150,
            8_388_608,
            1,
            &[
                "route_evidence_required",
                "validator_binding_required",
                "challenge_receipt_required",
            ],
            "deterministic_replayable",
            "challenge_gated_install",
            "operator_internal_only",
            "validator-search admission compiles one operator/internal envelope with explicit sandbox delegation, loopback-only network posture, and validator binding requirements.",
        ),
    ];
    let case_receipts = vec![
        admissibility_case(
            "benchmark_graph_frontier_relax_core_admitted",
            TassadarPostArticlePluginAdmissibilityCaseStatus::ExactAdmittedEnvelope,
            "route.deterministic_import.subset",
            "mount.benchmark_graph",
            "candidate_set.benchmark_graph.v1",
            Some("plugin.frontier_relax_core"),
            Some("1.0.0"),
            Some("reinstall.frontier_relax_core.session.v2"),
            "benchmark_gated_internal",
            "operator_internal_only",
            "choice.search_core_pair.closed_world_neutral.v1",
            &[
                "receipt.filter.benchmark_graph.closed_world.v1",
                "receipt.choice.search_core_pair.neutral.v1",
            ],
            Some("envelope.benchmark_graph.internal_search.v1"),
            None,
            "benchmark-graph admission stays exact because candidate enumeration, neutral choice, version constraints, and the compiled runtime envelope all remain explicit.",
        ),
        admissibility_case(
            "validator_search_candidate_select_core_admitted",
            TassadarPostArticlePluginAdmissibilityCaseStatus::ExactAdmittedEnvelope,
            "route.deterministic_import.subset",
            "mount.validator_search",
            "candidate_set.validator_search.v1",
            Some("plugin.candidate_select_core"),
            Some("1.1.0"),
            Some("install.candidate_select_core.rollback.v1"),
            "challenge_gated_install",
            "operator_internal_only",
            "choice.validator_search.singleton.v1",
            &["receipt.filter.validator_search.singleton.v1"],
            Some("envelope.validator_search.operator_internal.v1"),
            None,
            "validator-search admission stays exact because the route, mount, version, trust, and compiled envelope all remain explicit.",
        ),
        admissibility_case(
            "strict_no_imports_denied",
            TassadarPostArticlePluginAdmissibilityCaseStatus::ExactDeniedEnvelope,
            "route.article_closeout.served_exact",
            "mount.strict_no_imports",
            "candidate_set.strict_no_imports.v1",
            None,
            None,
            None,
            "benchmark_gated_internal",
            "operator_internal_only",
            "choice.strict_no_imports.singleton.v1",
            &["receipt.filter.strict_no_imports.closed_world.v1"],
            None,
            Some("import_posture_incompatible"),
            "strict no-import admission fails closed because the world mount demands a narrower import posture than the admissible plugin route can supply.",
        ),
        admissibility_case(
            "validator_search_version_denied",
            TassadarPostArticlePluginAdmissibilityCaseStatus::ExactDeniedEnvelope,
            "route.deterministic_import.subset",
            "mount.validator_search",
            "candidate_set.validator_search.v1",
            None,
            None,
            None,
            "challenge_gated_install",
            "operator_internal_only",
            "choice.validator_search.singleton.v1",
            &["receipt.filter.validator_search.singleton.v1"],
            None,
            Some("version_constraint_unsatisfied"),
            "validator-search admission fails closed when a candidate plugin version is outside the route-bound admissible version constraint.",
        ),
        admissibility_case(
            "missing_dependency_quarantined",
            TassadarPostArticlePluginAdmissibilityCaseStatus::ExactQuarantinedEnvelope,
            "route.deterministic_import.subset",
            "mount.missing_dependency",
            "candidate_set.missing_dependency.v1",
            None,
            None,
            None,
            "benchmark_gated_internal",
            "operator_internal_only",
            "choice.missing_dependency.singleton.v1",
            &["receipt.filter.missing_dependency.closed_world.v1"],
            None,
            Some("module_dependency_missing"),
            "missing dependency remains a typed quarantine posture instead of silently substituting or widening the admissible candidate set.",
        ),
        admissibility_case(
            "benchmark_graph_hidden_ranking_suppressed",
            TassadarPostArticlePluginAdmissibilityCaseStatus::ExactSuppressedEnvelope,
            "route.deterministic_import.subset",
            "mount.benchmark_graph",
            "candidate_set.benchmark_graph.v1",
            None,
            None,
            None,
            "benchmark_gated_internal",
            "operator_internal_only",
            "choice.search_core_pair.closed_world_neutral.v1",
            &[],
            None,
            Some("equivalent_choice_neutrality_violation"),
            "benchmark-graph admission is suppressed when an equivalent-choice class would require hidden ranking instead of a receipt-visible neutral selection trace.",
        ),
        admissibility_case(
            "public_broad_family_route_suppressed",
            TassadarPostArticlePluginAdmissibilityCaseStatus::ExactSuppressedEnvelope,
            "route.public_broad_family.publication",
            "mount.validator_search",
            "candidate_set.public_broad_family.v1",
            None,
            None,
            None,
            "challenge_gated_install",
            "served_disallowed",
            "choice.public_broad_family.singleton.v1",
            &["receipt.filter.public_broad_family.closed_world.v1"],
            None,
            Some("route_policy_suppressed"),
            "public-broad-family admission stays suppressed because the route policy is not yet allowed to widen served/public claims.",
        ),
    ];
    let exact_admitted_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactAdmittedEnvelope
        })
        .count() as u32;
    let exact_denied_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactDeniedEnvelope
        })
        .count() as u32;
    let exact_suppressed_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactSuppressedEnvelope
        })
        .count() as u32;
    let exact_quarantined_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status
                == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactQuarantinedEnvelope
        })
        .count() as u32;

    let mut bundle = TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundle {
        schema_version: 1,
        bundle_id: String::from(
            "tassadar.post_article_plugin_world_mount_envelope_compiler_and_admissibility.runtime_bundle.v1",
        ),
        host_owned_runtime_api_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
        ),
        invocation_receipt_profile_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
        ),
        world_mount_envelope_compiler_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_ID,
        ),
        admissibility_contract_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_ADMISSIBILITY_CONTRACT_ID),
        admissibility_rule_rows,
        candidate_set_rows,
        equivalent_choice_rows,
        envelope_rows,
        case_receipts,
        exact_admitted_case_count,
        exact_denied_case_count,
        exact_suppressed_case_count,
        exact_quarantined_case_count,
        claim_boundary: String::from(
            "this runtime bundle freezes the canonical post-article plugin admissibility and world-mount envelope compiler above the host-owned runtime API and invocation-receipt contract. It keeps candidate-set enumeration, equivalent-choice classes, route-policy and world-mount binding, version constraints, trust posture, publication posture, compiled runtime envelopes, and explicit denial or quarantine behavior machine-readable while keeping weighted plugin control, plugin publication, served/public universality, and arbitrary software capability blocked.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Post-article plugin admissibility bundle covers admissibility_rules={}, candidate_sets={}, equivalent_choice_classes={}, envelopes={}, admitted_cases={}, denied_cases={}, suppressed_cases={}, quarantined_cases={}.",
        bundle.admissibility_rule_rows.len(),
        bundle.candidate_set_rows.len(),
        bundle.equivalent_choice_rows.len(),
        bundle.envelope_rows.len(),
        bundle.exact_admitted_case_count,
        bundle.exact_denied_case_count,
        bundle.exact_suppressed_case_count,
        bundle.exact_quarantined_case_count,
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle|",
        &bundle,
    );
    bundle
}

#[must_use]
pub fn tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
    )
}

pub fn write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundle,
    TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundleError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle =
        build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn admissibility_rule_row(
    rule_id: &str,
    detail: &str,
) -> TassadarPostArticlePluginAdmissibilityRuleRow {
    TassadarPostArticlePluginAdmissibilityRuleRow {
        rule_id: String::from(rule_id),
        green: true,
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn candidate_set_row(
    candidate_set_id: &str,
    route_policy_id: &str,
    world_mount_id: &str,
    candidate_plugin_ids: &[&str],
    version_constraints: &[&str],
    closed_world_enumerated: bool,
    explicit_enumeration_required: bool,
    hidden_ranking_allowed: bool,
    receipt_visible_filter_required: bool,
    equivalent_choice_class_id: &str,
    detail: &str,
) -> TassadarPostArticlePluginCandidateSetRow {
    TassadarPostArticlePluginCandidateSetRow {
        candidate_set_id: String::from(candidate_set_id),
        route_policy_id: String::from(route_policy_id),
        world_mount_id: String::from(world_mount_id),
        candidate_plugin_ids: candidate_plugin_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        version_constraints: version_constraints
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        closed_world_enumerated,
        explicit_enumeration_required,
        hidden_ranking_allowed,
        receipt_visible_filter_required,
        equivalent_choice_class_id: String::from(equivalent_choice_class_id),
        detail: String::from(detail),
    }
}

fn equivalent_choice_row(
    equivalent_choice_class_id: &str,
    bounded_candidate_count: u32,
    neutral_choice_auditable: bool,
    hidden_ranking_allowed: bool,
    receipt_visible_transforms_required: bool,
    violation_reason_id: &str,
    detail: &str,
) -> TassadarPostArticlePluginEquivalentChoiceRow {
    TassadarPostArticlePluginEquivalentChoiceRow {
        equivalent_choice_class_id: String::from(equivalent_choice_class_id),
        bounded_candidate_count,
        neutral_choice_auditable,
        hidden_ranking_allowed,
        receipt_visible_transforms_required,
        violation_reason_id: String::from(violation_reason_id),
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn envelope_row(
    envelope_id: &str,
    route_policy_id: &str,
    world_mount_id: &str,
    selected_plugin_id: &str,
    selected_plugin_version: &str,
    mounted_capability_namespace_ids: &[&str],
    network_rule_ids: &[&str],
    artifact_mount_ids: &[&str],
    timeout_ceiling_millis: u32,
    memory_ceiling_bytes: u64,
    concurrency_ceiling: u16,
    provenance_requirement_ids: &[&str],
    replay_requirement_id: &str,
    trust_posture_id: &str,
    publication_posture_id: &str,
    detail: &str,
) -> TassadarPostArticlePluginWorldMountEnvelopeRow {
    TassadarPostArticlePluginWorldMountEnvelopeRow {
        envelope_id: String::from(envelope_id),
        route_policy_id: String::from(route_policy_id),
        world_mount_id: String::from(world_mount_id),
        selected_plugin_id: String::from(selected_plugin_id),
        selected_plugin_version: String::from(selected_plugin_version),
        mounted_capability_namespace_ids: mounted_capability_namespace_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        network_rule_ids: network_rule_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        artifact_mount_ids: artifact_mount_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        timeout_ceiling_millis,
        memory_ceiling_bytes,
        concurrency_ceiling,
        provenance_requirement_ids: provenance_requirement_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        replay_requirement_id: String::from(replay_requirement_id),
        trust_posture_id: String::from(trust_posture_id),
        publication_posture_id: String::from(publication_posture_id),
        closed_world_discovery_assumption: true,
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn admissibility_case(
    case_id: &str,
    status: TassadarPostArticlePluginAdmissibilityCaseStatus,
    route_policy_id: &str,
    world_mount_id: &str,
    candidate_set_id: &str,
    selected_plugin_id: Option<&str>,
    selected_plugin_version: Option<&str>,
    selected_install_id: Option<&str>,
    trust_posture_id: &str,
    publication_posture_id: &str,
    equivalent_choice_class_id: &str,
    filter_transform_receipt_ids: &[&str],
    envelope_id: Option<&str>,
    denial_reason_id: Option<&str>,
    note: &str,
) -> TassadarPostArticlePluginAdmissibilityCaseReceipt {
    let mut receipt = TassadarPostArticlePluginAdmissibilityCaseReceipt {
        case_id: String::from(case_id),
        status,
        route_policy_id: String::from(route_policy_id),
        world_mount_id: String::from(world_mount_id),
        candidate_set_id: String::from(candidate_set_id),
        selected_plugin_id: selected_plugin_id.map(String::from),
        selected_plugin_version: selected_plugin_version.map(String::from),
        selected_install_id: selected_install_id.map(String::from),
        trust_posture_id: String::from(trust_posture_id),
        publication_posture_id: String::from(publication_posture_id),
        equivalent_choice_class_id: String::from(equivalent_choice_class_id),
        filter_transform_receipt_ids: filter_transform_receipt_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        envelope_id: envelope_id.map(String::from),
        denial_reason_id: denial_reason_id.map(String::from),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_admissibility_case_receipt|",
        &receipt,
    );
    receipt
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-runtime should live under <repo>/crates/psionic-runtime")
        .to_path_buf()
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
) -> Result<T, TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle,
        read_json,
        tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle_path,
        write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle,
        TassadarPostArticlePluginAdmissibilityCaseStatus,
        TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundle,
        TASSADAR_POST_ARTICLE_PLUGIN_ADMISSIBILITY_CONTRACT_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF,
        TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_ID,
    };

    #[test]
    fn post_article_plugin_world_mount_admissibility_bundle_keeps_frontier_explicit() {
        let bundle =
            build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle();

        assert_eq!(
            bundle.world_mount_envelope_compiler_id,
            TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_ID
        );
        assert_eq!(
            bundle.admissibility_contract_id,
            TASSADAR_POST_ARTICLE_PLUGIN_ADMISSIBILITY_CONTRACT_ID
        );
        assert_eq!(bundle.admissibility_rule_rows.len(), 9);
        assert_eq!(bundle.candidate_set_rows.len(), 5);
        assert_eq!(bundle.equivalent_choice_rows.len(), 5);
        assert_eq!(bundle.envelope_rows.len(), 2);
        assert_eq!(bundle.case_receipts.len(), 7);
        assert_eq!(bundle.exact_admitted_case_count, 2);
        assert_eq!(bundle.exact_denied_case_count, 2);
        assert_eq!(bundle.exact_suppressed_case_count, 2);
        assert_eq!(bundle.exact_quarantined_case_count, 1);
        assert!(bundle.case_receipts.iter().any(|case| {
            case.status == TassadarPostArticlePluginAdmissibilityCaseStatus::ExactSuppressedEnvelope
                && case.denial_reason_id.as_deref()
                    == Some("equivalent_choice_neutrality_violation")
        }));
    }

    #[test]
    fn post_article_plugin_world_mount_admissibility_bundle_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle();
        let committed: TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundle =
            read_json(
                tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle_path(),
            )
            .expect("committed bundle");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle_path()
                .strip_prefix(super::repo_root())
                .expect("relative bundle path")
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_BUNDLE_REF
        );
    }

    #[test]
    fn write_post_article_plugin_world_mount_admissibility_bundle_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle.json",
        );
        let written =
            write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle(
                &output_path,
            )
            .expect("write bundle");
        let persisted: TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityBundle =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read bundle"))
                .expect("decode bundle");
        assert_eq!(written, persisted);
    }
}
