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
    TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_invocation_receipts_and_replay_classes_v1/tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_invocation_receipts_and_replay_classes_v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID: &str =
    "tassadar.plugin_runtime.invocation_receipts.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginInvocationReceiptCaseStatus {
    ExactSuccessReceipt,
    ExactRefusalReceipt,
    ExactFailureReceipt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginReceiptFieldRow {
    pub field_id: String,
    pub required: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginReplayClassRow {
    pub replay_class_id: String,
    pub retry_posture_id: String,
    pub propagation_posture_id: String,
    pub route_evidence_required: bool,
    pub challenge_receipt_required: bool,
    pub promotion_allowed: bool,
    pub served_claim_allowed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginFailureClassRow {
    pub failure_class_id: String,
    pub class_kind: String,
    pub default_replay_class_id: String,
    pub retry_posture_id: String,
    pub propagation_posture_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationResourceSummary {
    pub logical_start_tick: u64,
    pub logical_duration_ticks: u32,
    pub timeout_ceiling_millis: u32,
    pub memory_ceiling_bytes: u64,
    pub queue_wait_millis: u32,
    pub logical_cpu_millis: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel_consumed: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationReceiptCase {
    pub case_id: String,
    pub status: TassadarPostArticlePluginInvocationReceiptCaseStatus,
    pub receipt_id: String,
    pub invocation_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub install_id: String,
    pub artifact_digest: String,
    pub export_name: String,
    pub packet_abi_version: String,
    pub input_packet_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_packet_digest: Option<String>,
    pub mount_envelope_id: String,
    pub capability_envelope_id: String,
    pub backend_id: String,
    pub replay_class_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class_id: Option<String>,
    pub route_evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub challenge_receipt_ref: Option<String>,
    pub resource_summary: TassadarPostArticlePluginInvocationResourceSummary,
    pub note: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub packet_abi_version: String,
    pub receipt_field_rows: Vec<TassadarPostArticlePluginReceiptFieldRow>,
    pub replay_class_rows: Vec<TassadarPostArticlePluginReplayClassRow>,
    pub failure_class_rows: Vec<TassadarPostArticlePluginFailureClassRow>,
    pub case_receipts: Vec<TassadarPostArticlePluginInvocationReceiptCase>,
    pub exact_success_case_count: u32,
    pub exact_refusal_case_count: u32,
    pub exact_failure_case_count: u32,
    pub challenge_bound_case_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundleError {
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
pub fn build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle(
) -> TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundle {
    let receipt_field_rows = vec![
        receipt_field_row("receipt_id", true, "receipt identity remains explicit and stable."),
        receipt_field_row("invocation_id", true, "invocation identity remains explicit."),
        receipt_field_row("plugin_id", true, "plugin family identity remains explicit."),
        receipt_field_row("plugin_version", true, "plugin version remains explicit."),
        receipt_field_row("install_id", true, "admitted install identity remains explicit."),
        receipt_field_row(
            "artifact_digest",
            true,
            "artifact digest remains explicit so invocation truth stays tied to one declared artifact.",
        ),
        receipt_field_row(
            "export_name",
            true,
            "guest export identity remains explicit at receipt time.",
        ),
        receipt_field_row(
            "packet_abi_version",
            true,
            "packet ABI version remains explicit at receipt time.",
        ),
        receipt_field_row(
            "input_packet_digest",
            true,
            "input packet digest remains explicit for challengeable replay.",
        ),
        receipt_field_row(
            "output_packet_digest",
            false,
            "output packet digest is explicit when an invocation returns an output packet.",
        ),
        receipt_field_row(
            "mount_envelope_id",
            true,
            "world-mount envelope identity remains explicit.",
        ),
        receipt_field_row(
            "capability_envelope_id",
            true,
            "capability envelope identity remains explicit.",
        ),
        receipt_field_row(
            "backend_id",
            true,
            "runner/backend identity remains explicit in the receipt.",
        ),
        receipt_field_row(
            "replay_class_id",
            true,
            "replay posture remains explicit in the receipt.",
        ),
        receipt_field_row(
            "failure_class_id",
            false,
            "typed refusal or failure class remains explicit when an invocation does not succeed.",
        ),
        receipt_field_row(
            "resource_summary",
            true,
            "resource summary carries logical start, duration, timeout, memory ceiling, and bounded usage signals.",
        ),
        receipt_field_row(
            "route_evidence_refs",
            true,
            "route-integrated evidence references remain explicit in every receipt.",
        ),
        receipt_field_row(
            "challenge_receipt_ref",
            false,
            "challenge receipt references remain explicit when the replay posture requires or emits them.",
        ),
    ];
    let replay_class_rows = vec![
        replay_class_row(
            "deterministic_replayable",
            "no_retry_without_input_or_policy_change",
            "stop_step_may_reprompt",
            true,
            false,
            true,
            false,
            "deterministic replayable receipts are exact under fixed inputs and policy, and promoted routes must carry route evidence plus challenge receipts.",
        ),
        replay_class_row(
            "replayable_with_snapshots",
            "retry_after_snapshot_resume",
            "propagate_typed_failure_until_snapshot_or_budget_change",
            true,
            true,
            true,
            false,
            "snapshot-backed replay classes remain admissible only when receipts carry the replay posture and the route binds snapshot-aware evidence.",
        ),
        replay_class_row(
            "operator_replay_only",
            "operator_manual_retry_only",
            "quarantine_plugin_and_block_public_claims",
            true,
            false,
            false,
            false,
            "operator-only replay classes remain challengeable but cannot back promotion or public claims.",
        ),
        replay_class_row(
            "non_replayable_refused_for_publication",
            "no_retry_fail_closed",
            "block_publication_and_served_claims",
            true,
            false,
            false,
            false,
            "non-replayable receipts remain explicit refusal truth and cannot support promoted or public plugin claims.",
        ),
    ];
    let failure_class_rows = vec![
        failure_class_row(
            "policy_refusal",
            "refusal",
            "deterministic_replayable",
            "no_retry_fail_closed",
            "stop_workflow_until_policy_changes",
        ),
        failure_class_row(
            "schema_refusal",
            "refusal",
            "deterministic_replayable",
            "retry_after_input_fix",
            "stop_step_may_reprompt",
        ),
        failure_class_row(
            "capability_refusal",
            "refusal",
            "deterministic_replayable",
            "retry_after_mount_change",
            "stop_step_may_reprompt",
        ),
        failure_class_row(
            "runtime_timeout",
            "failure",
            "replayable_with_snapshots",
            "retry_after_snapshot_resume",
            "propagate_typed_failure",
        ),
        failure_class_row(
            "runtime_memory_limit",
            "failure",
            "replayable_with_snapshots",
            "retry_after_limit_change",
            "propagate_typed_failure",
        ),
        failure_class_row(
            "runtime_crash",
            "failure",
            "replayable_with_snapshots",
            "retry_after_engine_restart",
            "propagate_typed_failure",
        ),
        failure_class_row(
            "artifact_mismatch",
            "failure",
            "deterministic_replayable",
            "no_retry_fail_closed",
            "block_plugin_until_artifact_fixed",
        ),
        failure_class_row(
            "plugin_internal_refusal",
            "refusal",
            "deterministic_replayable",
            "retry_after_input_change",
            "stop_step_may_reprompt",
        ),
        failure_class_row(
            "plugin_internal_failure",
            "failure",
            "replayable_with_snapshots",
            "retry_after_snapshot_resume",
            "propagate_typed_failure",
        ),
        failure_class_row(
            "replay_posture_violation",
            "refusal",
            "operator_replay_only",
            "no_retry_fail_closed",
            "suppress_promotion_until_replay_posture_fixed",
        ),
        failure_class_row(
            "trust_posture_violation",
            "refusal",
            "operator_replay_only",
            "no_retry_fail_closed",
            "quarantine_plugin_and_block_route",
        ),
        failure_class_row(
            "publication_posture_violation",
            "refusal",
            "non_replayable_refused_for_publication",
            "no_retry_fail_closed",
            "block_publication_and_served_claims",
        ),
    ];
    let case_receipts = vec![
        invocation_receipt_case(
            "frontier_relax_core_deterministic_success",
            TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactSuccessReceipt,
            "receipt.frontier_relax_core.success.0001",
            "inv.frontier_relax_core.success.0001",
            "plugin.frontier_relax_core",
            "1.0.0",
            "reinstall.frontier_relax_core.session.v2",
            "sha256:6c7ee2d494f041e5b8cc14b30d7471d0fb759b7151dc12a3f5da6e3dded7f0cd",
            "handle_packet",
            "sha256:05a4d1a173d9a7ff857d7d6c7b7db786cb5a2892c17f1be2f7c54f18f982ed74",
            Some("sha256:f6e3ad5da47404b915b6f5e5702d0f48e179f5ea7f8d44e6762f72bca4f0a7ef"),
            "world_mount.frontier_relax_core.read_only.v1",
            "capability_envelope.frontier_relax_core.read_only.v1",
            "tassadar.plugin_runtime.engine_profile.wasm_bounded.v1",
            "deterministic_replayable",
            None,
            &[
                "fixtures/tassadar/reports/tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report.json",
                "fixtures/tassadar/reports/tassadar_installed_module_evidence_report.json",
                "fixtures/tassadar/reports/tassadar_module_promotion_state_report.json",
            ],
            Some(
                "fixtures/tassadar/runs/tassadar_effectful_replay_audit_v1/challenge_receipts/virtual_fs_proof_replay.challenge_receipt.json",
            ),
            resource_summary(4_200, 9, 150, 8_388_608, 0, 3, Some(18_432)),
            "deterministic frontier_relax_core invocation remains replayable because receipt identity, output digest, route evidence, and challenge receipt all stay explicit.",
        ),
        invocation_receipt_case(
            "candidate_select_core_schema_refusal",
            TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactRefusalReceipt,
            "receipt.candidate_select_core.schema_refusal.0002",
            "inv.candidate_select_core.schema_refusal.0002",
            "plugin.candidate_select_core",
            "1.1.0",
            "install.candidate_select_core.rollback.v1",
            "sha256:79a34165f460f1bd49ef4335a8a3574f4bd1dc8f2411da53af87aa7355094908",
            "handle_packet",
            "sha256:827c7f6bcfcd88d2ef9eb47b777d4df362e97aa9879129f541c415d9efc75d09",
            None,
            "world_mount.candidate_select_core.rollback.v1",
            "capability_envelope.candidate_select_core.rollback.v1",
            "tassadar.plugin_runtime.engine_profile.wasm_bounded.v1",
            "deterministic_replayable",
            Some("schema_refusal"),
            &[
                "fixtures/tassadar/reports/tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report.json",
                "fixtures/tassadar/reports/tassadar_module_promotion_state_report.json",
            ],
            None,
            resource_summary(4_240, 2, 150, 8_388_608, 0, 1, None),
            "schema refusal remains exact and challengeable by route evidence even when no output packet is produced.",
        ),
        invocation_receipt_case(
            "candidate_select_core_capability_refusal",
            TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactRefusalReceipt,
            "receipt.candidate_select_core.capability_refusal.0003",
            "inv.candidate_select_core.capability_refusal.0003",
            "plugin.candidate_select_core",
            "1.1.0",
            "install.candidate_select_core.rollback.v1",
            "sha256:79a34165f460f1bd49ef4335a8a3574f4bd1dc8f2411da53af87aa7355094908",
            "handle_packet",
            "sha256:dc6d29fce84b0a56c5f699bdb1236fa4f04334b8a2f7fd2ee7a8bc46ce9be92b",
            None,
            "world_mount.candidate_select_core.rollback.v1",
            "capability_envelope.candidate_select_core.rollback.v1",
            "tassadar.plugin_runtime.engine_profile.wasm_bounded.v1",
            "deterministic_replayable",
            Some("capability_refusal"),
            &[
                "fixtures/tassadar/reports/tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report.json",
                "fixtures/tassadar/reports/tassadar_module_promotion_state_report.json",
            ],
            None,
            resource_summary(4_250, 2, 150, 8_388_608, 0, 1, None),
            "capability denial remains a typed refusal and preserves the exact install and envelope identities used during admission.",
        ),
        invocation_receipt_case(
            "search_frontier_timeout_failure",
            TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactFailureReceipt,
            "receipt.search_frontier.timeout_failure.0004",
            "inv.search_frontier.timeout_failure.0004",
            "plugin.search_frontier",
            "1.0.0",
            "reinstall.frontier_relax_core.session.v2",
            "sha256:11f9b9be8f68aa2ff1e3d25723337ed90433dc98336c661cff79897fbd26075e",
            "handle_packet",
            "sha256:95d12917d27bb7793448254e817f3ee9e45afd2fbb27b4d9b8e0cbe6d4a31ee8",
            None,
            "world_mount.search_frontier.snapshot.v1",
            "capability_envelope.search_frontier.snapshot.v1",
            "tassadar.plugin_runtime.engine_profile.wasm_bounded.v1",
            "replayable_with_snapshots",
            Some("runtime_timeout"),
            &[
                "fixtures/tassadar/reports/tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report.json",
                "fixtures/tassadar/reports/tassadar_effectful_replay_audit_report.json",
            ],
            Some(
                "fixtures/tassadar/runs/tassadar_effectful_replay_audit_v1/challenge_receipts/async_safe_cancel_replay.challenge_receipt.json",
            ),
            resource_summary(4_320, 24, 25, 8_388_608, 0, 25, Some(131_072)),
            "timeout failures remain snapshot-replayable because the runtime carries replay posture and challenge binding explicitly.",
        ),
        invocation_receipt_case(
            "memory_probe_failure",
            TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactFailureReceipt,
            "receipt.memory_probe.failure.0005",
            "inv.memory_probe.failure.0005",
            "plugin.memory_probe",
            "1.0.0",
            "reinstall.frontier_relax_core.session.v2",
            "sha256:ae6713a1b2208d9363565108ba8d5b005402f176c0ff76e709e26d932855db02",
            "handle_packet",
            "sha256:e8b89e6f1b06220f3f71c2e7e3a69d4bf0d12f75c245270b2f234d3722e3f359",
            None,
            "world_mount.memory_probe.snapshot.v1",
            "capability_envelope.memory_probe.snapshot.v1",
            "tassadar.plugin_runtime.engine_profile.wasm_bounded.v1",
            "replayable_with_snapshots",
            Some("runtime_memory_limit"),
            &[
                "fixtures/tassadar/reports/tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report.json",
                "fixtures/tassadar/reports/tassadar_effectful_replay_audit_report.json",
            ],
            Some("receipt://memory_probe/challenge/runtime_memory_limit.snapshot"),
            resource_summary(4_360, 8, 50, 8_388_608, 0, 9, Some(49_152)),
            "memory-limit failures remain snapshot-replayable and keep typed resource summaries explicit.",
        ),
        invocation_receipt_case(
            "branch_prune_publication_posture_violation",
            TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactRefusalReceipt,
            "receipt.branch_prune.publication_posture_violation.0006",
            "inv.branch_prune.publication_posture_violation.0006",
            "plugin.branch_prune_core",
            "0.1.0",
            "install.branch_prune_core.refused_missing_evidence.v1",
            "sha256:fdcbfef2d2ff3ab3ba5c6711bb989d8bb690729e82396595f7a9192928ed3c1d",
            "handle_packet",
            "sha256:713462e48a2a77a74f9a318327b4e0a9ac52f31b0d24dd47cebb6a42199d9db7",
            None,
            "world_mount.branch_prune_core.operator_only.v1",
            "capability_envelope.branch_prune_core.operator_only.v1",
            "tassadar.plugin_runtime.engine_profile.wasm_bounded.v1",
            "non_replayable_refused_for_publication",
            Some("publication_posture_violation"),
            &[
                "fixtures/tassadar/reports/tassadar_installed_module_evidence_report.json",
                "fixtures/tassadar/reports/tassadar_module_promotion_state_report.json",
            ],
            None,
            resource_summary(4_400, 1, 100, 4_194_304, 0, 1, None),
            "publication posture violations remain explicit refusal truth and cannot be widened into replayable public claims.",
        ),
        invocation_receipt_case(
            "candidate_select_trust_posture_violation",
            TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactRefusalReceipt,
            "receipt.candidate_select.trust_posture_violation.0007",
            "inv.candidate_select.trust_posture_violation.0007",
            "plugin.candidate_select_core",
            "1.1.0",
            "install.candidate_select_core.refused_stale_evidence.v2",
            "sha256:6a14d0d4341e13d093f95ce109da3db50f9fa3a5c87cceac401a9b9cb18f1d95",
            "handle_packet",
            "sha256:8507d324d43afb7af6e9cfd0ca8cfd576552be4277ea26f15f57314be4628ebf",
            None,
            "world_mount.candidate_select_core.quarantined.v2",
            "capability_envelope.candidate_select_core.quarantined.v2",
            "tassadar.plugin_runtime.engine_profile.wasm_bounded.v1",
            "operator_replay_only",
            Some("trust_posture_violation"),
            &[
                "fixtures/tassadar/reports/tassadar_installed_module_evidence_report.json",
                "fixtures/tassadar/reports/tassadar_module_promotion_state_report.json",
            ],
            None,
            resource_summary(4_420, 1, 100, 4_194_304, 0, 1, None),
            "trust posture violations remain operator-only receipt truth until stale evidence is repaired and re-promoted.",
        ),
    ];
    let exact_success_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactSuccessReceipt
        })
        .count() as u32;
    let exact_refusal_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactRefusalReceipt
        })
        .count() as u32;
    let exact_failure_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactFailureReceipt
        })
        .count() as u32;
    let challenge_bound_case_count = case_receipts
        .iter()
        .filter(|case| case.challenge_receipt_ref.is_some())
        .count() as u32;

    let mut bundle = TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundle {
        schema_version: 1,
        bundle_id: String::from(
            "tassadar.post_article_plugin_invocation_receipts_and_replay_classes.runtime_bundle.v1",
        ),
        profile_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID),
        host_owned_runtime_api_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID),
        engine_abstraction_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        receipt_field_rows,
        replay_class_rows,
        failure_class_rows,
        case_receipts,
        exact_success_case_count,
        exact_refusal_case_count,
        exact_failure_case_count,
        challenge_bound_case_count,
        claim_boundary: String::from(
            "this runtime bundle freezes the canonical invocation-receipt identity and replay-class lattice above the host-owned plugin runtime API. It keeps invocation identity, packet digests, envelope ids, backend id, resource summaries, failure classes, replay posture, route evidence, and challenge bindings explicit while keeping weighted plugin control, plugin publication, served/public universality, and arbitrary software capability blocked.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Post-article plugin invocation receipt bundle covers receipt_fields={}, replay_classes={}, failure_classes={}, success_cases={}, refusal_cases={}, failure_cases={}, challenge_bound_cases={}.",
        bundle.receipt_field_rows.len(),
        bundle.replay_class_rows.len(),
        bundle.failure_class_rows.len(),
        bundle.exact_success_case_count,
        bundle.exact_refusal_case_count,
        bundle.exact_failure_case_count,
        bundle.challenge_bound_case_count,
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle|",
        &bundle,
    );
    bundle
}

#[must_use]
pub fn tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle_path() -> PathBuf
{
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF)
}

pub fn write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundle,
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundleError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn receipt_field_row(
    field_id: &str,
    required: bool,
    detail: &str,
) -> TassadarPostArticlePluginReceiptFieldRow {
    TassadarPostArticlePluginReceiptFieldRow {
        field_id: String::from(field_id),
        required,
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn replay_class_row(
    replay_class_id: &str,
    retry_posture_id: &str,
    propagation_posture_id: &str,
    route_evidence_required: bool,
    challenge_receipt_required: bool,
    promotion_allowed: bool,
    served_claim_allowed: bool,
    detail: &str,
) -> TassadarPostArticlePluginReplayClassRow {
    TassadarPostArticlePluginReplayClassRow {
        replay_class_id: String::from(replay_class_id),
        retry_posture_id: String::from(retry_posture_id),
        propagation_posture_id: String::from(propagation_posture_id),
        route_evidence_required,
        challenge_receipt_required,
        promotion_allowed,
        served_claim_allowed,
        detail: String::from(detail),
    }
}

fn failure_class_row(
    failure_class_id: &str,
    class_kind: &str,
    default_replay_class_id: &str,
    retry_posture_id: &str,
    propagation_posture_id: &str,
) -> TassadarPostArticlePluginFailureClassRow {
    TassadarPostArticlePluginFailureClassRow {
        failure_class_id: String::from(failure_class_id),
        class_kind: String::from(class_kind),
        default_replay_class_id: String::from(default_replay_class_id),
        retry_posture_id: String::from(retry_posture_id),
        propagation_posture_id: String::from(propagation_posture_id),
        detail: format!(
            "`{failure_class_id}` remains typed with replay=`{default_replay_class_id}`, retry=`{retry_posture_id}`, and propagation=`{propagation_posture_id}`."
        ),
    }
}

fn resource_summary(
    logical_start_tick: u64,
    logical_duration_ticks: u32,
    timeout_ceiling_millis: u32,
    memory_ceiling_bytes: u64,
    queue_wait_millis: u32,
    logical_cpu_millis: u32,
    fuel_consumed: Option<u64>,
) -> TassadarPostArticlePluginInvocationResourceSummary {
    TassadarPostArticlePluginInvocationResourceSummary {
        logical_start_tick,
        logical_duration_ticks,
        timeout_ceiling_millis,
        memory_ceiling_bytes,
        queue_wait_millis,
        logical_cpu_millis,
        fuel_consumed,
    }
}

#[allow(clippy::too_many_arguments)]
fn invocation_receipt_case(
    case_id: &str,
    status: TassadarPostArticlePluginInvocationReceiptCaseStatus,
    receipt_id: &str,
    invocation_id: &str,
    plugin_id: &str,
    plugin_version: &str,
    install_id: &str,
    artifact_digest: &str,
    export_name: &str,
    input_packet_digest: &str,
    output_packet_digest: Option<&str>,
    mount_envelope_id: &str,
    capability_envelope_id: &str,
    backend_id: &str,
    replay_class_id: &str,
    failure_class_id: Option<&str>,
    route_evidence_refs: &[&str],
    challenge_receipt_ref: Option<&str>,
    resource_summary: TassadarPostArticlePluginInvocationResourceSummary,
    note: &str,
) -> TassadarPostArticlePluginInvocationReceiptCase {
    let mut receipt = TassadarPostArticlePluginInvocationReceiptCase {
        case_id: String::from(case_id),
        status,
        receipt_id: String::from(receipt_id),
        invocation_id: String::from(invocation_id),
        plugin_id: String::from(plugin_id),
        plugin_version: String::from(plugin_version),
        install_id: String::from(install_id),
        artifact_digest: String::from(artifact_digest),
        export_name: String::from(export_name),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        input_packet_digest: String::from(input_packet_digest),
        output_packet_digest: output_packet_digest.map(String::from),
        mount_envelope_id: String::from(mount_envelope_id),
        capability_envelope_id: String::from(capability_envelope_id),
        backend_id: String::from(backend_id),
        replay_class_id: String::from(replay_class_id),
        failure_class_id: failure_class_id.map(String::from),
        route_evidence_refs: route_evidence_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        challenge_receipt_ref: challenge_receipt_ref.map(String::from),
        resource_summary,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_invocation_receipt_case|",
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
) -> Result<T, TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle,
        read_json, tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle_path,
        write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle,
        TassadarPostArticlePluginInvocationReceiptCaseStatus,
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundle,
        TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF,
        TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
    };

    #[test]
    fn post_article_plugin_invocation_receipt_bundle_keeps_fields_and_replay_classes_explicit() {
        let bundle =
            build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle();

        assert_eq!(
            bundle.profile_id,
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID
        );
        assert_eq!(bundle.receipt_field_rows.len(), 18);
        assert_eq!(bundle.replay_class_rows.len(), 4);
        assert_eq!(bundle.failure_class_rows.len(), 12);
        assert_eq!(bundle.case_receipts.len(), 7);
        assert_eq!(bundle.exact_success_case_count, 1);
        assert_eq!(bundle.exact_refusal_case_count, 4);
        assert_eq!(bundle.exact_failure_case_count, 2);
        assert_eq!(bundle.challenge_bound_case_count, 3);
        assert!(bundle.case_receipts.iter().any(|case| case.status
            == TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactFailureReceipt));
        assert!(bundle
            .replay_class_rows
            .iter()
            .any(|row| row.replay_class_id == "deterministic_replayable"
                && !row.challenge_receipt_required));
        assert!(bundle
            .replay_class_rows
            .iter()
            .any(|row| row.replay_class_id == "replayable_with_snapshots"
                && row.route_evidence_required));
        assert!(bundle
            .receipt_field_rows
            .iter()
            .any(|row| row.field_id == "challenge_receipt_ref" && !row.required));
    }

    #[test]
    fn post_article_plugin_invocation_receipt_bundle_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle();
        let committed: TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundle =
            read_json(
                tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle_path(),
            )
            .expect("committed bundle");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle_path()
                .strip_prefix(super::repo_root())
                .expect("relative bundle path")
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF
        );
    }

    #[test]
    fn write_post_article_plugin_invocation_receipt_bundle_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle.json",
        );
        let written =
            write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle(
                &output_path,
            )
            .expect("write bundle");
        let persisted: TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundle =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read bundle"))
                .expect("decode bundle");
        assert_eq!(written, persisted);
    }
}
