use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_starter_plugin_catalog_report.json";
pub const TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-starter-plugin-catalog.sh";

const STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/tassadar_post_article_starter_plugin_catalog_bundle.json";
const BOUNDED_PLATFORM_CLOSEOUT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report.json";
const AUTHORITY_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report.json";
const WORLD_MOUNT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report.json";
const CLOSURE_BUNDLE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_closure_bundle_report.json";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const AUTOPILOT_PORTING_NOTES_REF: &str =
    "~/code/alpha/autopilot/autopilot-extism-plugin-porting-into-rust-runtime.md";
const URL_EXTRACTOR_INVENTORY_REF: &str =
    "~/code/openagents-plugins/url-extractor-and-url-scraper.md";
const RSS_FEED_INVENTORY_REF: &str = "~/code/openagents-plugins/plugin-rss-feed/README.md";
const MULTI_PLUGIN_AUDIT_REF: &str =
    "docs/audits/2026-03-21-multi-plugin-real-run-orchestration-audit.md";
const PLUGIN_SYSTEM_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const NEXT_ISSUE_ID: &str = "TAS-217";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleStarterPluginCatalogStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleStarterPluginCatalogDependencyClass {
    RuntimeBundle,
    PlatformPrecedent,
    AuthorityPrecedent,
    WorldMountPrecedent,
    ClosureBundle,
    DesignInput,
    AuditInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub platform_closeout_report_id: String,
    pub platform_closeout_report_digest: String,
    pub authority_gate_report_id: String,
    pub authority_gate_report_digest: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub runtime_bundle_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticleStarterPluginCatalogDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogEntryRow {
    pub plugin_id: String,
    pub plugin_version: String,
    pub catalog_entry_id: String,
    pub capability_class_id: String,
    pub replay_class_id: String,
    pub trust_tier_id: String,
    pub descriptor_ref: String,
    pub fixture_bundle_ref: String,
    pub sample_mount_envelope_ref: String,
    pub operator_only: bool,
    pub local_deterministic: bool,
    pub read_only_network: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogCapabilityRow {
    pub plugin_id: String,
    pub capability_class_id: String,
    pub deterministic_replayable: bool,
    pub snapshot_backed_replay: bool,
    pub mount_required: bool,
    pub host_mediated_network_only: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogCompositionRow {
    pub case_id: String,
    pub step_plugin_ids: Vec<String>,
    pub hidden_host_orchestration_allowed: bool,
    pub schema_repair_allowed: bool,
    pub capability_leakage_allowed: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub runtime_bundle_ref: String,
    pub bounded_platform_closeout_report_ref: String,
    pub authority_gate_report_ref: String,
    pub world_mount_report_ref: String,
    pub closure_bundle_report_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding: TassadarPostArticleStarterPluginCatalogMachineIdentityBinding,
    pub dependency_rows: Vec<TassadarPostArticleStarterPluginCatalogDependencyRow>,
    pub entry_rows: Vec<TassadarPostArticleStarterPluginCatalogEntryRow>,
    pub capability_rows: Vec<TassadarPostArticleStarterPluginCatalogCapabilityRow>,
    pub composition_rows: Vec<TassadarPostArticleStarterPluginCatalogCompositionRow>,
    pub validation_rows: Vec<TassadarPostArticleStarterPluginCatalogValidationRow>,
    pub contract_status: TassadarPostArticleStarterPluginCatalogStatus,
    pub contract_green: bool,
    pub operator_internal_only_posture: bool,
    pub local_network_distinction_explicit: bool,
    pub descriptor_refs_complete: bool,
    pub fixture_bundle_refs_complete: bool,
    pub sample_mount_envelope_refs_complete: bool,
    pub composition_harness_green: bool,
    pub runtime_builtins_separate: bool,
    pub public_marketplace_language_suppressed: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub deferred_issue_ids: Vec<String>,
    pub next_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleStarterPluginCatalogReportError {
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CommonMachineIdentityInput {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
    #[serde(default)]
    computational_model_statement_id: Option<String>,
    #[serde(default)]
    closure_bundle_report_id: Option<String>,
    #[serde(default)]
    closure_bundle_report_digest: Option<String>,
    #[serde(default)]
    closure_bundle_digest: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct PlatformCloseoutInput {
    report_id: String,
    report_digest: String,
    machine_identity_binding: CommonMachineIdentityInput,
    closeout_green: bool,
    operator_internal_only_posture: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct AuthorityGateInput {
    report_id: String,
    report_digest: String,
    contract_green: bool,
    operator_internal_only_posture: bool,
    broader_publication_refused: bool,
    plugin_publication_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct WorldMountInput {
    report_id: String,
    report_digest: String,
    contract_green: bool,
    operator_internal_only_posture: bool,
    world_mount_envelope_compiler_frozen: bool,
    receipt_visible_filtering_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ClosureBundleInput {
    report_id: String,
    report_digest: String,
    bundle_green: bool,
    closure_bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RuntimeCapabilityClassInput {
    LocalDeterministic,
    ReadOnlyNetwork,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct RuntimeDescriptorInput {
    plugin_id: String,
    plugin_version: String,
    catalog_entry_id: String,
    capability_class: RuntimeCapabilityClassInput,
    replay_class_id: String,
    trust_tier_id: String,
    descriptor_ref: String,
    fixture_bundle_ref: String,
    sample_mount_envelope_ref: String,
    detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct RuntimeCapabilityRowInput {
    plugin_id: String,
    capability_class: RuntimeCapabilityClassInput,
    deterministic_replayable: bool,
    snapshot_backed_replay: bool,
    mount_required: bool,
    host_mediated_network_only: bool,
    detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct RuntimeCompositionCaseInput {
    case_id: String,
    step_plugin_ids: Vec<String>,
    hidden_host_orchestration_allowed: bool,
    schema_repair_allowed: bool,
    capability_leakage_allowed: bool,
    green: bool,
    detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct StarterCatalogRuntimeBundleInput {
    bundle_id: String,
    descriptor_rows: Vec<RuntimeDescriptorInput>,
    capability_matrix_rows: Vec<RuntimeCapabilityRowInput>,
    composition_case_rows: Vec<RuntimeCompositionCaseInput>,
    plugin_count: u32,
    local_deterministic_plugin_count: u32,
    read_only_network_plugin_count: u32,
    bounded_flow_count: u32,
    operator_only_posture: bool,
    runtime_builtins_separate: bool,
    public_marketplace_implication_allowed: bool,
    bundle_digest: String,
}

pub fn build_tassadar_post_article_starter_plugin_catalog_report() -> Result<
    TassadarPostArticleStarterPluginCatalogReport,
    TassadarPostArticleStarterPluginCatalogReportError,
> {
    let runtime: StarterCatalogRuntimeBundleInput =
        read_repo_json(STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF)?;
    let platform: PlatformCloseoutInput = read_repo_json(BOUNDED_PLATFORM_CLOSEOUT_REPORT_REF)?;
    let authority: AuthorityGateInput = read_repo_json(AUTHORITY_GATE_REPORT_REF)?;
    let world_mount: WorldMountInput = read_repo_json(WORLD_MOUNT_REPORT_REF)?;
    let closure_bundle: ClosureBundleInput = read_repo_json(CLOSURE_BUNDLE_REPORT_REF)?;

    let closure_bundle_bound_by_digest = platform
        .machine_identity_binding
        .closure_bundle_digest
        .as_deref()
        == Some(closure_bundle.closure_bundle_digest.as_str())
        && !closure_bundle.closure_bundle_digest.is_empty();
    let operator_internal_only_posture = runtime.operator_only_posture
        && platform.operator_internal_only_posture
        && authority.operator_internal_only_posture
        && world_mount.operator_internal_only_posture
        && !authority.plugin_publication_allowed
        && !platform.plugin_publication_allowed;
    let local_network_distinction_explicit = runtime.plugin_count == 6
        && runtime.local_deterministic_plugin_count == 5
        && runtime.read_only_network_plugin_count == 1
        && runtime
            .capability_matrix_rows
            .iter()
            .filter(|row| {
                matches!(
                    row.capability_class,
                    RuntimeCapabilityClassInput::ReadOnlyNetwork
                )
            })
            .count()
            == 1;
    let descriptor_refs_complete = runtime
        .descriptor_rows
        .iter()
        .all(|row| !row.descriptor_ref.is_empty());
    let fixture_bundle_refs_complete = runtime
        .descriptor_rows
        .iter()
        .all(|row| !row.fixture_bundle_ref.is_empty());
    let sample_mount_envelope_refs_complete = runtime
        .descriptor_rows
        .iter()
        .all(|row| !row.sample_mount_envelope_ref.is_empty());
    let composition_harness_green = runtime.bounded_flow_count == 2
        && runtime.composition_case_rows.iter().all(|row| {
            row.green
                && !row.hidden_host_orchestration_allowed
                && !row.schema_repair_allowed
                && !row.capability_leakage_allowed
        });
    let public_marketplace_language_suppressed =
        !runtime.public_marketplace_implication_allowed && authority.broader_publication_refused;

    let machine_identity_binding = TassadarPostArticleStarterPluginCatalogMachineIdentityBinding {
        machine_identity_id: platform
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: platform.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: platform.machine_identity_binding.canonical_route_id.clone(),
        canonical_route_descriptor_digest: platform
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: platform
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: platform
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: platform
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: platform
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: platform
            .machine_identity_binding
            .computational_model_statement_id
            .clone()
            .unwrap_or_default(),
        closure_bundle_report_id: closure_bundle.report_id.clone(),
        closure_bundle_report_digest: closure_bundle.report_digest.clone(),
        closure_bundle_digest: closure_bundle.closure_bundle_digest.clone(),
        platform_closeout_report_id: platform.report_id.clone(),
        platform_closeout_report_digest: platform.report_digest.clone(),
        authority_gate_report_id: authority.report_id.clone(),
        authority_gate_report_digest: authority.report_digest.clone(),
        runtime_bundle_id: runtime.bundle_id.clone(),
        runtime_bundle_digest: runtime.bundle_digest.clone(),
        runtime_bundle_ref: String::from(STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF),
        detail: format!(
            "machine_identity_id=`{}` canonical_route_id=`{}` closure_bundle_digest=`{}` and runtime_bundle_id=`{}` stay bound together for the starter plugin catalog.",
            platform.machine_identity_binding.machine_identity_id,
            platform.machine_identity_binding.canonical_route_id,
            closure_bundle.closure_bundle_digest,
            runtime.bundle_id,
        ),
    };

    let dependency_rows = vec![
        dependency_row(
            "runtime_bundle_present",
            TassadarPostArticleStarterPluginCatalogDependencyClass::RuntimeBundle,
            runtime.plugin_count == 6,
            STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF,
            Some(runtime.bundle_id.clone()),
            Some(runtime.bundle_digest.clone()),
            "the runtime-owned starter catalog bundle exists and names the six bounded starter plugins explicitly, including the digest-bound guest-artifact entry plus the earlier capability-free and manual networked_read_only rows.",
        ),
        dependency_row(
            "bounded_platform_closeout_green",
            TassadarPostArticleStarterPluginCatalogDependencyClass::PlatformPrecedent,
            platform.closeout_green,
            BOUNDED_PLATFORM_CLOSEOUT_REPORT_REF,
            Some(platform.report_id.clone()),
            Some(platform.report_digest.clone()),
            "the bounded weighted-plugin platform remains green before the starter catalog widens from substrate to a small curated plugin set.",
        ),
        dependency_row(
            "authority_gate_green",
            TassadarPostArticleStarterPluginCatalogDependencyClass::AuthorityPrecedent,
            authority.contract_green,
            AUTHORITY_GATE_REPORT_REF,
            Some(authority.report_id.clone()),
            Some(authority.report_digest.clone()),
            "the authority or promotion or publication gate remains green and keeps the starter catalog operator-only.",
        ),
        dependency_row(
            "world_mount_precedent_green",
            TassadarPostArticleStarterPluginCatalogDependencyClass::WorldMountPrecedent,
            world_mount.contract_green
                && world_mount.world_mount_envelope_compiler_frozen
                && world_mount.receipt_visible_filtering_required,
            WORLD_MOUNT_REPORT_REF,
            Some(world_mount.report_id.clone()),
            Some(world_mount.report_digest.clone()),
            "world-mount envelope and admissibility precedent remains explicit so starter plugin mounts do not rely on undocumented host behavior.",
        ),
        dependency_row(
            "closure_bundle_green",
            TassadarPostArticleStarterPluginCatalogDependencyClass::ClosureBundle,
            closure_bundle.bundle_green && closure_bundle_bound_by_digest,
            CLOSURE_BUNDLE_REPORT_REF,
            Some(closure_bundle.report_id.clone()),
            Some(closure_bundle.report_digest.clone()),
            "the starter catalog stays bound to the canonical machine closure bundle by digest instead of widening via host-only composition.",
        ),
        dependency_row(
            "local_plugin_system_spec_bound",
            TassadarPostArticleStarterPluginCatalogDependencyClass::DesignInput,
            true,
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the starter catalog is informed by the local plugin-system design notes but does not inherit legacy ABI or marketplace assumptions from them.",
        ),
        dependency_row(
            "multi_plugin_audit_bound",
            TassadarPostArticleStarterPluginCatalogDependencyClass::AuditInput,
            true,
            MULTI_PLUGIN_AUDIT_REF,
            None,
            None,
            "the starter catalog follows the staged audit that separated bounded runtime substrate from later real-run orchestration waves.",
        ),
    ];

    let entry_rows = runtime
        .descriptor_rows
        .iter()
        .map(|row| TassadarPostArticleStarterPluginCatalogEntryRow {
            plugin_id: row.plugin_id.clone(),
            plugin_version: row.plugin_version.clone(),
            catalog_entry_id: row.catalog_entry_id.clone(),
            capability_class_id: capability_class_id(&row.capability_class),
            replay_class_id: row.replay_class_id.clone(),
            trust_tier_id: row.trust_tier_id.clone(),
            descriptor_ref: row.descriptor_ref.clone(),
            fixture_bundle_ref: row.fixture_bundle_ref.clone(),
            sample_mount_envelope_ref: row.sample_mount_envelope_ref.clone(),
            operator_only: true,
            local_deterministic: matches!(
                row.capability_class,
                RuntimeCapabilityClassInput::LocalDeterministic
            ),
            read_only_network: matches!(
                row.capability_class,
                RuntimeCapabilityClassInput::ReadOnlyNetwork
            ),
            detail: row.detail.clone(),
        })
        .collect::<Vec<_>>();

    let capability_rows = runtime
        .capability_matrix_rows
        .iter()
            .map(|row| TassadarPostArticleStarterPluginCatalogCapabilityRow {
                plugin_id: row.plugin_id.clone(),
                capability_class_id: capability_class_id(&row.capability_class),
                deterministic_replayable: row.deterministic_replayable,
                snapshot_backed_replay: row.snapshot_backed_replay,
                mount_required: row.mount_required,
                host_mediated_network_only: row.host_mediated_network_only,
                green: if matches!(
                    row.capability_class,
                    RuntimeCapabilityClassInput::ReadOnlyNetwork
                ) {
                    row.mount_required && row.host_mediated_network_only
                } else if row.plugin_id == "plugin.example.echo_guest" {
                    row.mount_required && !row.host_mediated_network_only
                } else {
                    !row.mount_required && !row.host_mediated_network_only
                },
                detail: row.detail.clone(),
            })
        .collect::<Vec<_>>();

    let composition_rows = runtime
        .composition_case_rows
        .iter()
        .map(
            |row| TassadarPostArticleStarterPluginCatalogCompositionRow {
                case_id: row.case_id.clone(),
                step_plugin_ids: row.step_plugin_ids.clone(),
                hidden_host_orchestration_allowed: row.hidden_host_orchestration_allowed,
                schema_repair_allowed: row.schema_repair_allowed,
                capability_leakage_allowed: row.capability_leakage_allowed,
                green: row.green,
                detail: row.detail.clone(),
            },
        )
        .collect::<Vec<_>>();

    let validation_rows = vec![
        validation_row(
            "starter_plugin_count_exact",
            runtime.plugin_count == 6,
            &[STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF],
            "the starter catalog names exactly six operator-curated plugins, including one digest-bound guest-artifact entry plus the earlier capability-free and networked_read_only user-added rows, and does not imply a broader marketplace.",
        ),
        validation_row(
            "local_vs_network_distinction_explicit",
            local_network_distinction_explicit,
            &[STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF],
            "the catalog keeps five local deterministic plugins, including one digest-bound guest-artifact row, distinct from one read-only network plugin.",
        ),
        validation_row(
            "per_plugin_sidecars_complete",
            descriptor_refs_complete
                && fixture_bundle_refs_complete
                && sample_mount_envelope_refs_complete,
            &[STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF],
            "every starter plugin carries one descriptor, one fixture bundle, and one sample mount envelope ref.",
        ),
        validation_row(
            "composition_harness_green",
            composition_harness_green,
            &[STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF],
            "the starter catalog carries two bounded composition flows with no hidden host orchestration, schema repair, or capability leakage.",
        ),
        validation_row(
            "operator_only_posture_explicit",
            operator_internal_only_posture,
            &[
                STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF,
                BOUNDED_PLATFORM_CLOSEOUT_REPORT_REF,
                AUTHORITY_GATE_REPORT_REF,
            ],
            "starter catalog publication remains operator-curated and operator-only.",
        ),
        validation_row(
            "runtime_builtins_separate",
            runtime.runtime_builtins_separate,
            &[STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF],
            "starter plugins remain explicit plugin artifacts and are not collapsed into runtime built-ins.",
        ),
        validation_row(
            "public_marketplace_language_suppressed",
            public_marketplace_language_suppressed,
            &[
                STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF,
                AUTHORITY_GATE_REPORT_REF,
            ],
            "the starter catalog refuses public marketplace or public discovery language.",
        ),
        validation_row(
            "closure_bundle_digest_bound",
            closure_bundle_bound_by_digest,
            &[CLOSURE_BUNDLE_REPORT_REF, BOUNDED_PLATFORM_CLOSEOUT_REPORT_REF],
            "starter catalog claims stay tied to the canonical machine closure bundle digest.",
        ),
    ];

    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && validation_rows.iter().all(|row| row.green)
        && capability_rows.iter().all(|row| row.green);

    let mut report = TassadarPostArticleStarterPluginCatalogReport {
        schema_version: 1,
        report_id: String::from("tassadar.post_article.starter_plugin_catalog.report.v1"),
        checker_script_ref: String::from(TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_CHECKER_REF),
        runtime_bundle_ref: String::from(STARTER_PLUGIN_CATALOG_RUNTIME_BUNDLE_REF),
        bounded_platform_closeout_report_ref: String::from(BOUNDED_PLATFORM_CLOSEOUT_REPORT_REF),
        authority_gate_report_ref: String::from(AUTHORITY_GATE_REPORT_REF),
        world_mount_report_ref: String::from(WORLD_MOUNT_REPORT_REF),
        closure_bundle_report_ref: String::from(CLOSURE_BUNDLE_REPORT_REF),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        supporting_material_refs: vec![
            String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            String::from(AUTOPILOT_PORTING_NOTES_REF),
            String::from(URL_EXTRACTOR_INVENTORY_REF),
            String::from(RSS_FEED_INVENTORY_REF),
            String::from(MULTI_PLUGIN_AUDIT_REF),
            String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
        ],
        machine_identity_binding,
        dependency_rows,
        entry_rows,
        capability_rows,
        composition_rows,
        validation_rows,
        contract_status: if contract_green {
            TassadarPostArticleStarterPluginCatalogStatus::Green
        } else {
            TassadarPostArticleStarterPluginCatalogStatus::Incomplete
        },
        contract_green,
        operator_internal_only_posture,
        local_network_distinction_explicit,
        descriptor_refs_complete,
        fixture_bundle_refs_complete,
        sample_mount_envelope_refs_complete,
        composition_harness_green,
        runtime_builtins_separate: runtime.runtime_builtins_separate,
        public_marketplace_language_suppressed,
        closure_bundle_bound_by_digest,
        rebase_claim_allowed: platform.rebase_claim_allowed,
        plugin_capability_claim_allowed: platform.plugin_capability_claim_allowed,
        weighted_plugin_control_allowed: platform.weighted_plugin_control_allowed,
        plugin_publication_allowed: platform.plugin_publication_allowed,
        served_public_universality_allowed: platform.served_public_universality_allowed,
        arbitrary_software_capability_allowed: platform.arbitrary_software_capability_allowed,
        deferred_issue_ids: vec![
            String::from("TAS-217"),
            String::from("TAS-218"),
            String::from("TAS-219"),
            String::from("TAS-220"),
            String::from("TAS-221"),
            String::from("TAS-222"),
            String::from("TAS-223"),
            String::from("TAS-224"),
            String::from("TAS-225"),
            String::from("TAS-226"),
        ],
        next_issue_id: String::from(NEXT_ISSUE_ID),
        claim_boundary: String::from(
            "this catalog report publishes one small operator-curated starter plugin set above the bounded weighted-plugin platform while keeping plugin publication suppressed, served or public universality suppressed, arbitrary public tool use suppressed, and public marketplace language refused.",
        ),
        summary: format!(
            "starter catalog report binds {} starter plugins, {} local deterministic entries, {} read-only network entry, and {} bounded composition flows to the canonical machine closure bundle while keeping operator-only posture explicit.",
            runtime.plugin_count,
            runtime.local_deterministic_plugin_count,
            runtime.read_only_network_plugin_count,
            runtime.bounded_flow_count,
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_starter_plugin_catalog_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_starter_plugin_catalog_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_REPORT_REF)
}

pub fn write_tassadar_post_article_starter_plugin_catalog_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleStarterPluginCatalogReport,
    TassadarPostArticleStarterPluginCatalogReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleStarterPluginCatalogReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_starter_plugin_catalog_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn dependency_row(
    dependency_id: &str,
    dependency_class: TassadarPostArticleStarterPluginCatalogDependencyClass,
    satisfied: bool,
    source_ref: &str,
    bound_report_id: Option<String>,
    bound_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleStarterPluginCatalogDependencyRow {
    TassadarPostArticleStarterPluginCatalogDependencyRow {
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
) -> TassadarPostArticleStarterPluginCatalogValidationRow {
    TassadarPostArticleStarterPluginCatalogValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn capability_class_id(value: &RuntimeCapabilityClassInput) -> String {
    match value {
        RuntimeCapabilityClassInput::LocalDeterministic => String::from("local_deterministic"),
        RuntimeCapabilityClassInput::ReadOnlyNetwork => String::from("read_only_network"),
    }
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
) -> Result<T, TassadarPostArticleStarterPluginCatalogReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleStarterPluginCatalogReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_starter_plugin_catalog_report, read_json,
        tassadar_post_article_starter_plugin_catalog_report_path,
        write_tassadar_post_article_starter_plugin_catalog_report,
        TassadarPostArticleStarterPluginCatalogReport,
        TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_REPORT_REF,
    };

    #[test]
    fn starter_plugin_catalog_report_keeps_operator_only_catalog_explicit() {
        let report = build_tassadar_post_article_starter_plugin_catalog_report().expect("report");

        assert_eq!(
            report.report_id,
            "tassadar.post_article.starter_plugin_catalog.report.v1"
        );
        assert!(report.contract_green);
        assert!(report.operator_internal_only_posture);
        assert!(report.local_network_distinction_explicit);
        assert!(report.descriptor_refs_complete);
        assert!(report.fixture_bundle_refs_complete);
        assert!(report.sample_mount_envelope_refs_complete);
        assert!(report.composition_harness_green);
        assert!(report.runtime_builtins_separate);
        assert!(report.public_marketplace_language_suppressed);
        assert!(report.closure_bundle_bound_by_digest);
        assert!(report.plugin_capability_claim_allowed);
        assert!(report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert_eq!(report.next_issue_id, "TAS-217");
        assert_eq!(report.entry_rows.len(), 6);
        assert_eq!(report.capability_rows.len(), 6);
        assert_eq!(report.composition_rows.len(), 2);
    }

    #[test]
    fn starter_plugin_catalog_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_starter_plugin_catalog_report().expect("report");
        let committed: TassadarPostArticleStarterPluginCatalogReport =
            read_json(tassadar_post_article_starter_plugin_catalog_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_starter_plugin_catalog_report_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_post_article_starter_plugin_catalog_report.json");
        let written =
            write_tassadar_post_article_starter_plugin_catalog_report(&output_path).expect("write");
        let roundtrip: TassadarPostArticleStarterPluginCatalogReport =
            read_json(&output_path).expect("roundtrip");

        assert_eq!(written, roundtrip);
        assert_eq!(
            tassadar_post_article_starter_plugin_catalog_report_path()
                .strip_prefix(super::repo_root())
                .expect("starter catalog report should live under repo root")
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_REPORT_REF
        );
    }
}
