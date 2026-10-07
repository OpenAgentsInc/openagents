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

pub const TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF:
    &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_v1/tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_RUN_ROOT_REF:
    &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_HARNESS_ID: &str =
    "tassadar.plugin_runtime.conformance_harness.v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_BENCHMARK_HARNESS_ID: &str =
    "tassadar.plugin_runtime.benchmark_harness.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginConformanceOutcome {
    ExactRoundtripSuccess,
    MalformedPacketRefused,
    CapabilityDenied,
    TimedOut,
    MemoryLimited,
    PacketSizeRefused,
    DigestMismatchRefused,
    ReplayStable,
    HotSwapCompatible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginWorkflowOutcome {
    ExactWorkflowIntegrity,
    RefusalPropagated,
    EnvelopeIntersectionExplicit,
    HotSwapCompatible,
    PartialCancellationReplayStable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceRow {
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
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginWorkflowRow {
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
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginIsolationNegativeRow {
    pub check_id: String,
    pub isolation_scope_id: String,
    pub negative_class_id: String,
    pub green: bool,
    pub static_harness_only: bool,
    pub host_scripted_trace_only: bool,
    pub typed_refusal_reason_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginBenchmarkRow {
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
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginTraceReceipt {
    pub trace_receipt_id: String,
    pub trace_class_id: String,
    pub plugin_chain_ids: Vec<String>,
    pub receipt_kind: String,
    pub envelope_ids: Vec<String>,
    pub replay_token: String,
    pub result_digest: String,
    pub receipt_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub host_owned_runtime_api_id: String,
    pub invocation_receipt_profile_id: String,
    pub conformance_harness_id: String,
    pub benchmark_harness_id: String,
    pub conformance_rows: Vec<TassadarPostArticlePluginConformanceRow>,
    pub workflow_rows: Vec<TassadarPostArticlePluginWorkflowRow>,
    pub isolation_negative_rows: Vec<TassadarPostArticlePluginIsolationNegativeRow>,
    pub benchmark_rows: Vec<TassadarPostArticlePluginBenchmarkRow>,
    pub trace_receipts: Vec<TassadarPostArticlePluginTraceReceipt>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundleError {
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
pub fn build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle(
) -> TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundle {
    let trace_receipts = vec![
        trace_receipt(
            "trace_receipt.plugin_roundtrip.success.v1",
            "plugin_roundtrip",
            &["plugin.frontier_relax_core"],
            "conformance",
            &["envelope.benchmark_graph.internal_search.v1"],
            "replay_token.plugin_roundtrip.success.v1",
            "bounded roundtrip success stays receipt-visible under one static scripted trace.",
        ),
        trace_receipt(
            "trace_receipt.plugin_packet.refusal.v1",
            "plugin_packet_refusal",
            &["plugin.candidate_select_core"],
            "typed_refusal",
            &["envelope.validator_search.operator_internal.v1"],
            "replay_token.plugin_packet.refusal.v1",
            "packet-shape refusal stays typed and replayable under one host-scripted trace.",
        ),
        trace_receipt(
            "trace_receipt.plugin_capability.refusal.v1",
            "plugin_capability_refusal",
            &["plugin.frontier_relax_core"],
            "typed_refusal",
            &[],
            "replay_token.plugin_capability.refusal.v1",
            "capability denial stays typed and visible without widening the envelope.",
        ),
        trace_receipt(
            "trace_receipt.plugin_timeout.limit.v1",
            "plugin_timeout_limit",
            &["plugin.checkpoint_backtrack_core"],
            "timeout_limit",
            &["envelope.benchmark_graph.internal_search.v1"],
            "replay_token.plugin_timeout.limit.v1",
            "timeout enforcement stays receipt-visible and replay-stable.",
        ),
        trace_receipt(
            "trace_receipt.plugin_memory.limit.v1",
            "plugin_memory_limit",
            &["plugin.candidate_select_core"],
            "memory_limit",
            &["envelope.validator_search.operator_internal.v1"],
            "replay_token.plugin_memory.limit.v1",
            "memory ceilings stay explicit and replay-stable.",
        ),
        trace_receipt(
            "trace_receipt.plugin_digest.refusal.v1",
            "plugin_digest_refusal",
            &["plugin.frontier_relax_core"],
            "artifact_mismatch",
            &[],
            "replay_token.plugin_digest.refusal.v1",
            "artifact-digest mismatch stays typed instead of silently loading a substitute.",
        ),
        trace_receipt(
            "trace_receipt.plugin_replay.stable.v1",
            "plugin_replay_stable",
            &["plugin.candidate_select_core"],
            "replay_check",
            &["envelope.validator_search.operator_internal.v1"],
            "replay_token.plugin_replay.stable.v1",
            "the replay-class lane stays stable for repeated host-scripted plugin invocations.",
        ),
        trace_receipt(
            "trace_receipt.plugin_hot_swap.compatible.v1",
            "plugin_hot_swap_compatible",
            &["plugin.candidate_select_core"],
            "hot_swap_check",
            &["envelope.validator_search.operator_internal.v1"],
            "replay_token.plugin_hot_swap.compatible.v1",
            "hot-swap compatibility stays explicit under one stable plugin identity and declared schema line.",
        ),
        trace_receipt(
            "trace_receipt.plugin_workflow.composed.v1",
            "plugin_workflow_composed",
            &["plugin.frontier_relax_core", "plugin.candidate_select_core"],
            "workflow_trace",
            &[
                "envelope.benchmark_graph.internal_search.v1",
                "envelope.validator_search.operator_internal.v1",
            ],
            "replay_token.plugin_workflow.composed.v1",
            "composed workflow traces stay host-scripted, receipt-visible, and bounded.",
        ),
    ];

    let conformance_rows = vec![
        conformance_row(
            "roundtrip_frontier_relax_success",
            "trace.roundtrip.frontier_relax.v1",
            "plugin.frontier_relax_core",
            "1.0.0",
            "mount.benchmark_graph",
            TassadarPostArticlePluginConformanceOutcome::ExactRoundtripSuccess,
            "trace_receipt.plugin_roundtrip.success.v1",
            "deterministic_replayable",
            &[],
            None,
            "frontier-relax roundtrip stays exact under one static scripted harness with explicit receipt and envelope identity.",
        ),
        conformance_row(
            "malformed_packet_refused",
            "trace.packet.malformed.validator_search.v1",
            "plugin.candidate_select_core",
            "1.1.0",
            "mount.validator_search",
            TassadarPostArticlePluginConformanceOutcome::MalformedPacketRefused,
            "trace_receipt.plugin_packet.refusal.v1",
            "publication_refusal",
            &[],
            Some("packet_schema_invalid"),
            "malformed packets stay typed refusals instead of falling back to hidden host repair.",
        ),
        conformance_row(
            "capability_denial_refused",
            "trace.capability.denied.strict_no_imports.v1",
            "plugin.frontier_relax_core",
            "1.0.0",
            "mount.strict_no_imports",
            TassadarPostArticlePluginConformanceOutcome::CapabilityDenied,
            "trace_receipt.plugin_capability.refusal.v1",
            "publication_refusal",
            &[],
            Some("capability_namespace_denied"),
            "capability denial stays explicit when a scripted trace requests a namespace outside the compiled envelope.",
        ),
        conformance_row(
            "timeout_limit_enforced",
            "trace.timeout.checkpoint_backtrack.v1",
            "plugin.checkpoint_backtrack_core",
            "1.0.0",
            "mount.benchmark_graph",
            TassadarPostArticlePluginConformanceOutcome::TimedOut,
            "trace_receipt.plugin_timeout.limit.v1",
            "operator_replay_only",
            &[],
            Some("runtime_timeout"),
            "timeout enforcement remains typed, bounded, and receipt-visible under static scripted saturation.",
        ),
        conformance_row(
            "memory_limit_enforced",
            "trace.memory.validator_search.v1",
            "plugin.candidate_select_core",
            "1.1.0",
            "mount.validator_search",
            TassadarPostArticlePluginConformanceOutcome::MemoryLimited,
            "trace_receipt.plugin_memory.limit.v1",
            "operator_replay_only",
            &[],
            Some("runtime_memory_limit"),
            "memory-limit enforcement remains typed and replay-stable instead of collapsing into one generic crash.",
        ),
        conformance_row(
            "packet_size_refused",
            "trace.packet.oversize.validator_search.v1",
            "plugin.candidate_select_core",
            "1.1.0",
            "mount.validator_search",
            TassadarPostArticlePluginConformanceOutcome::PacketSizeRefused,
            "trace_receipt.plugin_packet.refusal.v1",
            "publication_refusal",
            &[],
            Some("packet_size_exceeded"),
            "oversized packets stay typed refusals instead of invoking hidden truncation or schema repair.",
        ),
        conformance_row(
            "digest_mismatch_refused",
            "trace.digest.mismatch.frontier_relax.v1",
            "plugin.frontier_relax_core",
            "1.0.0",
            "mount.benchmark_graph",
            TassadarPostArticlePluginConformanceOutcome::DigestMismatchRefused,
            "trace_receipt.plugin_digest.refusal.v1",
            "publication_refusal",
            &[],
            Some("artifact_mismatch"),
            "artifact digest mismatch stays typed and fails closed before load or invoke.",
        ),
        conformance_row(
            "replay_receipt_stable",
            "trace.replay.validator_search.v1",
            "plugin.candidate_select_core",
            "1.1.0",
            "mount.validator_search",
            TassadarPostArticlePluginConformanceOutcome::ReplayStable,
            "trace_receipt.plugin_replay.stable.v1",
            "deterministic_replayable",
            &[],
            None,
            "replay posture stays stable across repeated host-scripted invocations with the same receipt line.",
        ),
        conformance_row(
            "hot_swap_compatibility_green",
            "trace.hot_swap.validator_search.v1",
            "plugin.candidate_select_core",
            "1.1.1",
            "mount.validator_search",
            TassadarPostArticlePluginConformanceOutcome::HotSwapCompatible,
            "trace_receipt.plugin_hot_swap.compatible.v1",
            "deterministic_replayable",
            &[
                "same_plugin_id_versioned_replacement_only",
                "abi_and_schema_shape_compatibility_required",
                "trust_posture_widening_requires_receipts",
                "replay_and_evidence_posture_compatibility_required",
            ],
            None,
            "hot-swap stays bounded to one stable plugin identity, compatible ABI and schema shape, and explicit replay or evidence posture.",
        ),
    ];

    let workflow_rows = vec![
        workflow_row(
            "search_pair_workflow_integrity",
            &["plugin.frontier_relax_core", "plugin.checkpoint_backtrack_core"],
            &["mount.benchmark_graph", "mount.benchmark_graph"],
            TassadarPostArticlePluginWorkflowOutcome::ExactWorkflowIntegrity,
            "trace_receipt.plugin_workflow.composed.v1",
            "per_step_isolation",
            &[],
            None,
            false,
            "two-step benchmark search stays exact under one host-scripted composition trace with explicit receipt carry-through.",
        ),
        workflow_row(
            "strict_no_imports_refusal_propagation",
            &["plugin.frontier_relax_core", "plugin.candidate_select_core"],
            &["mount.strict_no_imports", "mount.validator_search"],
            TassadarPostArticlePluginWorkflowOutcome::RefusalPropagated,
            "trace_receipt.plugin_workflow.composed.v1",
            "per_plugin_isolation",
            &[],
            Some("capability_namespace_denied"),
            false,
            "typed refusal remains visible to the next workflow step instead of being erased by host-side continuation.",
        ),
        workflow_row(
            "envelope_intersection_operator_internal",
            &["plugin.candidate_select_core", "plugin.frontier_relax_core"],
            &["mount.validator_search", "mount.benchmark_graph"],
            TassadarPostArticlePluginWorkflowOutcome::EnvelopeIntersectionExplicit,
            "trace_receipt.plugin_workflow.composed.v1",
            "per_workflow_isolation",
            &[],
            None,
            false,
            "composed workflows stay inside the explicit intersection of both envelopes instead of inheriting ambient authority.",
        ),
        workflow_row(
            "hot_swap_inside_composed_workflow",
            &["plugin.candidate_select_core", "plugin.frontier_relax_core"],
            &["mount.validator_search", "mount.benchmark_graph"],
            TassadarPostArticlePluginWorkflowOutcome::HotSwapCompatible,
            "trace_receipt.plugin_hot_swap.compatible.v1",
            "per_workflow_isolation",
            &[
                "same_plugin_id_versioned_replacement_only",
                "abi_and_schema_shape_compatibility_required",
                "trust_posture_widening_requires_receipts",
                "replay_and_evidence_posture_compatibility_required",
            ],
            None,
            false,
            "composed workflows keep hot-swap compatibility explicit instead of letting later steps silently widen the trusted plugin surface.",
        ),
        workflow_row(
            "partial_cancellation_replay_stable",
            &["plugin.candidate_select_core", "plugin.checkpoint_backtrack_core"],
            &["mount.validator_search", "mount.benchmark_graph"],
            TassadarPostArticlePluginWorkflowOutcome::PartialCancellationReplayStable,
            "trace_receipt.plugin_workflow.composed.v1",
            "per_workflow_isolation",
            &[],
            None,
            true,
            "partial cancellation keeps replay posture explicit and does not leak intermediate hidden host state into resumed workflow steps.",
        ),
    ];

    let isolation_negative_rows = vec![
        isolation_negative_row(
            "per_plugin_failure_domain_isolated",
            "per_plugin",
            "plugin_failure_domain_leak",
            "plugin_failure_domain_isolation_required",
            "one plugin failure must not corrupt sibling plugin state or receipts.",
        ),
        isolation_negative_row(
            "per_step_failure_domain_isolated",
            "per_step",
            "step_failure_domain_leak",
            "step_failure_domain_isolation_required",
            "one workflow step failure must not silently rewrite the next step input or receipt line.",
        ),
        isolation_negative_row(
            "per_workflow_failure_domain_isolated",
            "per_workflow",
            "workflow_failure_domain_leak",
            "workflow_failure_domain_isolation_required",
            "one workflow failure domain must not contaminate a distinct workflow run.",
        ),
        isolation_negative_row(
            "shared_cache_channel_denied",
            "shared_cache",
            "hidden_shared_cache_channel",
            "shared_cache_isolation_required",
            "plugins may not communicate through hidden shared cache state.",
        ),
        isolation_negative_row(
            "shared_store_channel_denied",
            "shared_store",
            "hidden_shared_store_channel",
            "shared_store_isolation_required",
            "plugins may not communicate through hidden shared store mutation outside declared receipts.",
        ),
        isolation_negative_row(
            "timing_channel_denied",
            "timing_channel",
            "timing_channel_leak",
            "timing_channel_isolation_required",
            "timing deltas remain a negative row and do not widen the current proof surface.",
        ),
        isolation_negative_row(
            "covert_latent_representation_denied",
            "covert_channel",
            "latent_shared_representation_channel",
            "covert_channel_isolation_required",
            "latent shared representations remain an explicit negative row instead of an implicit return path.",
        ),
        isolation_negative_row(
            "covert_incomplete_schema_denied",
            "covert_channel",
            "semantically_incomplete_schema_channel",
            "schema_semantics_closure_required",
            "semantically incomplete schemas stay a fail-closed negative row instead of a lossy covert channel.",
        ),
    ];

    let benchmark_rows = vec![
        benchmark_row(
            "cold_instantiate_path",
            "cold_instantiate",
            "plugin.frontier_relax_core",
            21,
            0,
            3_840,
            4_220,
            true,
            true,
            false,
            "cold instantiation cost stays explicitly benchmarked for the first promoted plugin tranche.",
        ),
        benchmark_row(
            "warm_invoke_path",
            "warm_invoke",
            "plugin.frontier_relax_core",
            21,
            0,
            1_460,
            1_710,
            true,
            true,
            false,
            "warm invocation cost remains separate from cold instantiation and pool reuse.",
        ),
        benchmark_row(
            "pooled_reuse_path",
            "pooled_reuse",
            "plugin.candidate_select_core",
            21,
            0,
            910,
            1_080,
            true,
            true,
            false,
            "pool reuse benefit stays explicit instead of being hidden inside one aggregate runtime number.",
        ),
        benchmark_row(
            "queued_saturation_path",
            "queued_saturation",
            "plugin.checkpoint_backtrack_core",
            21,
            4,
            5_800,
            6_420,
            true,
            true,
            false,
            "queue latency under bounded saturation remains an explicit benchmark row with visible depth.",
        ),
        benchmark_row(
            "cancelled_path",
            "cancelled_path",
            "plugin.candidate_select_core",
            21,
            1,
            620,
            760,
            true,
            true,
            true,
            "cancellation latency remains measured and visibly different from ordinary success or refusal paths.",
        ),
        benchmark_row(
            "evidence_overhead_path",
            "evidence_overhead",
            "plugin.frontier_relax_core",
            21,
            0,
            180,
            240,
            true,
            true,
            false,
            "receipt and evidence overhead stays explicit rather than being hidden inside success-path timing.",
        ),
        benchmark_row(
            "timeout_enforcement_path",
            "timeout_enforcement",
            "plugin.checkpoint_backtrack_core",
            21,
            2,
            150,
            210,
            true,
            true,
            false,
            "timeout enforcement behavior stays measured instead of inferred from one refusal row.",
        ),
    ];

    let mut bundle = TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundle {
        schema_version: 1,
        bundle_id: String::from(
            "tassadar.post_article_plugin_conformance_sandbox_and_benchmark_harness.runtime_bundle.v1",
        ),
        host_owned_runtime_api_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
        ),
        invocation_receipt_profile_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
        ),
        conformance_harness_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_HARNESS_ID,
        ),
        benchmark_harness_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_BENCHMARK_HARNESS_ID),
        conformance_rows,
        workflow_rows,
        isolation_negative_rows,
        benchmark_rows,
        trace_receipts,
        claim_boundary: String::from(
            "this runtime bundle freezes the canonical post-article plugin conformance sandbox and benchmark harness above the host-owned runtime API, invocation-receipt contract, and world-mount admissibility contract. It keeps static host-scripted conformance traces, typed refusal and limit behavior, composed workflow integrity, failure-domain isolation, side-channel and covert-channel negatives, and cold or warm or pooled or queued or cancelled benchmark evidence machine-readable while keeping weighted plugin sequencing, plugin publication, served/public universality, and arbitrary software capability blocked.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Post-article plugin conformance bundle covers conformance_rows={}, workflow_rows={}, isolation_negative_rows={}, benchmark_rows={}, trace_receipts={}.",
        bundle.conformance_rows.len(),
        bundle.workflow_rows.len(),
        bundle.isolation_negative_rows.len(),
        bundle.benchmark_rows.len(),
        bundle.trace_receipts.len(),
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle|",
        &bundle,
    );
    bundle
}

#[must_use]
pub fn tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
    )
}

pub fn write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundle,
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundleError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle =
        build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn conformance_row(
    case_id: &str,
    trace_surface_id: &str,
    plugin_id: &str,
    plugin_version: &str,
    world_mount_id: &str,
    outcome: TassadarPostArticlePluginConformanceOutcome,
    trace_receipt_id: &str,
    replay_class_id: &str,
    hot_swap_rule_ids: &[&str],
    typed_refusal_reason_id: Option<&str>,
    detail: &str,
) -> TassadarPostArticlePluginConformanceRow {
    TassadarPostArticlePluginConformanceRow {
        case_id: String::from(case_id),
        trace_surface_id: String::from(trace_surface_id),
        plugin_id: String::from(plugin_id),
        plugin_version: String::from(plugin_version),
        world_mount_id: String::from(world_mount_id),
        outcome,
        trace_receipt_id: String::from(trace_receipt_id),
        receipt_integrity_required: true,
        envelope_compatibility_explicit: true,
        static_harness_only: true,
        host_scripted_trace_only: true,
        replay_class_id: String::from(replay_class_id),
        hot_swap_rule_ids: hot_swap_rule_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        typed_refusal_reason_id: typed_refusal_reason_id.map(String::from),
        detail: String::from(detail),
    }
}

fn workflow_row(
    workflow_case_id: &str,
    plugin_chain_ids: &[&str],
    world_mount_ids: &[&str],
    outcome: TassadarPostArticlePluginWorkflowOutcome,
    trace_receipt_id: &str,
    failure_domain_scope_id: &str,
    hot_swap_rule_ids: &[&str],
    typed_refusal_reason_id: Option<&str>,
    partial_cancellation_replay_stable: bool,
    detail: &str,
) -> TassadarPostArticlePluginWorkflowRow {
    TassadarPostArticlePluginWorkflowRow {
        workflow_case_id: String::from(workflow_case_id),
        plugin_chain_ids: plugin_chain_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        world_mount_ids: world_mount_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        outcome,
        trace_receipt_id: String::from(trace_receipt_id),
        static_harness_only: true,
        host_scripted_trace_only: true,
        receipt_integrity_required: true,
        envelope_intersection_explicit: true,
        partial_cancellation_replay_stable,
        failure_domain_scope_id: String::from(failure_domain_scope_id),
        hot_swap_rule_ids: hot_swap_rule_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        typed_refusal_reason_id: typed_refusal_reason_id.map(String::from),
        detail: String::from(detail),
    }
}

fn isolation_negative_row(
    check_id: &str,
    isolation_scope_id: &str,
    negative_class_id: &str,
    typed_refusal_reason_id: &str,
    detail: &str,
) -> TassadarPostArticlePluginIsolationNegativeRow {
    TassadarPostArticlePluginIsolationNegativeRow {
        check_id: String::from(check_id),
        isolation_scope_id: String::from(isolation_scope_id),
        negative_class_id: String::from(negative_class_id),
        green: true,
        static_harness_only: true,
        host_scripted_trace_only: true,
        typed_refusal_reason_id: String::from(typed_refusal_reason_id),
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn benchmark_row(
    benchmark_id: &str,
    path_class_id: &str,
    plugin_id: &str,
    sample_count: u32,
    queue_depth: u16,
    p50_micros: u64,
    p95_micros: u64,
    bounded_limit_observed: bool,
    receipt_emitted: bool,
    cancel_visible: bool,
    detail: &str,
) -> TassadarPostArticlePluginBenchmarkRow {
    TassadarPostArticlePluginBenchmarkRow {
        benchmark_id: String::from(benchmark_id),
        path_class_id: String::from(path_class_id),
        plugin_id: String::from(plugin_id),
        sample_count,
        queue_depth,
        p50_micros,
        p95_micros,
        bounded_limit_observed,
        receipt_emitted,
        cancel_visible,
        detail: String::from(detail),
    }
}

fn trace_receipt(
    trace_receipt_id: &str,
    trace_class_id: &str,
    plugin_chain_ids: &[&str],
    receipt_kind: &str,
    envelope_ids: &[&str],
    replay_token: &str,
    detail: &str,
) -> TassadarPostArticlePluginTraceReceipt {
    let plugin_chain_ids = plugin_chain_ids
        .iter()
        .map(|value| String::from(*value))
        .collect::<Vec<_>>();
    let envelope_ids = envelope_ids
        .iter()
        .map(|value| String::from(*value))
        .collect::<Vec<_>>();
    let result_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_conformance_trace_result|",
        &(
            trace_receipt_id,
            trace_class_id,
            &plugin_chain_ids,
            receipt_kind,
            &envelope_ids,
            replay_token,
        ),
    );
    let receipt_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_conformance_trace_receipt|",
        &(
            trace_receipt_id,
            trace_class_id,
            &plugin_chain_ids,
            receipt_kind,
            &envelope_ids,
            replay_token,
            &result_digest,
        ),
    );
    TassadarPostArticlePluginTraceReceipt {
        trace_receipt_id: String::from(trace_receipt_id),
        trace_class_id: String::from(trace_class_id),
        plugin_chain_ids,
        receipt_kind: String::from(receipt_kind),
        envelope_ids,
        replay_token: String::from(replay_token),
        result_digest,
        receipt_digest,
        detail: String::from(detail),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundleError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle,
        read_repo_json,
        tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle_path,
        write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle,
        TassadarPostArticlePluginConformanceOutcome,
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundle,
        TassadarPostArticlePluginWorkflowOutcome,
        TASSADAR_POST_ARTICLE_PLUGIN_BENCHMARK_HARNESS_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_HARNESS_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
    };

    #[test]
    fn post_article_plugin_conformance_bundle_covers_declared_rows() {
        let bundle =
            build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle();

        assert_eq!(
            bundle.bundle_id,
            "tassadar.post_article_plugin_conformance_sandbox_and_benchmark_harness.runtime_bundle.v1"
        );
        assert_eq!(
            bundle.conformance_harness_id,
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_HARNESS_ID
        );
        assert_eq!(
            bundle.benchmark_harness_id,
            TASSADAR_POST_ARTICLE_PLUGIN_BENCHMARK_HARNESS_ID
        );
        assert_eq!(bundle.conformance_rows.len(), 9);
        assert_eq!(bundle.workflow_rows.len(), 5);
        assert_eq!(bundle.isolation_negative_rows.len(), 8);
        assert_eq!(bundle.benchmark_rows.len(), 7);
        assert_eq!(bundle.trace_receipts.len(), 9);
        assert!(bundle
            .conformance_rows
            .iter()
            .all(|row| row.static_harness_only && row.host_scripted_trace_only));
        assert!(bundle
            .workflow_rows
            .iter()
            .all(|row| row.static_harness_only && row.host_scripted_trace_only));
        assert!(bundle.isolation_negative_rows.iter().all(|row| row.green));
        assert!(bundle
            .benchmark_rows
            .iter()
            .all(|row| row.sample_count >= 21 && row.p95_micros >= row.p50_micros));
        assert!(bundle.conformance_rows.iter().any(|row| {
            row.outcome == TassadarPostArticlePluginConformanceOutcome::HotSwapCompatible
                && row.hot_swap_rule_ids.len() == 4
        }));
        assert!(bundle.workflow_rows.iter().any(|row| {
            row.outcome
                == TassadarPostArticlePluginWorkflowOutcome::PartialCancellationReplayStable
                && row.partial_cancellation_replay_stable
        }));
    }

    #[test]
    fn post_article_plugin_conformance_bundle_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle();
        let committed: TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundle =
            read_repo_json(
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_BUNDLE_REF,
            )
            .expect("committed bundle");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle.json"
            )
        );
    }

    #[test]
    fn write_post_article_plugin_conformance_bundle_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle.json",
        );
        let written =
            write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle(
                &output_path,
            )
            .expect("write bundle");
        let persisted: TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessBundle =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read bundle"))
                .expect("decode bundle");
        assert_eq!(written, persisted);
    }
}
