use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report,
    TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError,
    TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
};
use psionic_runtime::{
    build_tassadar_module_trust_isolation_report,
    build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle,
    TassadarPostArticlePluginConformanceOutcome,
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundle,
    TassadarPostArticlePluginWorkflowOutcome, TASSADAR_MODULE_TRUST_ISOLATION_REPORT_REF,
    TASSADAR_POST_ARTICLE_PLUGIN_BENCHMARK_HARNESS_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_HARNESS_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-plugin-conformance-sandbox-and-benchmark-harness.sh";

const ASYNC_LIFECYCLE_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_async_lifecycle_profile_report.json";
const EFFECTFUL_REPLAY_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_effectful_replay_audit_report.json";
const WORLD_MOUNT_COMPATIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_world_mount_compatibility_report.json";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginConformanceHarnessDependencyClass {
    AdmissibilityPrecedent,
    LifecyclePrecedent,
    ReplayPrecedent,
    IsolationPrecedent,
    WorldMountPrecedent,
    DesignInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceHarnessMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub admissibility_report_id: String,
    pub admissibility_report_digest: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub world_mount_envelope_compiler_id: String,
    pub admissibility_contract_id: String,
    pub conformance_harness_id: String,
    pub benchmark_harness_id: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub runtime_bundle_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceHarnessDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticlePluginConformanceHarnessDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceHarnessReportRow {
    pub case_id: String,
    pub trace_surface_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub world_mount_id: String,
    pub outcome: TassadarPostArticlePluginConformanceOutcome,
    pub trace_receipt_id: String,
    pub receipt_integrity_required: bool,
    pub envelope_compatibility_explicit: bool,
    pub static_harness_only: bool,
    pub host_scripted_trace_only: bool,
    pub replay_class_id: String,
    pub hot_swap_rule_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_refusal_reason_id: Option<String>,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginWorkflowHarnessReportRow {
    pub workflow_case_id: String,
    pub plugin_chain_ids: Vec<String>,
    pub world_mount_ids: Vec<String>,
    pub outcome: TassadarPostArticlePluginWorkflowOutcome,
    pub trace_receipt_id: String,
    pub static_harness_only: bool,
    pub host_scripted_trace_only: bool,
    pub receipt_integrity_required: bool,
    pub envelope_intersection_explicit: bool,
    pub partial_cancellation_replay_stable: bool,
    pub failure_domain_scope_id: String,
    pub hot_swap_rule_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_refusal_reason_id: Option<String>,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginIsolationNegativeReportRow {
    pub check_id: String,
    pub isolation_scope_id: String,
    pub negative_class_id: String,
    pub green: bool,
    pub static_harness_only: bool,
    pub host_scripted_trace_only: bool,
    pub typed_refusal_reason_id: String,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginBenchmarkHarnessReportRow {
    pub benchmark_id: String,
    pub path_class_id: String,
    pub plugin_id: String,
    pub sample_count: u32,
    pub queue_depth: u16,
    pub p50_micros: u64,
    pub p95_micros: u64,
    pub bounded_limit_observed: bool,
    pub receipt_emitted: bool,
    pub cancel_visible: bool,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceHarnessValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub admissibility_report_ref: String,
    pub async_lifecycle_profile_report_ref: String,
    pub effectful_replay_audit_report_ref: String,
    pub module_trust_isolation_report_ref: String,
    pub world_mount_compatibility_report_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding: TassadarPostArticlePluginConformanceHarnessMachineIdentityBinding,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundle,
    pub dependency_rows: Vec<TassadarPostArticlePluginConformanceHarnessDependencyRow>,
    pub conformance_rows: Vec<TassadarPostArticlePluginConformanceHarnessReportRow>,
    pub workflow_rows: Vec<TassadarPostArticlePluginWorkflowHarnessReportRow>,
    pub isolation_negative_rows: Vec<TassadarPostArticlePluginIsolationNegativeReportRow>,
    pub benchmark_rows: Vec<TassadarPostArticlePluginBenchmarkHarnessReportRow>,
    pub validation_rows: Vec<TassadarPostArticlePluginConformanceHarnessValidationRow>,
    pub contract_status: TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessStatus,
    pub contract_green: bool,
    pub operator_internal_only_posture: bool,
    pub static_harness_only: bool,
    pub host_scripted_trace_only: bool,
    pub receipt_integrity_frozen: bool,
    pub envelope_compatibility_explicit: bool,
    pub workflow_integrity_frozen: bool,
    pub failure_domain_isolation_frozen: bool,
    pub side_channel_negatives_green: bool,
    pub covert_channel_negatives_green: bool,
    pub hot_swap_compatibility_frozen: bool,
    pub replay_under_partial_cancellation_frozen: bool,
    pub benchmark_paths_measured: bool,
    pub evidence_overhead_explicit: bool,
    pub timeout_enforcement_measured: bool,
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
pub enum TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError {
    #[error(transparent)]
    Admissibility(
        #[from] TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError,
    ),
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

pub fn build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report(
) -> Result<
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReport,
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError,
> {
    let admissibility =
        build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report(
        )?;
    let async_lifecycle: AsyncLifecycleProfileFixture =
        read_repo_json(ASYNC_LIFECYCLE_PROFILE_REPORT_REF)?;
    let replay_audit: EffectfulReplayAuditFixture =
        read_repo_json(EFFECTFUL_REPLAY_AUDIT_REPORT_REF)?;
    let world_mount: WorldMountCompatibilityFixture =
        read_repo_json(WORLD_MOUNT_COMPATIBILITY_REPORT_REF)?;
    let module_trust = build_tassadar_module_trust_isolation_report();
    let runtime_bundle =
        build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle();

    let admissibility_dependency_closed =
        admissibility.contract_green && admissibility.deferred_issue_ids.is_empty();
    let async_lifecycle_precedent_bound = async_lifecycle.overall_green
        && async_lifecycle.exact_case_count == 3
        && async_lifecycle.refusal_case_count == 3
        && async_lifecycle
            .routeable_lifecycle_surface_ids
            .iter()
            .any(|id| id == "interruptible_counter_job")
        && async_lifecycle
            .routeable_lifecycle_surface_ids
            .iter()
            .any(|id| id == "retryable_timeout_search_job")
        && async_lifecycle
            .routeable_lifecycle_surface_ids
            .iter()
            .any(|id| id == "safe_boundary_cancellation_job");
    let replay_precedent_bound = replay_audit.challengeable_case_count == 3
        && replay_audit.refusal_case_count == 3
        && replay_audit
            .replay_safe_effect_family_ids
            .iter()
            .any(|id| id == "tassadar.effect_profile.virtual_fs_mounts.v1")
        && replay_audit
            .replay_safe_effect_family_ids
            .iter()
            .any(|id| id == "tassadar.internal_compute.async_lifecycle.v1")
        && replay_audit
            .refused_effect_family_ids
            .iter()
            .any(|id| id == "effect_receipt_missing");
    let isolation_precedent_bound = module_trust.allowed_case_count == 2
        && module_trust.refused_case_count == 3
        && module_trust.cross_tier_refusal_count == 1
        && module_trust.privilege_escalation_refusal_count == 1
        && module_trust.mount_policy_refusal_count == 1;
    let world_mount_precedent_bound = world_mount.allowed_case_count == 2
        && world_mount.denied_case_count == 1
        && world_mount.unresolved_case_count == 1;

    let machine_identity_binding = TassadarPostArticlePluginConformanceHarnessMachineIdentityBinding {
        machine_identity_id: admissibility
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: admissibility
            .machine_identity_binding
            .canonical_model_id
            .clone(),
        canonical_route_id: admissibility
            .machine_identity_binding
            .canonical_route_id
            .clone(),
        canonical_route_descriptor_digest: admissibility
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: admissibility
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: admissibility
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: admissibility
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: admissibility
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: admissibility
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        admissibility_report_id: admissibility.report_id.clone(),
        admissibility_report_digest: admissibility.report_digest.clone(),
        packet_abi_version: admissibility
            .machine_identity_binding
            .packet_abi_version
            .clone(),
        host_owned_runtime_api_id: admissibility
            .machine_identity_binding
            .host_owned_runtime_api_id
            .clone(),
        engine_abstraction_id: admissibility
            .machine_identity_binding
            .engine_abstraction_id
            .clone(),
        invocation_receipt_profile_id: admissibility
            .machine_identity_binding
            .invocation_receipt_profile_id
            .clone(),
        world_mount_envelope_compiler_id: admissibility
            .machine_identity_binding
            .world_mount_envelope_compiler_id
            .clone(),
        admissibility_contract_id: admissibility
            .machine_identity_binding
            .admissibility_contract_id
            .clone(),
        conformance_harness_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_HARNESS_ID),
        benchmark_harness_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_BENCHMARK_HARNESS_ID),
        runtime_bundle_id: runtime_bundle.bundle_id.clone(),
        runtime_bundle_digest: runtime_bundle.bundle_digest.clone(),
        runtime_bundle_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
        ),
        detail: format!(
            "machine_identity_id=`{}` canonical_route_id=`{}` admissibility_report_id=`{}` conformance_harness_id=`{}` and runtime_bundle_id=`{}` remain bound together.",
            admissibility.machine_identity_binding.machine_identity_id,
            admissibility.machine_identity_binding.canonical_route_id,
            admissibility.report_id,
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_HARNESS_ID,
            runtime_bundle.bundle_id,
        ),
    };

    let dependency_rows = vec![
        dependency_row(
            "admissibility_contract_closed",
            TassadarPostArticlePluginConformanceHarnessDependencyClass::AdmissibilityPrecedent,
            admissibility_dependency_closed,
            TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            Some(admissibility.report_id.clone()),
            Some(admissibility.report_digest.clone()),
            "the earlier admissibility contract is green and no longer defers TAS-203.",
        ),
        dependency_row(
            "async_lifecycle_precedent_bound",
            TassadarPostArticlePluginConformanceHarnessDependencyClass::LifecyclePrecedent,
            async_lifecycle_precedent_bound,
            ASYNC_LIFECYCLE_PROFILE_REPORT_REF,
            Some(async_lifecycle.report_id.clone()),
            Some(async_lifecycle.report_digest.clone()),
            "async lifecycle precedent keeps interruptible, timeout-retry, and cancellation surfaces explicit for queued and cancelled benchmark paths.",
        ),
        dependency_row(
            "effectful_replay_precedent_bound",
            TassadarPostArticlePluginConformanceHarnessDependencyClass::ReplayPrecedent,
            replay_precedent_bound,
            EFFECTFUL_REPLAY_AUDIT_REPORT_REF,
            Some(replay_audit.report_id.clone()),
            Some(replay_audit.report_digest.clone()),
            "effectful replay precedent keeps replay-safe effect families and typed refusal classes explicit for the plugin conformance harness.",
        ),
        dependency_row(
            "module_trust_isolation_precedent_bound",
            TassadarPostArticlePluginConformanceHarnessDependencyClass::IsolationPrecedent,
            isolation_precedent_bound,
            TASSADAR_MODULE_TRUST_ISOLATION_REPORT_REF,
            Some(module_trust.report_id.clone()),
            Some(module_trust.report_digest.clone()),
            "module trust isolation precedent keeps cross-tier, privilege-escalation, and mount-policy refusal lines explicit for plugin failure-domain isolation.",
        ),
        dependency_row(
            "world_mount_precedent_bound",
            TassadarPostArticlePluginConformanceHarnessDependencyClass::WorldMountPrecedent,
            world_mount_precedent_bound,
            WORLD_MOUNT_COMPATIBILITY_REPORT_REF,
            Some(world_mount.report_id.clone()),
            Some(world_mount.report_digest.clone()),
            "world-mount compatibility precedent keeps allowed, denied, and unresolved mount posture explicit for plugin conformance traces.",
        ),
        dependency_row(
            "plugin_system_conformance_shape_cited",
            TassadarPostArticlePluginConformanceHarnessDependencyClass::DesignInput,
            true,
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the local plugin-system note remains the cited design input for conformance, benchmark, hot-swap, refusal, and isolation requirements.",
        ),
    ];

    let conformance_rows = runtime_bundle
        .conformance_rows
        .iter()
        .map(|row| {
            let refusal_required = matches!(
                row.outcome,
                TassadarPostArticlePluginConformanceOutcome::MalformedPacketRefused
                    | TassadarPostArticlePluginConformanceOutcome::CapabilityDenied
                    | TassadarPostArticlePluginConformanceOutcome::TimedOut
                    | TassadarPostArticlePluginConformanceOutcome::MemoryLimited
                    | TassadarPostArticlePluginConformanceOutcome::PacketSizeRefused
                    | TassadarPostArticlePluginConformanceOutcome::DigestMismatchRefused
            );
            let hot_swap_required =
                row.outcome == TassadarPostArticlePluginConformanceOutcome::HotSwapCompatible;
            TassadarPostArticlePluginConformanceHarnessReportRow {
                case_id: row.case_id.clone(),
                trace_surface_id: row.trace_surface_id.clone(),
                plugin_id: row.plugin_id.clone(),
                plugin_version: row.plugin_version.clone(),
                world_mount_id: row.world_mount_id.clone(),
                outcome: row.outcome,
                trace_receipt_id: row.trace_receipt_id.clone(),
                receipt_integrity_required: row.receipt_integrity_required,
                envelope_compatibility_explicit: row.envelope_compatibility_explicit,
                static_harness_only: row.static_harness_only,
                host_scripted_trace_only: row.host_scripted_trace_only,
                replay_class_id: row.replay_class_id.clone(),
                hot_swap_rule_ids: row.hot_swap_rule_ids.clone(),
                typed_refusal_reason_id: row.typed_refusal_reason_id.clone(),
                green: row.receipt_integrity_required
                    && row.envelope_compatibility_explicit
                    && row.static_harness_only
                    && row.host_scripted_trace_only
                    && has_trace_receipt(&runtime_bundle, &row.trace_receipt_id)
                    && (!refusal_required || row.typed_refusal_reason_id.is_some())
                    && (!hot_swap_required || !row.hot_swap_rule_ids.is_empty()),
                source_refs: vec![
                    String::from(
                        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
                    ),
                    String::from(TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF),
                    String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                ],
                detail: row.detail.clone(),
            }
        })
        .collect::<Vec<_>>();

    let workflow_rows = runtime_bundle
        .workflow_rows
        .iter()
        .map(|row| {
            let refusal_required =
                row.outcome == TassadarPostArticlePluginWorkflowOutcome::RefusalPropagated;
            let hot_swap_required =
                row.outcome == TassadarPostArticlePluginWorkflowOutcome::HotSwapCompatible;
            let partial_cancel_required = row.outcome
                == TassadarPostArticlePluginWorkflowOutcome::PartialCancellationReplayStable;
            TassadarPostArticlePluginWorkflowHarnessReportRow {
                workflow_case_id: row.workflow_case_id.clone(),
                plugin_chain_ids: row.plugin_chain_ids.clone(),
                world_mount_ids: row.world_mount_ids.clone(),
                outcome: row.outcome,
                trace_receipt_id: row.trace_receipt_id.clone(),
                static_harness_only: row.static_harness_only,
                host_scripted_trace_only: row.host_scripted_trace_only,
                receipt_integrity_required: row.receipt_integrity_required,
                envelope_intersection_explicit: row.envelope_intersection_explicit,
                partial_cancellation_replay_stable: row.partial_cancellation_replay_stable,
                failure_domain_scope_id: row.failure_domain_scope_id.clone(),
                hot_swap_rule_ids: row.hot_swap_rule_ids.clone(),
                typed_refusal_reason_id: row.typed_refusal_reason_id.clone(),
                green: row.static_harness_only
                    && row.host_scripted_trace_only
                    && row.receipt_integrity_required
                    && row.envelope_intersection_explicit
                    && has_trace_receipt(&runtime_bundle, &row.trace_receipt_id)
                    && (!refusal_required || row.typed_refusal_reason_id.is_some())
                    && (!hot_swap_required || !row.hot_swap_rule_ids.is_empty())
                    && (!partial_cancel_required || row.partial_cancellation_replay_stable),
                source_refs: vec![
                    String::from(
                        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
                    ),
                    String::from(TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF),
                    String::from(EFFECTFUL_REPLAY_AUDIT_REPORT_REF),
                    String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                ],
                detail: row.detail.clone(),
            }
        })
        .collect::<Vec<_>>();

    let isolation_negative_rows = runtime_bundle
        .isolation_negative_rows
        .iter()
        .map(|row| TassadarPostArticlePluginIsolationNegativeReportRow {
            check_id: row.check_id.clone(),
            isolation_scope_id: row.isolation_scope_id.clone(),
            negative_class_id: row.negative_class_id.clone(),
            green: row.green
                && row.static_harness_only
                && row.host_scripted_trace_only
                && !row.typed_refusal_reason_id.is_empty(),
            static_harness_only: row.static_harness_only,
            host_scripted_trace_only: row.host_scripted_trace_only,
            typed_refusal_reason_id: row.typed_refusal_reason_id.clone(),
            source_refs: vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
                ),
                String::from(TASSADAR_MODULE_TRUST_ISOLATION_REPORT_REF),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            detail: row.detail.clone(),
        })
        .collect::<Vec<_>>();

    let benchmark_rows = runtime_bundle
        .benchmark_rows
        .iter()
        .map(|row| {
            let queued_required = row.path_class_id == "queued_saturation";
            let cancelled_required = row.path_class_id == "cancelled_path";
            let timeout_required = row.path_class_id == "timeout_enforcement";
            TassadarPostArticlePluginBenchmarkHarnessReportRow {
                benchmark_id: row.benchmark_id.clone(),
                path_class_id: row.path_class_id.clone(),
                plugin_id: row.plugin_id.clone(),
                sample_count: row.sample_count,
                queue_depth: row.queue_depth,
                p50_micros: row.p50_micros,
                p95_micros: row.p95_micros,
                bounded_limit_observed: row.bounded_limit_observed,
                receipt_emitted: row.receipt_emitted,
                cancel_visible: row.cancel_visible,
                green: row.sample_count >= 5
                    && row.p50_micros > 0
                    && row.p95_micros >= row.p50_micros
                    && row.receipt_emitted
                    && (!queued_required || row.queue_depth > 0)
                    && (!cancelled_required || row.cancel_visible)
                    && (!timeout_required || row.bounded_limit_observed),
                source_refs: vec![
                    String::from(
                        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
                    ),
                    String::from(ASYNC_LIFECYCLE_PROFILE_REPORT_REF),
                    String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                ],
                detail: row.detail.clone(),
            }
        })
        .collect::<Vec<_>>();

    let static_harness_only = conformance_rows.iter().all(|row| row.static_harness_only)
        && workflow_rows.iter().all(|row| row.static_harness_only)
        && isolation_negative_rows
            .iter()
            .all(|row| row.static_harness_only);
    let host_scripted_trace_only = conformance_rows
        .iter()
        .all(|row| row.host_scripted_trace_only)
        && workflow_rows.iter().all(|row| row.host_scripted_trace_only)
        && isolation_negative_rows
            .iter()
            .all(|row| row.host_scripted_trace_only);
    let receipt_integrity_frozen = conformance_rows
        .iter()
        .all(|row| row.receipt_integrity_required && row.green)
        && workflow_rows
            .iter()
            .all(|row| row.receipt_integrity_required && row.green)
        && runtime_bundle.trace_receipts.len() == 9;
    let envelope_compatibility_explicit = conformance_rows
        .iter()
        .all(|row| row.envelope_compatibility_explicit && row.green)
        && workflow_rows
            .iter()
            .all(|row| row.envelope_intersection_explicit && row.green);
    let workflow_integrity_frozen =
        workflow_rows.len() == 5 && workflow_rows.iter().all(|row| row.green);
    let failure_domain_isolation_frozen = isolation_negative_rows
        .iter()
        .filter(|row| row.isolation_scope_id.starts_with("per_"))
        .count()
        == 3
        && isolation_negative_rows
            .iter()
            .filter(|row| row.isolation_scope_id.starts_with("per_"))
            .all(|row| row.green);
    let side_channel_negatives_green = isolation_negative_rows
        .iter()
        .any(|row| row.negative_class_id == "hidden_shared_cache_channel" && row.green)
        && isolation_negative_rows
            .iter()
            .any(|row| row.negative_class_id == "hidden_shared_store_channel" && row.green)
        && isolation_negative_rows
            .iter()
            .any(|row| row.negative_class_id == "timing_channel_leak" && row.green);
    let covert_channel_negatives_green = isolation_negative_rows
        .iter()
        .any(|row| row.negative_class_id == "latent_shared_representation_channel" && row.green)
        && isolation_negative_rows.iter().any(|row| {
            row.negative_class_id == "semantically_incomplete_schema_channel" && row.green
        });
    let hot_swap_compatibility_frozen = conformance_rows.iter().any(|row| {
        row.outcome == TassadarPostArticlePluginConformanceOutcome::HotSwapCompatible && row.green
    }) && workflow_rows.iter().any(|row| {
        row.outcome == TassadarPostArticlePluginWorkflowOutcome::HotSwapCompatible && row.green
    });
    let replay_under_partial_cancellation_frozen = workflow_rows.iter().any(|row| {
        row.outcome == TassadarPostArticlePluginWorkflowOutcome::PartialCancellationReplayStable
            && row.green
    }) && replay_precedent_bound;
    let benchmark_paths_measured = benchmark_rows
        .iter()
        .any(|row| row.path_class_id == "cold_instantiate" && row.green)
        && benchmark_rows
            .iter()
            .any(|row| row.path_class_id == "warm_invoke" && row.green)
        && benchmark_rows
            .iter()
            .any(|row| row.path_class_id == "pooled_reuse" && row.green)
        && benchmark_rows
            .iter()
            .any(|row| row.path_class_id == "queued_saturation" && row.green)
        && benchmark_rows
            .iter()
            .any(|row| row.path_class_id == "cancelled_path" && row.green);
    let evidence_overhead_explicit = benchmark_rows
        .iter()
        .any(|row| row.path_class_id == "evidence_overhead" && row.green);
    let timeout_enforcement_measured = benchmark_rows
        .iter()
        .any(|row| row.path_class_id == "timeout_enforcement" && row.green);
    let operator_internal_only_posture = admissibility.operator_internal_only_posture
        && admissibility.rebase_claim_allowed
        && !admissibility.plugin_capability_claim_allowed
        && !admissibility.plugin_publication_allowed;

    let validation_rows = vec![
        validation_row(
            "admissibility_dependency_closed",
            admissibility_dependency_closed,
            &[TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF],
            "the earlier admissibility contract is green and no longer defers TAS-203.",
        ),
        validation_row(
            "async_lifecycle_precedent_bound",
            async_lifecycle_precedent_bound,
            &[ASYNC_LIFECYCLE_PROFILE_REPORT_REF],
            "async lifecycle precedent still carries interruptible, timeout-retry, and cancellation paths explicitly.",
        ),
        validation_row(
            "effectful_replay_precedent_bound",
            replay_precedent_bound,
            &[EFFECTFUL_REPLAY_AUDIT_REPORT_REF],
            "effectful replay precedent still carries replay-safe and refusal effect families explicitly.",
        ),
        validation_row(
            "module_trust_isolation_precedent_bound",
            isolation_precedent_bound,
            &[TASSADAR_MODULE_TRUST_ISOLATION_REPORT_REF],
            "module trust isolation precedent still carries cross-tier, privilege-escalation, and mount-policy refusal lines explicitly.",
        ),
        validation_row(
            "world_mount_precedent_bound",
            world_mount_precedent_bound,
            &[WORLD_MOUNT_COMPATIBILITY_REPORT_REF],
            "world-mount compatibility precedent still carries allowed, denied, and unresolved posture explicitly.",
        ),
        validation_row(
            "static_host_scripted_conformance_frozen",
            static_harness_only && host_scripted_trace_only,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
                LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            ],
            "conformance traces remain static and host-scripted rather than model-owned sequencing.",
        ),
        validation_row(
            "typed_refusal_and_limit_behavior_frozen",
            conformance_rows.iter().all(|row| row.green),
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
                EFFECTFUL_REPLAY_AUDIT_REPORT_REF,
            ],
            "roundtrip, refusal, timeout, memory-limit, packet-size, digest-mismatch, replay, and hot-swap rows stay typed and explicit.",
        ),
        validation_row(
            "workflow_integrity_frozen",
            workflow_integrity_frozen && replay_under_partial_cancellation_frozen,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
                EFFECTFUL_REPLAY_AUDIT_REPORT_REF,
            ],
            "workflow integrity, refusal propagation, envelope intersection, hot-swap, and partial-cancellation replay remain explicit.",
        ),
        validation_row(
            "failure_domain_isolation_frozen",
            failure_domain_isolation_frozen,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
                TASSADAR_MODULE_TRUST_ISOLATION_REPORT_REF,
            ],
            "per-plugin, per-step, and per-workflow failure-domain isolation remain explicit and green.",
        ),
        validation_row(
            "side_and_covert_channel_negatives_green",
            side_channel_negatives_green && covert_channel_negatives_green,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
                TASSADAR_MODULE_TRUST_ISOLATION_REPORT_REF,
            ],
            "shared-cache, shared-store, timing-channel, and covert-channel negatives remain explicit and green.",
        ),
        validation_row(
            "benchmark_paths_measured",
            benchmark_paths_measured && evidence_overhead_explicit && timeout_enforcement_measured,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
                ASYNC_LIFECYCLE_PROFILE_REPORT_REF,
                LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            ],
            "cold, warm, pooled, queued, cancelled, evidence-overhead, and timeout-enforcement benchmark rows remain explicit.",
        ),
        validation_row(
            "overclaim_posture_blocked",
            operator_internal_only_posture,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
            ],
            "the conformance sandbox stays operator/internal-only and does not imply weighted plugin sequencing, publication, served/public universality, or arbitrary software capability.",
        ),
    ];

    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && conformance_rows.iter().all(|row| row.green)
        && workflow_rows.iter().all(|row| row.green)
        && isolation_negative_rows.iter().all(|row| row.green)
        && benchmark_rows.iter().all(|row| row.green)
        && static_harness_only
        && host_scripted_trace_only
        && receipt_integrity_frozen
        && envelope_compatibility_explicit
        && workflow_integrity_frozen
        && failure_domain_isolation_frozen
        && side_channel_negatives_green
        && covert_channel_negatives_green
        && hot_swap_compatibility_frozen
        && replay_under_partial_cancellation_frozen
        && benchmark_paths_measured
        && evidence_overhead_explicit
        && timeout_enforcement_measured
        && validation_rows.iter().all(|row| row.green)
        && operator_internal_only_posture;
    let contract_status = if contract_green {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessStatus::Green
    } else {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessStatus::Incomplete
    };
    let rebase_claim_allowed = contract_green;

    let mut report = TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_plugin_conformance_sandbox_and_benchmark_harness.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_CHECKER_REF,
        ),
        admissibility_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
        ),
        async_lifecycle_profile_report_ref: String::from(ASYNC_LIFECYCLE_PROFILE_REPORT_REF),
        effectful_replay_audit_report_ref: String::from(EFFECTFUL_REPLAY_AUDIT_REPORT_REF),
        module_trust_isolation_report_ref: String::from(TASSADAR_MODULE_TRUST_ISOLATION_REPORT_REF),
        world_mount_compatibility_report_ref: String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        supporting_material_refs: vec![
            String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            ),
            String::from(ASYNC_LIFECYCLE_PROFILE_REPORT_REF),
            String::from(EFFECTFUL_REPLAY_AUDIT_REPORT_REF),
            String::from(TASSADAR_MODULE_TRUST_ISOLATION_REPORT_REF),
            String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
            String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        ],
        machine_identity_binding,
        runtime_bundle_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
        ),
        runtime_bundle,
        dependency_rows,
        conformance_rows,
        workflow_rows,
        isolation_negative_rows,
        benchmark_rows,
        validation_rows,
        contract_status,
        contract_green,
        operator_internal_only_posture,
        static_harness_only,
        host_scripted_trace_only,
        receipt_integrity_frozen,
        envelope_compatibility_explicit,
        workflow_integrity_frozen,
        failure_domain_isolation_frozen,
        side_channel_negatives_green,
        covert_channel_negatives_green,
        hot_swap_compatibility_frozen,
        replay_under_partial_cancellation_frozen,
        benchmark_paths_measured,
        evidence_overhead_explicit,
        timeout_enforcement_measured,
        rebase_claim_allowed,
        plugin_capability_claim_allowed: false,
        weighted_plugin_control_allowed: false,
        plugin_publication_allowed: false,
        served_public_universality_allowed: false,
        arbitrary_software_capability_allowed: false,
        deferred_issue_ids: vec![],
        claim_boundary: String::from(
            "this sandbox report freezes the canonical post-article plugin conformance sandbox and benchmark harness above the admissibility contract. It keeps static host-scripted conformance traces, typed refusal and limit behavior, workflow integrity, explicit envelope intersection, hot-swap compatibility, failure-domain isolation, side-channel and covert-channel negatives, and benchmark-path evidence machine-readable while keeping weighted plugin sequencing, plugin publication, served/public universality, and arbitrary software capability blocked.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article plugin conformance sandbox report keeps contract_status={:?}, dependency_rows={}, conformance_rows={}, workflow_rows={}, isolation_negative_rows={}, benchmark_rows={}, validation_rows={}, and deferred_issue_ids={}.",
        report.contract_status,
        report.dependency_rows.len(),
        report.conformance_rows.len(),
        report.workflow_rows.len(),
        report.isolation_negative_rows.len(),
        report.benchmark_rows.len(),
        report.validation_rows.len(),
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report_path(
) -> PathBuf {
    repo_root()
        .join(TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF)
}

pub fn write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReport,
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report =
        build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn dependency_row(
    dependency_id: &str,
    dependency_class: TassadarPostArticlePluginConformanceHarnessDependencyClass,
    satisfied: bool,
    source_ref: &str,
    bound_report_id: Option<String>,
    bound_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticlePluginConformanceHarnessDependencyRow {
    TassadarPostArticlePluginConformanceHarnessDependencyRow {
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
) -> TassadarPostArticlePluginConformanceHarnessValidationRow {
    TassadarPostArticlePluginConformanceHarnessValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn has_trace_receipt(
    bundle: &TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundle,
    trace_receipt_id: &str,
) -> bool {
    bundle
        .trace_receipts
        .iter()
        .any(|row| row.trace_receipt_id == trace_receipt_id)
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
) -> Result<T, TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[derive(Clone, Debug, Deserialize)]
struct AsyncLifecycleProfileFixture {
    report_id: String,
    report_digest: String,
    exact_case_count: u32,
    refusal_case_count: u32,
    overall_green: bool,
    routeable_lifecycle_surface_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct EffectfulReplayAuditFixture {
    report_id: String,
    report_digest: String,
    challengeable_case_count: u32,
    refusal_case_count: u32,
    replay_safe_effect_family_ids: Vec<String>,
    refused_effect_family_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct WorldMountCompatibilityFixture {
    report_id: String,
    report_digest: String,
    allowed_case_count: u32,
    denied_case_count: u32,
    unresolved_case_count: u32,
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report,
        read_repo_json,
        tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report_path,
        write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report,
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReport,
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessStatus,
        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
    };

    #[test]
    fn post_article_plugin_conformance_sandbox_report_keeps_frontier_explicit() {
        let report =
            build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report()
                .expect("report");

        assert_eq!(
            report.contract_status,
            TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessStatus::Green
        );
        assert_eq!(
            report.machine_identity_binding.conformance_harness_id,
            "tassadar.plugin_runtime.conformance_harness.v1"
        );
        assert_eq!(
            report.machine_identity_binding.benchmark_harness_id,
            "tassadar.plugin_runtime.benchmark_harness.v1"
        );
        assert_eq!(report.dependency_rows.len(), 6);
        assert_eq!(report.conformance_rows.len(), 9);
        assert_eq!(report.workflow_rows.len(), 5);
        assert_eq!(report.isolation_negative_rows.len(), 8);
        assert_eq!(report.benchmark_rows.len(), 7);
        assert_eq!(report.validation_rows.len(), 12);
        assert!(report.deferred_issue_ids.is_empty());
        assert!(report.operator_internal_only_posture);
        assert!(report.static_harness_only);
        assert!(report.host_scripted_trace_only);
        assert!(report.receipt_integrity_frozen);
        assert!(report.envelope_compatibility_explicit);
        assert!(report.workflow_integrity_frozen);
        assert!(report.failure_domain_isolation_frozen);
        assert!(report.side_channel_negatives_green);
        assert!(report.covert_channel_negatives_green);
        assert!(report.hot_swap_compatibility_frozen);
        assert!(report.replay_under_partial_cancellation_frozen);
        assert!(report.benchmark_paths_measured);
        assert!(report.evidence_overhead_explicit);
        assert!(report.timeout_enforcement_measured);
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_plugin_conformance_sandbox_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report()
                .expect("report");
        let committed: TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReport =
            read_repo_json(
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report.json"
            )
        );
    }

    #[test]
    fn write_post_article_plugin_conformance_sandbox_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report.json",
        );
        let written =
            write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
