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
    build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundle,
    TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-plugin-runtime-api-and-engine-abstraction.sh";

const PACKET_ABI_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_packet_abi_and_rust_pdk_report.json";
const IMPORT_POLICY_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_import_policy_matrix_report.json";
const ASYNC_LIFECYCLE_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_async_lifecycle_profile_report.json";
const SIMULATOR_EFFECT_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_simulator_effect_profile_report.json";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginRuntimeApiAndEngineAbstractionStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginRuntimeApiDependencyClass {
    CanonicalPrecedent,
    RuntimePolicy,
    LifecyclePrecedent,
    EffectBoundary,
    DesignInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeApiMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub packet_abi_report_id: String,
    pub packet_abi_report_digest: String,
    pub packet_abi_version: String,
    pub rust_first_pdk_id: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub runtime_bundle_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeApiDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticlePluginRuntimeApiDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeApiRow {
    pub runtime_api_id: String,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeEngineRow {
    pub engine_operation_id: String,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeBoundReportRow {
    pub bound_id: String,
    pub required: bool,
    pub model_visible: bool,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeSignalBoundaryRow {
    pub signal_id: String,
    pub model_visible: bool,
    pub fixed_runtime_contract: bool,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeFailureIsolationReportRow {
    pub isolation_id: String,
    pub failure_scope_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeApiValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub packet_abi_report_ref: String,
    pub import_policy_matrix_report_ref: String,
    pub async_lifecycle_profile_report_ref: String,
    pub simulator_effect_profile_report_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding: TassadarPostArticlePluginRuntimeApiMachineIdentityBinding,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundle,
    pub dependency_rows: Vec<TassadarPostArticlePluginRuntimeApiDependencyRow>,
    pub runtime_api_rows: Vec<TassadarPostArticlePluginRuntimeApiRow>,
    pub engine_rows: Vec<TassadarPostArticlePluginRuntimeEngineRow>,
    pub bound_rows: Vec<TassadarPostArticlePluginRuntimeBoundReportRow>,
    pub signal_boundary_rows: Vec<TassadarPostArticlePluginRuntimeSignalBoundaryRow>,
    pub failure_isolation_rows: Vec<TassadarPostArticlePluginRuntimeFailureIsolationReportRow>,
    pub validation_rows: Vec<TassadarPostArticlePluginRuntimeApiValidationRow>,
    pub contract_status: TassadarPostArticlePluginRuntimeApiAndEngineAbstractionStatus,
    pub contract_green: bool,
    pub operator_internal_only_posture: bool,
    pub runtime_api_frozen: bool,
    pub engine_abstraction_frozen: bool,
    pub artifact_digest_verification_required: bool,
    pub runtime_bounds_frozen: bool,
    pub model_information_boundary_frozen: bool,
    pub logical_time_control_neutral: bool,
    pub wall_time_control_neutral: bool,
    pub cost_model_invariance_required: bool,
    pub scheduling_semantics_frozen: bool,
    pub failure_domain_isolation_frozen: bool,
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
pub enum TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError {
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

pub fn build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report() -> Result<
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReport,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError,
> {
    let packet_abi: PacketAbiFixture = read_repo_json(PACKET_ABI_REPORT_REF)?;
    let import_policy: ImportPolicyMatrixFixture = read_repo_json(IMPORT_POLICY_MATRIX_REPORT_REF)?;
    let async_lifecycle: AsyncLifecycleProfileFixture =
        read_repo_json(ASYNC_LIFECYCLE_PROFILE_REPORT_REF)?;
    let simulator_effect: SimulatorEffectProfileFixture =
        read_repo_json(SIMULATOR_EFFECT_PROFILE_REPORT_REF)?;
    let runtime_bundle =
        build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle();

    let packet_abi_dependency_closed = packet_abi.packet_abi_frozen
        && packet_abi.rust_first_pdk_frozen
        && packet_abi.deferred_issue_ids.is_empty();
    let import_policy_boundary_closed = import_policy
        .policy_matrix
        .entries
        .iter()
        .any(|entry| entry.execution_boundary == "internal_only")
        && import_policy
            .policy_matrix
            .entries
            .iter()
            .any(|entry| entry.execution_boundary == "sandbox_delegation_only")
        && import_policy
            .policy_matrix
            .entries
            .iter()
            .any(|entry| entry.execution_boundary == "refused");
    let async_cancel_timeout_precedent = async_lifecycle.overall_green
        && async_lifecycle
            .routeable_lifecycle_surface_ids
            .iter()
            .any(|surface| surface == "safe_boundary_cancellation_job")
        && async_lifecycle
            .routeable_lifecycle_surface_ids
            .iter()
            .any(|surface| surface == "retryable_timeout_search_job");
    let simulator_ambient_effects_refused = !simulator_effect.served_publication_allowed
        && simulator_effect
            .refused_effect_ids
            .iter()
            .any(|effect| effect == "host.clock.now")
        && simulator_effect
            .refused_effect_ids
            .iter()
            .any(|effect| effect == "host.random.os_entropy")
        && simulator_effect
            .refused_effect_ids
            .iter()
            .any(|effect| effect == "host.network.socket_io");

    let machine_identity_binding = TassadarPostArticlePluginRuntimeApiMachineIdentityBinding {
        machine_identity_id: packet_abi
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: packet_abi.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: packet_abi.machine_identity_binding.canonical_route_id.clone(),
        canonical_route_descriptor_digest: packet_abi
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: packet_abi
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: packet_abi
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: packet_abi
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: packet_abi
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: packet_abi
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        packet_abi_report_id: packet_abi.report_id.clone(),
        packet_abi_report_digest: packet_abi.report_digest.clone(),
        packet_abi_version: packet_abi
            .machine_identity_binding
            .packet_abi_version
            .clone(),
        rust_first_pdk_id: packet_abi
            .machine_identity_binding
            .rust_first_pdk_id
            .clone(),
        host_owned_runtime_api_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID),
        engine_abstraction_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID),
        runtime_bundle_id: runtime_bundle.bundle_id.clone(),
        runtime_bundle_digest: runtime_bundle.bundle_digest.clone(),
        runtime_bundle_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
        ),
        detail: String::from(
            "the host-owned runtime API stays bound to the same canonical post-TAS-186 machine identity, canonical route, computational-model statement, and packet ABI contract as the earlier plugin tranche.",
        ),
    };

    let dependency_rows = vec![
        dependency_row(
            "packet_abi_contract_closed",
            TassadarPostArticlePluginRuntimeApiDependencyClass::CanonicalPrecedent,
            packet_abi_dependency_closed,
            PACKET_ABI_REPORT_REF,
            Some(packet_abi.report_id.clone()),
            Some(packet_abi.report_digest.clone()),
            "the packet ABI and Rust-first guest surface must already be frozen before the runtime API can claim a stable host-owned layer above them.",
        ),
        dependency_row(
            "import_policy_matrix_bound",
            TassadarPostArticlePluginRuntimeApiDependencyClass::RuntimePolicy,
            import_policy_boundary_closed,
            IMPORT_POLICY_MATRIX_REPORT_REF,
            Some(import_policy.report_id.clone()),
            Some(import_policy.report_digest.clone()),
            "import policy precedent keeps deterministic internal-only imports, sandbox delegation, and refused side effects explicit for the runtime contract.",
        ),
        dependency_row(
            "async_lifecycle_precedent_bound",
            TassadarPostArticlePluginRuntimeApiDependencyClass::LifecyclePrecedent,
            async_cancel_timeout_precedent,
            ASYNC_LIFECYCLE_PROFILE_REPORT_REF,
            Some(async_lifecycle.report_id.clone()),
            Some(async_lifecycle.report_digest.clone()),
            "async lifecycle precedent keeps retry, timeout, and cancellation semantics bounded before the plugin runtime claims them.",
        ),
        dependency_row(
            "simulator_effect_boundary_bound",
            TassadarPostArticlePluginRuntimeApiDependencyClass::EffectBoundary,
            simulator_ambient_effects_refused,
            SIMULATOR_EFFECT_PROFILE_REPORT_REF,
            Some(simulator_effect.report_id.clone()),
            Some(simulator_effect.report_digest.clone()),
            "simulator-effect precedent keeps ambient clock, entropy, and socket I/O outside the runtime contract.",
        ),
        dependency_row(
            "plugin_system_runtime_shape_cited",
            TassadarPostArticlePluginRuntimeApiDependencyClass::DesignInput,
            true,
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the local plugin-system note states the host-owned runtime API shape, engine abstraction, queue/pool/cancel semantics, and explicit conformance checks this contract is freezing.",
        ),
    ];

    let runtime_api_rows = vec![
        runtime_api_row(
            "artifact_load_declared_only",
            runtime_bundle
                .artifact_loading_rows
                .iter()
                .any(|row| row.row_id == "load_declared_plugin_artifact" && row.green),
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
                PACKET_ABI_REPORT_REF,
            ],
            "runtime loads only declared plugin artifacts and linked bundle members instead of arbitrary host binaries.",
        ),
        runtime_api_row(
            "artifact_digest_verification_pre_instantiate",
            runtime_bundle.artifact_loading_rows.iter().any(|row| {
                row.row_id == "verify_artifact_digest_before_instantiate" && row.green
            }),
            &[TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF],
            "artifact digest verification happens before instantiate or pool reuse admission.",
        ),
        runtime_api_row(
            "load_invoke_cancel_surface_host_owned",
            runtime_bundle.engine_operation_rows.iter().any(|row| {
                row.operation_id == "instantiate_artifact" && row.host_owned_surface_id == "load_plugin"
            }) && runtime_bundle.engine_operation_rows.iter().any(|row| {
                row.operation_id == "invoke_export" && row.host_owned_surface_id == "invoke"
            }) && runtime_bundle.engine_operation_rows.iter().any(|row| {
                row.operation_id == "cancel_running_execution" && row.host_owned_surface_id == "cancel"
            }),
            &[TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF],
            "instantiate, invoke, and cancel remain behind one host-owned runtime API.",
        ),
        runtime_api_row(
            "mount_capabilities_is_runtime_owned",
            runtime_bundle
                .engine_operation_rows
                .iter()
                .any(|row| row.operation_id == "mount_capabilities" && row.green),
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
                PACKET_ABI_REPORT_REF,
            ],
            "capability mounting remains runtime-owned and mount-envelope mediated.",
        ),
        runtime_api_row(
            "usage_summary_receipts_required",
            runtime_bundle
                .engine_operation_rows
                .iter()
                .any(|row| row.operation_id == "collect_usage_summary" && row.green),
            &[TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF],
            "every runtime outcome carries an explicit usage summary and receipt field set.",
        ),
        runtime_api_row(
            "pool_reuse_below_contract",
            runtime_bundle
                .engine_operation_rows
                .iter()
                .any(|row| row.operation_id == "pool_instance_reuse" && row.green),
            &[TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF],
            "pool reuse remains a runtime-owned optimization below the top-level contract.",
        ),
        runtime_api_row(
            "typed_timeout_and_memory_outcomes",
            runtime_bundle
                .case_receipts
                .iter()
                .any(|case| case.outcome_id == "runtime_timeout")
                && runtime_bundle
                    .case_receipts
                    .iter()
                    .any(|case| case.outcome_id == "runtime_memory_limit"),
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
                ASYNC_LIFECYCLE_PROFILE_REPORT_REF,
            ],
            "timeout and memory overflow stay typed outcomes instead of silent degradation.",
        ),
        runtime_api_row(
            "failure_isolation_scopes_explicit",
            runtime_bundle.failure_isolation_rows.len() == 3
                && runtime_bundle
                    .failure_isolation_rows
                    .iter()
                    .all(|row| row.green),
            &[TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF],
            "per-plugin, per-step, and per-workflow failure scopes stay explicit.",
        ),
    ];

    let engine_rows = runtime_bundle
        .engine_operation_rows
        .iter()
        .map(|row| TassadarPostArticlePluginRuntimeEngineRow {
            engine_operation_id: row.operation_id.clone(),
            current_posture: String::from("host_owned_backend_neutral"),
            green: row.green && row.backend_specific_types_hidden,
            source_refs: vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
            )],
            detail: row.detail.clone(),
        })
        .collect::<Vec<_>>();

    let bound_rows = runtime_bundle
        .bound_rows
        .iter()
        .map(|row| TassadarPostArticlePluginRuntimeBoundReportRow {
            bound_id: row.bound_id.clone(),
            required: row.required,
            model_visible: row.model_visible,
            green: row.green,
            source_refs: vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
                ),
                String::from(ASYNC_LIFECYCLE_PROFILE_REPORT_REF),
            ],
            detail: row.detail.clone(),
        })
        .collect::<Vec<_>>();

    let signal_boundary_rows = runtime_bundle
        .signal_rows
        .iter()
        .map(|row| TassadarPostArticlePluginRuntimeSignalBoundaryRow {
            signal_id: row.signal_id.clone(),
            model_visible: row.model_visible,
            fixed_runtime_contract: row.fixed_runtime_contract,
            green: row.green,
            source_refs: vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
            )],
            detail: row.detail.clone(),
        })
        .collect::<Vec<_>>();

    let failure_isolation_rows = runtime_bundle
        .failure_isolation_rows
        .iter()
        .map(
            |row| TassadarPostArticlePluginRuntimeFailureIsolationReportRow {
                isolation_id: row.isolation_id.clone(),
                failure_scope_id: row.failure_scope_id.clone(),
                green: row.green,
                source_refs: vec![String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
                )],
                detail: row.detail.clone(),
            },
        )
        .collect::<Vec<_>>();

    let queue_depth_hidden = signal_boundary_rows
        .iter()
        .find(|row| row.signal_id == "queue_depth")
        .map(|row| !row.model_visible && row.fixed_runtime_contract && row.green)
        .unwrap_or(false);
    let runtime_cost_hidden = signal_boundary_rows
        .iter()
        .find(|row| row.signal_id == "runtime_cost_class")
        .map(|row| !row.model_visible && row.fixed_runtime_contract && row.green)
        .unwrap_or(false);
    let retry_hidden = signal_boundary_rows
        .iter()
        .find(|row| row.signal_id == "retry_budget")
        .map(|row| !row.model_visible && row.fixed_runtime_contract && row.green)
        .unwrap_or(false);
    let backend_types_hidden = engine_rows.iter().all(|row| row.green);

    let validation_rows = vec![
        validation_row(
            "packet_abi_dependency_closed",
            packet_abi_dependency_closed,
            &[PACKET_ABI_REPORT_REF],
            "the earlier packet ABI contract no longer defers TAS-200 and remains frozen.",
        ),
        validation_row(
            "ambient_imports_stay_blocked",
            import_policy_boundary_closed,
            &[IMPORT_POLICY_MATRIX_REPORT_REF],
            "import-policy precedent still keeps deterministic internal-only imports separate from sandbox delegation and refused side effects.",
        ),
        validation_row(
            "cancel_timeout_precedent_bound",
            async_cancel_timeout_precedent,
            &[ASYNC_LIFECYCLE_PROFILE_REPORT_REF],
            "async-lifecycle precedent still keeps retry, timeout, and safe-boundary cancellation explicit.",
        ),
        validation_row(
            "ambient_effects_stay_refused",
            simulator_ambient_effects_refused,
            &[SIMULATOR_EFFECT_PROFILE_REPORT_REF],
            "simulator-effect precedent still refuses ambient clock, entropy, and socket I/O.",
        ),
        validation_row(
            "queue_depth_hidden_from_model",
            queue_depth_hidden,
            &[TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF],
            "queue depth remains runtime-owned and hidden from the model.",
        ),
        validation_row(
            "runtime_cost_hidden_from_model",
            runtime_cost_hidden && retry_hidden,
            &[TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF],
            "runtime cost and retries remain hidden from the model.",
        ),
        validation_row(
            "backend_specific_types_hidden",
            backend_types_hidden,
            &[TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF],
            "backend-specific types stay below the host-owned runtime API.",
        ),
        validation_row(
            "overclaim_posture_blocked",
            true,
            &[
                PACKET_ABI_REPORT_REF,
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
            ],
            "the runtime contract stays operator/internal-only and does not imply weighted plugin control, publication rights, or public software capability.",
        ),
    ];

    let runtime_api_frozen = runtime_api_rows.iter().all(|row| row.green);
    let engine_abstraction_frozen = engine_rows.iter().all(|row| row.green);
    let artifact_digest_verification_required = runtime_api_rows.iter().any(|row| {
        row.runtime_api_id == "artifact_digest_verification_pre_instantiate" && row.green
    });
    let runtime_bounds_frozen = bound_rows.iter().filter(|row| row.required).count() >= 5
        && bound_rows.iter().all(|row| row.green);
    let model_information_boundary_frozen = signal_boundary_rows
        .iter()
        .filter(|row| !row.model_visible)
        .count()
        >= 6
        && signal_boundary_rows.iter().all(|row| row.green);
    let logical_time_control_neutral = !runtime_bundle.time_semantics.logical_time_model_observable;
    let wall_time_control_neutral = !runtime_bundle.time_semantics.wall_time_model_observable;
    let cost_model_invariance_required =
        runtime_bundle.time_semantics.cost_model_invariance_required;
    let scheduling_semantics_frozen = runtime_bundle.time_semantics.scheduling_semantics_fixed;
    let failure_domain_isolation_frozen =
        failure_isolation_rows.iter().all(|row| row.green) && failure_isolation_rows.len() == 3;
    let operator_internal_only_posture = packet_abi.operator_internal_only_posture
        && packet_abi.rebase_claim_allowed
        && !packet_abi.plugin_capability_claim_allowed
        && !packet_abi.plugin_publication_allowed;
    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && runtime_api_frozen
        && engine_abstraction_frozen
        && artifact_digest_verification_required
        && runtime_bounds_frozen
        && model_information_boundary_frozen
        && logical_time_control_neutral
        && wall_time_control_neutral
        && cost_model_invariance_required
        && scheduling_semantics_frozen
        && failure_domain_isolation_frozen
        && validation_rows.iter().all(|row| row.green)
        && operator_internal_only_posture;

    let contract_status = if contract_green {
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionStatus::Green
    } else {
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionStatus::Incomplete
    };
    let rebase_claim_allowed = contract_green;

    let mut report = TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_plugin_runtime_api_and_engine_abstraction.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_CHECKER_REF,
        ),
        packet_abi_report_ref: String::from(PACKET_ABI_REPORT_REF),
        import_policy_matrix_report_ref: String::from(IMPORT_POLICY_MATRIX_REPORT_REF),
        async_lifecycle_profile_report_ref: String::from(ASYNC_LIFECYCLE_PROFILE_REPORT_REF),
        simulator_effect_profile_report_ref: String::from(SIMULATOR_EFFECT_PROFILE_REPORT_REF),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        supporting_material_refs: vec![
            String::from(PACKET_ABI_REPORT_REF),
            String::from(IMPORT_POLICY_MATRIX_REPORT_REF),
            String::from(ASYNC_LIFECYCLE_PROFILE_REPORT_REF),
            String::from(SIMULATOR_EFFECT_PROFILE_REPORT_REF),
            String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        ],
        machine_identity_binding,
        runtime_bundle_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
        ),
        runtime_bundle,
        dependency_rows,
        runtime_api_rows,
        engine_rows,
        bound_rows,
        signal_boundary_rows,
        failure_isolation_rows,
        validation_rows,
        contract_status,
        contract_green,
        operator_internal_only_posture,
        runtime_api_frozen,
        engine_abstraction_frozen,
        artifact_digest_verification_required,
        runtime_bounds_frozen,
        model_information_boundary_frozen,
        logical_time_control_neutral,
        wall_time_control_neutral,
        cost_model_invariance_required,
        scheduling_semantics_frozen,
        failure_domain_isolation_frozen,
        rebase_claim_allowed,
        plugin_capability_claim_allowed: false,
        weighted_plugin_control_allowed: false,
        plugin_publication_allowed: false,
        served_public_universality_allowed: false,
        arbitrary_software_capability_allowed: false,
        deferred_issue_ids: Vec::new(),
        claim_boundary: String::from(
            "this report freezes one host-owned plugin runtime API and engine abstraction above the packet ABI, import-policy, async-lifecycle, and simulator-effect precedents. It keeps digest-verified loading, bounded instantiate/invoke/mount/cancel semantics, bounded queue/pool/timeout/memory/concurrency ceilings, hidden runtime cost and queue signals, fixed scheduling semantics, and explicit failure isolation machine-readable while keeping weighted plugin control, plugin publication, served/public universality, and arbitrary software capability blocked.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article plugin runtime API report keeps contract_status={:?}, dependency_rows={}, runtime_api_rows={}, engine_rows={}, bound_rows={}, signal_rows={}, validation_rows={}, and deferred_issue_ids={}.",
        report.contract_status,
        report.dependency_rows.len(),
        report.runtime_api_rows.len(),
        report.engine_rows.len(),
        report.bound_rows.len(),
        report.signal_boundary_rows.len(),
        report.validation_rows.len(),
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF)
}

pub fn write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReport,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[derive(Clone, Debug, Deserialize)]
struct PacketAbiFixture {
    report_id: String,
    report_digest: String,
    operator_internal_only_posture: bool,
    packet_abi_frozen: bool,
    rust_first_pdk_frozen: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    plugin_publication_allowed: bool,
    deferred_issue_ids: Vec<String>,
    machine_identity_binding: PacketAbiMachineIdentityBindingFixture,
}

#[derive(Clone, Debug, Deserialize)]
struct PacketAbiMachineIdentityBindingFixture {
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
    rust_first_pdk_id: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ImportPolicyMatrixFixture {
    report_id: String,
    report_digest: String,
    policy_matrix: ImportPolicyMatrix,
}

#[derive(Clone, Debug, Deserialize)]
struct ImportPolicyMatrix {
    entries: Vec<ImportPolicyMatrixEntry>,
}

#[derive(Clone, Debug, Deserialize)]
struct ImportPolicyMatrixEntry {
    execution_boundary: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AsyncLifecycleProfileFixture {
    report_id: String,
    report_digest: String,
    overall_green: bool,
    routeable_lifecycle_surface_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct SimulatorEffectProfileFixture {
    report_id: String,
    report_digest: String,
    refused_effect_ids: Vec<String>,
    served_publication_allowed: bool,
}

fn dependency_row(
    dependency_id: &str,
    dependency_class: TassadarPostArticlePluginRuntimeApiDependencyClass,
    satisfied: bool,
    source_ref: &str,
    bound_report_id: Option<String>,
    bound_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticlePluginRuntimeApiDependencyRow {
    TassadarPostArticlePluginRuntimeApiDependencyRow {
        dependency_id: String::from(dependency_id),
        dependency_class,
        satisfied,
        source_ref: String::from(source_ref),
        bound_report_id,
        bound_report_digest,
        detail: String::from(detail),
    }
}

fn runtime_api_row(
    runtime_api_id: &str,
    green: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticlePluginRuntimeApiRow {
    TassadarPostArticlePluginRuntimeApiRow {
        runtime_api_id: String::from(runtime_api_id),
        current_posture: if green {
            String::from("frozen")
        } else {
            String::from("incomplete")
        },
        green,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticlePluginRuntimeApiValidationRow {
    TassadarPostArticlePluginRuntimeApiValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-sandbox should live under <repo>/crates/psionic-sandbox")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report, read_json,
        tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report_path,
        write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report,
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReport,
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionStatus,
        TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
    };

    #[test]
    fn post_article_plugin_runtime_api_report_keeps_bounds_and_claims_explicit() {
        let report = build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report()
            .expect("report");

        assert_eq!(
            report.contract_status,
            TassadarPostArticlePluginRuntimeApiAndEngineAbstractionStatus::Green
        );
        assert_eq!(report.dependency_rows.len(), 5);
        assert_eq!(report.runtime_api_rows.len(), 8);
        assert_eq!(report.engine_rows.len(), 6);
        assert_eq!(report.bound_rows.len(), 6);
        assert_eq!(report.signal_boundary_rows.len(), 8);
        assert_eq!(report.failure_isolation_rows.len(), 3);
        assert_eq!(report.validation_rows.len(), 8);
        assert!(report.deferred_issue_ids.is_empty());
        assert!(report.contract_green);
        assert!(report.operator_internal_only_posture);
        assert!(report.runtime_api_frozen);
        assert!(report.engine_abstraction_frozen);
        assert!(report.artifact_digest_verification_required);
        assert!(report.runtime_bounds_frozen);
        assert!(report.model_information_boundary_frozen);
        assert!(report.logical_time_control_neutral);
        assert!(report.wall_time_control_neutral);
        assert!(report.cost_model_invariance_required);
        assert!(report.scheduling_semantics_frozen);
        assert!(report.failure_domain_isolation_frozen);
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_plugin_runtime_api_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report()
                .expect("report");
        let committed: TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReport = read_json(
            tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report_path(),
        )
        .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report_path()
                .strip_prefix(super::repo_root())
                .expect("relative report path")
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF
        );
    }

    #[test]
    fn write_post_article_plugin_runtime_api_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report.json");
        let written = write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report(
            &output_path,
        )
        .expect("write report");
        let persisted: TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
