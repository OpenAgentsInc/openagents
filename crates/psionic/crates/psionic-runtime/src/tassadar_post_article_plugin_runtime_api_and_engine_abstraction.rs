use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION;

pub const TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_runtime_api_and_engine_abstraction_v1/tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_runtime_api_and_engine_abstraction_v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID: &str =
    "tassadar.plugin_runtime.host_owned_api.v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID: &str =
    "tassadar.plugin_runtime.engine_abstraction.v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_PROFILE_ID: &str =
    "tassadar.plugin_runtime.engine_profile.wasm_bounded.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginRuntimeApiCaseStatus {
    ExactSuccess,
    ExactTypedRefusal,
    ExactRuntimeFailure,
    ExactCancellation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeArtifactLoadingRow {
    pub row_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeEngineOperationRow {
    pub operation_id: String,
    pub host_owned_surface_id: String,
    pub backend_specific_types_hidden: bool,
    pub model_visible: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeBoundRow {
    pub bound_id: String,
    pub required: bool,
    pub model_visible: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeSignalRow {
    pub signal_id: String,
    pub model_visible: bool,
    pub fixed_runtime_contract: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeTimeSemantics {
    pub logical_time_semantics_id: String,
    pub logical_time_model_observable: bool,
    pub wall_time_semantics_id: String,
    pub wall_time_model_observable: bool,
    pub queue_depth_model_observable: bool,
    pub retry_budget_model_observable: bool,
    pub runtime_cost_model_observable: bool,
    pub cost_model_invariance_required: bool,
    pub scheduling_semantics_fixed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeFailureIsolationRow {
    pub isolation_id: String,
    pub failure_scope_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeUsageSummary {
    pub logical_cpu_millis: u32,
    pub guest_memory_bytes: u64,
    pub queue_wait_millis: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel_consumed: Option<u64>,
    pub pool_reuse: bool,
    pub emitted_receipt_field_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeCaseReceipt {
    pub case_id: String,
    pub lifecycle_surface_id: String,
    pub status: TassadarPostArticlePluginRuntimeApiCaseStatus,
    pub invocation_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub artifact_digest: String,
    pub export_name: String,
    pub packet_abi_version: String,
    pub mounted_capability_namespace_ids: Vec<String>,
    pub visible_signal_ids: Vec<String>,
    pub hidden_signal_ids: Vec<String>,
    pub replay_posture_id: String,
    pub outcome_id: String,
    pub usage_summary: TassadarPostArticlePluginRuntimeUsageSummary,
    pub note: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub engine_profile_id: String,
    pub packet_abi_version: String,
    pub artifact_loading_rows: Vec<TassadarPostArticlePluginRuntimeArtifactLoadingRow>,
    pub engine_operation_rows: Vec<TassadarPostArticlePluginRuntimeEngineOperationRow>,
    pub bound_rows: Vec<TassadarPostArticlePluginRuntimeBoundRow>,
    pub signal_rows: Vec<TassadarPostArticlePluginRuntimeSignalRow>,
    pub time_semantics: TassadarPostArticlePluginRuntimeTimeSemantics,
    pub failure_isolation_rows: Vec<TassadarPostArticlePluginRuntimeFailureIsolationRow>,
    pub case_receipts: Vec<TassadarPostArticlePluginRuntimeCaseReceipt>,
    pub exact_success_case_count: u32,
    pub exact_typed_refusal_case_count: u32,
    pub exact_runtime_failure_case_count: u32,
    pub exact_cancellation_case_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundleError {
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
pub fn build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle(
) -> TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundle {
    let artifact_loading_rows = vec![
        artifact_loading_row(
            "load_declared_plugin_artifact",
            "runtime loads only the manifest-declared plugin artifact and linked bundle members instead of accepting ad hoc binaries.",
        ),
        artifact_loading_row(
            "verify_artifact_digest_before_instantiate",
            "artifact digest verification is required before instantiate or pool admission so runtime identity stays machine-checkable.",
        ),
        artifact_loading_row(
            "freeze_linked_bundle_membership",
            "linked multi-module packaging stays bound to one stable bundle identity instead of late host-side composition.",
        ),
    ];
    let engine_operation_rows = vec![
        engine_operation_row(
            "instantiate_artifact",
            "load_plugin",
            false,
            "instantiate stays behind the host-owned runtime API and does not leak backend-specific engine handles.",
        ),
        engine_operation_row(
            "invoke_export",
            "invoke",
            false,
            "export invocation stays packet-shaped behind the host-owned runtime API.",
        ),
        engine_operation_row(
            "mount_capabilities",
            "invoke",
            false,
            "capability mounting remains runtime-owned and mount-envelope mediated rather than guest-driven.",
        ),
        engine_operation_row(
            "cancel_running_execution",
            "cancel",
            false,
            "cancellation remains an explicit runtime API action instead of backend-specific interruption wiring.",
        ),
        engine_operation_row(
            "collect_usage_summary",
            "invoke",
            false,
            "usage collection stays receipt-bound and host-owned instead of a backend-private side channel.",
        ),
        engine_operation_row(
            "pool_instance_reuse",
            "load_plugin",
            false,
            "pool reuse stays a runtime-owned optimization with no backend-specific surface leaked to the top-level contract.",
        ),
    ];
    let bound_rows = vec![
        bound_row(
            "timeout_bound",
            true,
            false,
            "every invocation runs under an explicit timeout ceiling and timeout overflow resolves to a typed runtime failure.",
        ),
        bound_row(
            "memory_bound",
            true,
            false,
            "every invocation runs under an explicit memory ceiling and memory overflow resolves to a typed runtime failure.",
        ),
        bound_row(
            "queue_bound",
            true,
            false,
            "queue depth remains bounded and saturation resolves to a typed refusal instead of unbounded waiting.",
        ),
        bound_row(
            "pool_bound",
            true,
            false,
            "per-plugin pool size remains bounded and the runtime never relies on unbounded instance spawning.",
        ),
        bound_row(
            "concurrency_bound",
            true,
            false,
            "concurrency slot count remains runtime-owned and fixed per admitted runtime profile.",
        ),
        bound_row(
            "fuel_bound_optional",
            false,
            false,
            "instruction fuel remains an optional extra ceiling that can be enabled without changing the host-owned runtime API shape.",
        ),
    ];
    let signal_rows = vec![
        signal_row(
            "mount_envelope_identity",
            true,
            true,
            "the guest may observe the resolved mount-envelope identity because capability selection must remain explicit.",
        ),
        signal_row(
            "mounted_capability_namespace_ids",
            true,
            true,
            "the guest may observe which capability namespaces were mounted because mount scope is part of the bounded packet contract.",
        ),
        signal_row(
            "queue_depth",
            false,
            true,
            "queue depth stays hidden from the model and is fixed runtime contract rather than a control signal.",
        ),
        signal_row(
            "retry_budget",
            false,
            true,
            "retry policy remains runtime-owned and hidden from the model until a later receipt or replay contract freezes it explicitly.",
        ),
        signal_row(
            "runtime_cost_class",
            false,
            true,
            "runtime cost remains a hidden invariant instead of a model-visible optimization signal.",
        ),
        signal_row(
            "timeout_budget_class",
            false,
            true,
            "timeout budgets remain runtime-owned and are surfaced only by typed outcomes and receipts.",
        ),
        signal_row(
            "memory_limit_class",
            false,
            true,
            "memory ceilings remain runtime-owned and are surfaced only by typed outcomes and receipts.",
        ),
        signal_row(
            "wall_clock_elapsed",
            false,
            true,
            "wall-clock time stays hidden and control-neutral inside the host-owned runtime contract.",
        ),
    ];
    let time_semantics = TassadarPostArticlePluginRuntimeTimeSemantics {
        logical_time_semantics_id: String::from("logical_deadline_budget_class"),
        logical_time_model_observable: false,
        wall_time_semantics_id: String::from("wall_clock_hidden_abort_budget"),
        wall_time_model_observable: false,
        queue_depth_model_observable: false,
        retry_budget_model_observable: false,
        runtime_cost_model_observable: false,
        cost_model_invariance_required: true,
        scheduling_semantics_fixed: true,
        detail: String::from(
            "logical time and wall time stay control-neutral inside the runtime contract; queue depth, retries, and runtime cost remain hidden, while cost-model invariance and fixed scheduling semantics stay mandatory across equivalent choices.",
        ),
    };
    let failure_isolation_rows = vec![
        failure_isolation_row(
            "per_plugin_failure_isolation",
            "plugin_instance",
            "one plugin instance failure must not silently widen or corrupt another admitted plugin artifact.",
        ),
        failure_isolation_row(
            "per_step_failure_isolation",
            "workflow_step",
            "one plugin step failure must remain local to that invocation outcome and be carried by typed receipt state.",
        ),
        failure_isolation_row(
            "per_workflow_failure_summary",
            "workflow",
            "workflow-level failure aggregation stays outside the plugin engine and is projected by explicit receipt facts rather than hidden host control flow.",
        ),
    ];
    let case_receipts = vec![
        case_receipt(
            "load_verify_invoke_success",
            "load_instantiate_invoke",
            TassadarPostArticlePluginRuntimeApiCaseStatus::ExactSuccess,
            "inv.plugin.echo.0001",
            "plugin.echo",
            "1.0.0",
            "sha256:1171cc0b2f52da94e06f0b5c8c67872ba8c19fc97efec4a94ef39966a1959d83",
            "handle_packet",
            &["artifact.read_only"],
            &["mount_envelope_identity", "mounted_capability_namespace_ids"],
            &[
                "queue_depth",
                "retry_budget",
                "runtime_cost_class",
                "timeout_budget_class",
                "memory_limit_class",
                "wall_clock_elapsed",
            ],
            "deterministic_receipt_replay",
            "success",
            usage_summary(
                3,
                1_572_864,
                0,
                Some(17_920),
                false,
                &[
                    "invocation_identity_digest",
                    "input_packet_digest",
                    "output_packet_digest",
                ],
            ),
            "load, digest verification, instantiate, and invoke remain host-owned and receipt-bound for the canonical success path.",
        ),
        case_receipt(
            "warm_pool_reuse_success",
            "warm_pool_invoke",
            TassadarPostArticlePluginRuntimeApiCaseStatus::ExactSuccess,
            "inv.plugin.artifact_probe.0002",
            "plugin.artifact_probe",
            "1.0.0",
            "sha256:e0dc8d15af2ec6c30f20ab7eca81cc4f01bfc35fece43ecb1ce42d672b31718f",
            "handle_packet",
            &["artifact.read_only", "capability.artifact_probe"],
            &["mount_envelope_identity", "mounted_capability_namespace_ids"],
            &[
                "queue_depth",
                "retry_budget",
                "runtime_cost_class",
                "timeout_budget_class",
                "memory_limit_class",
                "wall_clock_elapsed",
            ],
            "deterministic_receipt_replay",
            "success",
            usage_summary(
                2,
                1_851_392,
                1,
                Some(14_208),
                true,
                &[
                    "invocation_identity_digest",
                    "input_packet_digest",
                    "output_packet_digest",
                ],
            ),
            "pool reuse stays a runtime-owned optimization and does not widen the top-level API surface.",
        ),
        case_receipt(
            "queue_saturation_refusal",
            "queue_admission",
            TassadarPostArticlePluginRuntimeApiCaseStatus::ExactTypedRefusal,
            "inv.plugin.queue.0003",
            "plugin.echo",
            "1.0.0",
            "sha256:1171cc0b2f52da94e06f0b5c8c67872ba8c19fc97efec4a94ef39966a1959d83",
            "handle_packet",
            &["artifact.read_only"],
            &["mount_envelope_identity", "mounted_capability_namespace_ids"],
            &[
                "queue_depth",
                "retry_budget",
                "runtime_cost_class",
                "timeout_budget_class",
                "memory_limit_class",
                "wall_clock_elapsed",
            ],
            "deterministic_receipt_replay",
            "queue_saturation",
            usage_summary(
                0,
                0,
                4,
                None,
                false,
                &[
                    "invocation_identity_digest",
                    "input_packet_digest",
                    "typed_refusal_id",
                ],
            ),
            "bounded queue saturation remains an explicit refusal instead of unbounded buffering or host-side retry.",
        ),
        case_receipt(
            "timeout_abort_failure",
            "timeout_abort",
            TassadarPostArticlePluginRuntimeApiCaseStatus::ExactRuntimeFailure,
            "inv.plugin.timeout.0004",
            "plugin.long_running",
            "1.0.0",
            "sha256:3189aefc1d7476c4cbcf4f6db0cf5011efcd2c322a84dad8560a72186cb0a2ef",
            "handle_packet",
            &["artifact.read_only"],
            &["mount_envelope_identity", "mounted_capability_namespace_ids"],
            &[
                "queue_depth",
                "retry_budget",
                "runtime_cost_class",
                "timeout_budget_class",
                "memory_limit_class",
                "wall_clock_elapsed",
            ],
            "snapshot_based_failure_replay",
            "runtime_timeout",
            usage_summary(
                25,
                2_359_296,
                0,
                Some(131_072),
                false,
                &[
                    "invocation_identity_digest",
                    "input_packet_digest",
                    "host_error_id",
                ],
            ),
            "timeout overflow remains a typed runtime failure with no hidden downgrade or extended wall-clock continuation.",
        ),
        case_receipt(
            "memory_limit_failure",
            "memory_abort",
            TassadarPostArticlePluginRuntimeApiCaseStatus::ExactRuntimeFailure,
            "inv.plugin.memory.0005",
            "plugin.memory_probe",
            "1.0.0",
            "sha256:ae6713a1b2208d9363565108ba8d5b005402f176c0ff76e709e26d932855db02",
            "handle_packet",
            &["artifact.read_only", "capability.memory_probe"],
            &["mount_envelope_identity", "mounted_capability_namespace_ids"],
            &[
                "queue_depth",
                "retry_budget",
                "runtime_cost_class",
                "timeout_budget_class",
                "memory_limit_class",
                "wall_clock_elapsed",
            ],
            "snapshot_based_failure_replay",
            "runtime_memory_limit",
            usage_summary(
                9,
                8_388_608,
                0,
                Some(49_152),
                false,
                &[
                    "invocation_identity_digest",
                    "input_packet_digest",
                    "host_error_id",
                ],
            ),
            "memory overflow remains a typed runtime failure and does not degrade into host-side oversubscription.",
        ),
        case_receipt(
            "explicit_cancel_acknowledged",
            "cancel_running_execution",
            TassadarPostArticlePluginRuntimeApiCaseStatus::ExactCancellation,
            "inv.plugin.cancel.0006",
            "plugin.search_frontier",
            "1.0.0",
            "sha256:11f9b9be8f68aa2ff1e3d25723337ed90433dc98336c661cff79897fbd26075e",
            "handle_packet",
            &["artifact.read_only", "capability.search_frontier"],
            &["mount_envelope_identity", "mounted_capability_namespace_ids"],
            &[
                "queue_depth",
                "retry_budget",
                "runtime_cost_class",
                "timeout_budget_class",
                "memory_limit_class",
                "wall_clock_elapsed",
            ],
            "snapshot_based_failure_replay",
            "cancel_acknowledged",
            usage_summary(
                4,
                2_097_152,
                0,
                Some(21_504),
                false,
                &[
                    "invocation_identity_digest",
                    "input_packet_digest",
                    "host_error_id",
                ],
            ),
            "explicit cancellation stays behind one host-owned API and acknowledges only after runtime-owned interruption handling.",
        ),
    ];
    let exact_success_case_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarPostArticlePluginRuntimeApiCaseStatus::ExactSuccess)
        .count() as u32;
    let exact_typed_refusal_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarPostArticlePluginRuntimeApiCaseStatus::ExactTypedRefusal
        })
        .count() as u32;
    let exact_runtime_failure_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarPostArticlePluginRuntimeApiCaseStatus::ExactRuntimeFailure
        })
        .count() as u32;
    let exact_cancellation_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarPostArticlePluginRuntimeApiCaseStatus::ExactCancellation
        })
        .count() as u32;

    let mut bundle = TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundle {
        schema_version: 1,
        bundle_id: String::from(
            "tassadar.post_article_plugin_runtime_api_and_engine_abstraction.runtime_bundle.v1",
        ),
        host_owned_runtime_api_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID),
        engine_abstraction_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID),
        engine_profile_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_PROFILE_ID),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        artifact_loading_rows,
        engine_operation_rows,
        bound_rows,
        signal_rows,
        time_semantics,
        failure_isolation_rows,
        case_receipts,
        exact_success_case_count,
        exact_typed_refusal_case_count,
        exact_runtime_failure_case_count,
        exact_cancellation_case_count,
        claim_boundary: String::from(
            "this runtime bundle freezes one host-owned plugin runtime API above the packet ABI and manifest contract, with digest-verified load, bounded engine operations, explicit timeout/memory/queue/pool/concurrency ceilings, fixed model-information boundaries, and typed success/refusal/failure/cancellation outcomes. It does not imply weighted plugin control, plugin publication, served/public universality, or arbitrary public software execution.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Post-article plugin runtime API bundle covers success_cases={}, refusal_cases={}, failure_cases={}, cancellation_cases={}, engine_rows={}, bound_rows={}, and signal_rows={}.",
        bundle.exact_success_case_count,
        bundle.exact_typed_refusal_case_count,
        bundle.exact_runtime_failure_case_count,
        bundle.exact_cancellation_case_count,
        bundle.engine_operation_rows.len(),
        bundle.bound_rows.len(),
        bundle.signal_rows.len(),
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle|",
        &bundle,
    );
    bundle
}

#[must_use]
pub fn tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF)
}

pub fn write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundle,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundleError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn artifact_loading_row(
    row_id: &str,
    detail: &str,
) -> TassadarPostArticlePluginRuntimeArtifactLoadingRow {
    TassadarPostArticlePluginRuntimeArtifactLoadingRow {
        row_id: String::from(row_id),
        green: true,
        detail: String::from(detail),
    }
}

fn engine_operation_row(
    operation_id: &str,
    host_owned_surface_id: &str,
    model_visible: bool,
    detail: &str,
) -> TassadarPostArticlePluginRuntimeEngineOperationRow {
    TassadarPostArticlePluginRuntimeEngineOperationRow {
        operation_id: String::from(operation_id),
        host_owned_surface_id: String::from(host_owned_surface_id),
        backend_specific_types_hidden: true,
        model_visible,
        green: true,
        detail: String::from(detail),
    }
}

fn bound_row(
    bound_id: &str,
    required: bool,
    model_visible: bool,
    detail: &str,
) -> TassadarPostArticlePluginRuntimeBoundRow {
    TassadarPostArticlePluginRuntimeBoundRow {
        bound_id: String::from(bound_id),
        required,
        model_visible,
        green: true,
        detail: String::from(detail),
    }
}

fn signal_row(
    signal_id: &str,
    model_visible: bool,
    fixed_runtime_contract: bool,
    detail: &str,
) -> TassadarPostArticlePluginRuntimeSignalRow {
    TassadarPostArticlePluginRuntimeSignalRow {
        signal_id: String::from(signal_id),
        model_visible,
        fixed_runtime_contract,
        green: true,
        detail: String::from(detail),
    }
}

fn failure_isolation_row(
    isolation_id: &str,
    failure_scope_id: &str,
    detail: &str,
) -> TassadarPostArticlePluginRuntimeFailureIsolationRow {
    TassadarPostArticlePluginRuntimeFailureIsolationRow {
        isolation_id: String::from(isolation_id),
        failure_scope_id: String::from(failure_scope_id),
        green: true,
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn case_receipt(
    case_id: &str,
    lifecycle_surface_id: &str,
    status: TassadarPostArticlePluginRuntimeApiCaseStatus,
    invocation_id: &str,
    plugin_id: &str,
    plugin_version: &str,
    artifact_digest: &str,
    export_name: &str,
    mounted_capability_namespace_ids: &[&str],
    visible_signal_ids: &[&str],
    hidden_signal_ids: &[&str],
    replay_posture_id: &str,
    outcome_id: &str,
    usage_summary: TassadarPostArticlePluginRuntimeUsageSummary,
    note: &str,
) -> TassadarPostArticlePluginRuntimeCaseReceipt {
    let mut receipt = TassadarPostArticlePluginRuntimeCaseReceipt {
        case_id: String::from(case_id),
        lifecycle_surface_id: String::from(lifecycle_surface_id),
        status,
        invocation_id: String::from(invocation_id),
        plugin_id: String::from(plugin_id),
        plugin_version: String::from(plugin_version),
        artifact_digest: String::from(artifact_digest),
        export_name: String::from(export_name),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        mounted_capability_namespace_ids: mounted_capability_namespace_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        visible_signal_ids: visible_signal_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        hidden_signal_ids: hidden_signal_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        replay_posture_id: String::from(replay_posture_id),
        outcome_id: String::from(outcome_id),
        usage_summary,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_case|",
        &receipt,
    );
    receipt
}

fn usage_summary(
    logical_cpu_millis: u32,
    guest_memory_bytes: u64,
    queue_wait_millis: u32,
    fuel_consumed: Option<u64>,
    pool_reuse: bool,
    emitted_receipt_field_ids: &[&str],
) -> TassadarPostArticlePluginRuntimeUsageSummary {
    TassadarPostArticlePluginRuntimeUsageSummary {
        logical_cpu_millis,
        guest_memory_bytes,
        queue_wait_millis,
        fuel_consumed,
        pool_reuse,
        emitted_receipt_field_ids: emitted_receipt_field_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
    }
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
) -> Result<T, TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle, read_json,
        tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle_path,
        write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle,
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundle,
        TassadarPostArticlePluginRuntimeApiCaseStatus,
        TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF,
    };

    #[test]
    fn post_article_plugin_runtime_bundle_keeps_bounds_and_time_semantics_explicit() {
        let bundle = build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle();

        assert_eq!(
            bundle.host_owned_runtime_api_id,
            TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID
        );
        assert_eq!(
            bundle.engine_abstraction_id,
            TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID
        );
        assert_eq!(bundle.artifact_loading_rows.len(), 3);
        assert_eq!(bundle.engine_operation_rows.len(), 6);
        assert_eq!(bundle.bound_rows.len(), 6);
        assert_eq!(bundle.signal_rows.len(), 8);
        assert_eq!(bundle.failure_isolation_rows.len(), 3);
        assert_eq!(bundle.case_receipts.len(), 6);
        assert_eq!(bundle.exact_success_case_count, 2);
        assert_eq!(bundle.exact_typed_refusal_case_count, 1);
        assert_eq!(bundle.exact_runtime_failure_case_count, 2);
        assert_eq!(bundle.exact_cancellation_case_count, 1);
        assert!(!bundle.time_semantics.logical_time_model_observable);
        assert!(!bundle.time_semantics.wall_time_model_observable);
        assert!(!bundle.time_semantics.queue_depth_model_observable);
        assert!(!bundle.time_semantics.retry_budget_model_observable);
        assert!(!bundle.time_semantics.runtime_cost_model_observable);
        assert!(bundle.time_semantics.cost_model_invariance_required);
        assert!(bundle.time_semantics.scheduling_semantics_fixed);
        assert!(bundle.engine_operation_rows.iter().all(|row| row.green));
        assert!(bundle.bound_rows.iter().all(|row| row.green));
        assert!(bundle.failure_isolation_rows.iter().all(|row| row.green));
        assert!(bundle.case_receipts.iter().any(|case| {
            case.status == TassadarPostArticlePluginRuntimeApiCaseStatus::ExactCancellation
        }));
    }

    #[test]
    fn post_article_plugin_runtime_bundle_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle();
        let committed: TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundle = read_json(
            tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle_path(),
        )
        .expect("committed bundle");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle_path()
                .strip_prefix(super::repo_root())
                .expect("relative bundle path")
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_BUNDLE_REF
        );
    }

    #[test]
    fn write_post_article_plugin_runtime_bundle_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle.json");
        let written = write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle(
            &output_path,
        )
        .expect("write bundle");
        let persisted: TassadarPostArticlePluginRuntimeApiAndEngineAbstractionBundle =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read bundle"))
                .expect("decode bundle");
        assert_eq!(written, persisted);
    }
}
