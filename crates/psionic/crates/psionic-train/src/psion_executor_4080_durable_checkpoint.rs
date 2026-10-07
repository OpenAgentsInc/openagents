use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ModelIoError, PortableModelBundle, PortableModelImportRequest, TensorMaterializationPolicy,
    PSION_EXECUTOR_4080_REMOTE_LAUNCH_DOC_PATH,
    PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH,
};

/// Stable schema version for the admitted 4080 durable-checkpoint packet.
pub const PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_SCHEMA_VERSION: &str =
    "psion.executor.4080_durable_checkpoint.v1";
/// Canonical fixture path for the admitted 4080 durable-checkpoint packet.
pub const PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_4080_durable_checkpoint_v1.json";
/// Canonical doc path for the admitted 4080 durable-checkpoint packet.
pub const PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_DURABLE_CHECKPOINT.md";

const LOCAL_4080_PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const LOCAL_TAILNET_CONTROL_PROFILE_ID: &str = "local_tailnet_cluster_control_plane";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const MIXED_BACKEND_CHECKPOINT_REFERENCE_DOC_PATH: &str =
    "docs/MIXED_BACKEND_CHECKPOINT_REFERENCE.md";
const MODEL_IO_REFERENCE_DOC_PATH: &str = "docs/MODEL_IO_REFERENCE.md";
const TAILNET_RUN_BUNDLE_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260328k/first_swarm_real_run_bundle.json";
const TAILNET_COORDINATOR_REPORT_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260328k/coordinator_runtime_report.json";
const TAILNET_CONTRIBUTOR_REPORT_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260328k/contributor_runtime_report.json";
const MERGED_ARTIFACT_REPORT_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260328k/retained_artifacts/merged_artifact_report.json";
const MERGED_PORTABLE_BUNDLE_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260328k/retained_artifacts/merged_portable_bundle.safetensors";
const EXPECTED_RUN_ID: &str = "tailrun-home-admitted-20260328k";
const EXPECTED_LINUX_WORKER_ID: &str = "swarm-linux-4080-a";
const EXPECTED_MAC_WORKER_ID: &str = "swarm-mac-a";

#[derive(Clone, Debug, Deserialize)]
struct TailnetRunBundle {
    run_id: String,
    topology_contract_digest: String,
    workflow_plan_digest: String,
    bundle_sha256: String,
    validator_summary_digest: String,
    promotion_receipt_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CoordinatorRuntimeReport {
    run_id: String,
    window_plan: WindowPlan,
    submission_receipts: Vec<SubmissionReceipt>,
}

#[derive(Clone, Debug, Deserialize)]
struct WindowPlan {
    input_checkpoint_pointer: InputCheckpointPointer,
}

#[derive(Clone, Debug, Deserialize)]
struct InputCheckpointPointer {
    scope: CheckpointScope,
    checkpoint_family: String,
    checkpoint: CheckpointReceipt,
    manifest_digest: String,
    pointer_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CheckpointScope {
    kind: String,
    scope_id: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CheckpointReceipt {
    checkpoint_family: String,
    checkpoint_ref: String,
    step: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct SubmissionReceipt {
    worker_id: String,
    source_checkpoint_pointer_digest: String,
    target_checkpoint_pointer_digest: String,
    receipt_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ContributorRuntimeReport {
    run_id: String,
    local_contribution: LocalContribution,
}

#[derive(Clone, Debug, Deserialize)]
struct LocalContribution {
    contributor_receipt: ContributorReceipt,
}

#[derive(Clone, Debug, Deserialize)]
struct ContributorReceipt {
    checkpoint_family: String,
    initial_state_dict_digest: String,
    final_state_dict_digest: String,
    receipt_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct MergedArtifactReport {
    run_id: String,
    local_contributor_receipt_digest: String,
    remote_contributor_receipt_digest: String,
    merged_portable_bundle_artifact_digest: String,
    merged_portable_bundle_state_dict_digest: String,
}

/// One retained acceptance row for the durable-checkpoint packet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutor4080DurableCheckpointChecklistRow {
    /// Stable checklist id.
    pub checklist_id: String,
    /// Final status.
    pub status: String,
    /// Honest detail.
    pub detail: String,
}

/// Typed packet recording the first durable checkpoint path for the admitted 4080 lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutor4080DurableCheckpointPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Admitted 4080 worker profile id.
    pub worker_profile_id: String,
    /// Admitted Tailnet control-plane profile id.
    pub control_plane_profile_id: String,
    /// Prerequisite remote-launch packet reference.
    pub remote_launch_packet_ref: String,
    /// Stable SHA256 over the remote-launch packet bytes.
    pub remote_launch_packet_sha256: String,
    /// Retained run bundle reference.
    pub retained_bundle_ref: String,
    /// Stable SHA256 over the retained run bundle bytes.
    pub retained_bundle_sha256: String,
    /// Retained coordinator report reference.
    pub coordinator_report_ref: String,
    /// Stable SHA256 over the retained coordinator report bytes.
    pub coordinator_report_sha256: String,
    /// Retained contributor report reference.
    pub contributor_report_ref: String,
    /// Stable SHA256 over the retained contributor report bytes.
    pub contributor_report_sha256: String,
    /// Retained merged-artifact report reference.
    pub merged_artifact_report_ref: String,
    /// Stable SHA256 over the retained merged-artifact report bytes.
    pub merged_artifact_report_sha256: String,
    /// Retained portable-bundle reference readable from the control plane.
    pub portable_bundle_ref: String,
    /// Stable SHA256 over the retained portable-bundle bytes.
    pub portable_bundle_sha256: String,
    /// Stable run id.
    pub run_id: String,
    /// Frozen topology-contract digest.
    pub topology_contract_digest: String,
    /// Frozen workflow-plan digest.
    pub workflow_plan_digest: String,
    /// Checkpoint family exposed by the contributor receipt.
    pub checkpoint_family: String,
    /// Checkpoint-scope kind from the control-plane pointer.
    pub checkpoint_scope_kind: String,
    /// Checkpoint-scope id from the control-plane pointer.
    pub checkpoint_scope_id: String,
    /// Stable checkpoint reference from the control-plane pointer.
    pub checkpoint_ref: String,
    /// Stable checkpoint manifest digest from the control-plane pointer.
    pub checkpoint_manifest_digest: String,
    /// Stable checkpoint pointer digest retained by both worker submissions.
    pub checkpoint_pointer_digest: String,
    /// Stable checkpoint step retained by the pointer.
    pub checkpoint_step: u64,
    /// Linux submission receipt digest.
    pub linux_submission_receipt_digest: String,
    /// Mac submission receipt digest.
    pub mac_submission_receipt_digest: String,
    /// Stable local contributor receipt digest retained by the merged-artifact report.
    pub local_contributor_receipt_digest: String,
    /// Stable remote contributor receipt digest retained by the merged-artifact report.
    pub remote_contributor_receipt_digest: String,
    /// Stable initial state-dict digest from the remote contributor receipt.
    pub initial_state_dict_digest: String,
    /// Stable final state-dict digest from the remote contributor receipt.
    pub final_state_dict_digest: String,
    /// Stable merged portable-bundle artifact digest.
    pub merged_portable_bundle_artifact_digest: String,
    /// Stable merged portable-bundle state-dict digest.
    pub merged_portable_bundle_state_dict_digest: String,
    /// Deferred import-plan digest proving control-plane readability.
    pub deferred_import_plan_digest: String,
    /// Deferred tensor count retained by the portable-bundle import plan.
    pub deferred_tensor_count: usize,
    /// Imported model family from the retained portable bundle.
    pub imported_model_family: String,
    /// Imported revision from the retained portable bundle.
    pub imported_revision: String,
    /// Imported checkpoint family from the retained portable bundle.
    pub imported_checkpoint_family: String,
    /// Imported state-dict digest from the retained portable bundle.
    pub imported_state_dict_digest: String,
    /// Stable compatibility-contract digest from the retained portable bundle.
    pub compatibility_contract_digest: String,
    /// Stable validator summary digest retained by the run bundle.
    pub validator_summary_digest: String,
    /// Stable promotion receipt digest retained by the run bundle.
    pub promotion_receipt_digest: String,
    /// Retained checklist rows.
    pub checklist_rows: Vec<PsionExecutor4080DurableCheckpointChecklistRow>,
    /// Support references.
    pub support_refs: Vec<String>,
    /// Honest summary.
    pub summary: String,
    /// Stable packet digest.
    pub packet_digest: String,
}

impl PsionExecutor4080DurableCheckpointPacket {
    /// Validate the retained durable-checkpoint packet.
    pub fn validate(&self) -> Result<(), PsionExecutor4080DurableCheckpointError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_4080_durable_checkpoint.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_SCHEMA_VERSION {
            return Err(
                PsionExecutor4080DurableCheckpointError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_4080_durable_checkpoint.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.worker_profile_id",
                self.worker_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.control_plane_profile_id",
                self.control_plane_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.remote_launch_packet_ref",
                self.remote_launch_packet_ref.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.remote_launch_packet_sha256",
                self.remote_launch_packet_sha256.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.retained_bundle_ref",
                self.retained_bundle_ref.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.retained_bundle_sha256",
                self.retained_bundle_sha256.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.coordinator_report_ref",
                self.coordinator_report_ref.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.coordinator_report_sha256",
                self.coordinator_report_sha256.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.contributor_report_ref",
                self.contributor_report_ref.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.contributor_report_sha256",
                self.contributor_report_sha256.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.merged_artifact_report_ref",
                self.merged_artifact_report_ref.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.merged_artifact_report_sha256",
                self.merged_artifact_report_sha256.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.portable_bundle_ref",
                self.portable_bundle_ref.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.portable_bundle_sha256",
                self.portable_bundle_sha256.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.topology_contract_digest",
                self.topology_contract_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.workflow_plan_digest",
                self.workflow_plan_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.checkpoint_family",
                self.checkpoint_family.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.checkpoint_scope_kind",
                self.checkpoint_scope_kind.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.checkpoint_scope_id",
                self.checkpoint_scope_id.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.checkpoint_ref",
                self.checkpoint_ref.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.checkpoint_manifest_digest",
                self.checkpoint_manifest_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.checkpoint_pointer_digest",
                self.checkpoint_pointer_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.linux_submission_receipt_digest",
                self.linux_submission_receipt_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.mac_submission_receipt_digest",
                self.mac_submission_receipt_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.local_contributor_receipt_digest",
                self.local_contributor_receipt_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.remote_contributor_receipt_digest",
                self.remote_contributor_receipt_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.initial_state_dict_digest",
                self.initial_state_dict_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.final_state_dict_digest",
                self.final_state_dict_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.merged_portable_bundle_artifact_digest",
                self.merged_portable_bundle_artifact_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.merged_portable_bundle_state_dict_digest",
                self.merged_portable_bundle_state_dict_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.deferred_import_plan_digest",
                self.deferred_import_plan_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.imported_model_family",
                self.imported_model_family.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.imported_revision",
                self.imported_revision.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.imported_checkpoint_family",
                self.imported_checkpoint_family.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.imported_state_dict_digest",
                self.imported_state_dict_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.compatibility_contract_digest",
                self.compatibility_contract_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.validator_summary_digest",
                self.validator_summary_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.promotion_receipt_digest",
                self.promotion_receipt_digest.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_4080_durable_checkpoint.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.worker_profile_id != LOCAL_4080_PROFILE_ID {
            return Err(PsionExecutor4080DurableCheckpointError::InvalidValue {
                field: String::from("psion_executor_4080_durable_checkpoint.worker_profile_id"),
                detail: String::from("worker profile id drifted"),
            });
        }
        if self.control_plane_profile_id != LOCAL_TAILNET_CONTROL_PROFILE_ID {
            return Err(PsionExecutor4080DurableCheckpointError::InvalidValue {
                field: String::from(
                    "psion_executor_4080_durable_checkpoint.control_plane_profile_id",
                ),
                detail: String::from("control-plane profile id drifted"),
            });
        }
        if self.checklist_rows.len() != 3 {
            return Err(PsionExecutor4080DurableCheckpointError::InvalidValue {
                field: String::from("psion_executor_4080_durable_checkpoint.checklist_rows"),
                detail: String::from(
                    "durable-checkpoint packet must keep exactly three checklist rows",
                ),
            });
        }
        if stable_durable_checkpoint_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutor4080DurableCheckpointError::InvalidValue {
                field: String::from("psion_executor_4080_durable_checkpoint.packet_digest"),
                detail: String::from("packet digest drifted"),
            });
        }
        Ok(())
    }
}

/// Build the retained 4080 durable-checkpoint packet.
pub fn builtin_executor_4080_durable_checkpoint_packet(
    workspace_root: &Path,
) -> Result<PsionExecutor4080DurableCheckpointPacket, PsionExecutor4080DurableCheckpointError> {
    let remote_launch_packet_bytes =
        read_bytes(workspace_root, PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH)?;
    let bundle_bytes = read_bytes(workspace_root, TAILNET_RUN_BUNDLE_PATH)?;
    let bundle: TailnetRunBundle = serde_json::from_slice(&bundle_bytes).map_err(|error| {
        PsionExecutor4080DurableCheckpointError::Parse {
            path: String::from(TAILNET_RUN_BUNDLE_PATH),
            error,
        }
    })?;
    let coordinator_report_bytes = read_bytes(workspace_root, TAILNET_COORDINATOR_REPORT_PATH)?;
    let coordinator_report: CoordinatorRuntimeReport =
        serde_json::from_slice(&coordinator_report_bytes).map_err(|error| {
            PsionExecutor4080DurableCheckpointError::Parse {
                path: String::from(TAILNET_COORDINATOR_REPORT_PATH),
                error,
            }
        })?;
    let contributor_report_bytes = read_bytes(workspace_root, TAILNET_CONTRIBUTOR_REPORT_PATH)?;
    let contributor_report: ContributorRuntimeReport =
        serde_json::from_slice(&contributor_report_bytes).map_err(|error| {
            PsionExecutor4080DurableCheckpointError::Parse {
                path: String::from(TAILNET_CONTRIBUTOR_REPORT_PATH),
                error,
            }
        })?;
    let merged_report_bytes = read_bytes(workspace_root, MERGED_ARTIFACT_REPORT_PATH)?;
    let merged_report: MergedArtifactReport =
        serde_json::from_slice(&merged_report_bytes).map_err(|error| {
            PsionExecutor4080DurableCheckpointError::Parse {
                path: String::from(MERGED_ARTIFACT_REPORT_PATH),
                error,
            }
        })?;
    let portable_bundle_bytes = read_bytes(workspace_root, MERGED_PORTABLE_BUNDLE_PATH)?;

    if bundle.run_id != EXPECTED_RUN_ID
        || coordinator_report.run_id != EXPECTED_RUN_ID
        || contributor_report.run_id != EXPECTED_RUN_ID
        || merged_report.run_id != EXPECTED_RUN_ID
    {
        return Err(PsionExecutor4080DurableCheckpointError::InvalidValue {
            field: String::from("psion_executor_4080_durable_checkpoint.run_id"),
            detail: String::from(
                "retained bundle, coordinator report, contributor report, and merged report must stay aligned on one admitted rerun id",
            ),
        });
    }

    let checkpoint_pointer = &coordinator_report.window_plan.input_checkpoint_pointer;
    if checkpoint_pointer.checkpoint_family != checkpoint_pointer.checkpoint.checkpoint_family {
        return Err(PsionExecutor4080DurableCheckpointError::InvalidValue {
            field: String::from(
                "psion_executor_4080_durable_checkpoint.checkpoint_family",
            ),
            detail: String::from(
                "window pointer family and checkpoint family must stay aligned",
            ),
        });
    }

    let linux_submission = coordinator_report
        .submission_receipts
        .iter()
        .find(|receipt| receipt.worker_id == EXPECTED_LINUX_WORKER_ID)
        .ok_or_else(|| PsionExecutor4080DurableCheckpointError::InvalidValue {
            field: String::from(
                "psion_executor_4080_durable_checkpoint.linux_submission_receipt_digest",
            ),
            detail: String::from(
                "coordinator report must retain one Linux 4080 submission receipt",
            ),
        })?;
    let mac_submission = coordinator_report
        .submission_receipts
        .iter()
        .find(|receipt| receipt.worker_id == EXPECTED_MAC_WORKER_ID)
        .ok_or_else(|| PsionExecutor4080DurableCheckpointError::InvalidValue {
            field: String::from(
                "psion_executor_4080_durable_checkpoint.mac_submission_receipt_digest",
            ),
            detail: String::from(
                "coordinator report must retain one Mac submission receipt",
            ),
        })?;
    for (worker_id, receipt) in [
        (EXPECTED_LINUX_WORKER_ID, linux_submission),
        (EXPECTED_MAC_WORKER_ID, mac_submission),
    ] {
        if receipt.source_checkpoint_pointer_digest != checkpoint_pointer.pointer_digest
            || receipt.target_checkpoint_pointer_digest != checkpoint_pointer.pointer_digest
        {
            return Err(PsionExecutor4080DurableCheckpointError::InvalidValue {
                field: String::from(
                    "psion_executor_4080_durable_checkpoint.checkpoint_pointer_digest",
                ),
                detail: format!(
                    "submission receipt for worker `{worker_id}` must preserve the retained pointer digest as both source and target"
                ),
            });
        }
    }

    let contributor_receipt = &contributor_report.local_contribution.contributor_receipt;
    if contributor_receipt.receipt_digest != merged_report.remote_contributor_receipt_digest {
        return Err(PsionExecutor4080DurableCheckpointError::InvalidValue {
            field: String::from(
                "psion_executor_4080_durable_checkpoint.remote_contributor_receipt_digest",
            ),
            detail: String::from(
                "merged report must stay bound to the remote contributor receipt retained in the contributor runtime report",
            ),
        });
    }
    if contributor_receipt.checkpoint_family
        != format!("swarm.local.open_adapter.policy:{EXPECTED_RUN_ID}")
    {
        return Err(PsionExecutor4080DurableCheckpointError::InvalidValue {
            field: String::from("psion_executor_4080_durable_checkpoint.checkpoint_family"),
            detail: String::from(
                "remote contributor receipt must stay on the retained admitted checkpoint family",
            ),
        });
    }
    if merged_report.merged_portable_bundle_artifact_digest
        != hex::encode(Sha256::digest(&portable_bundle_bytes))
    {
        return Err(PsionExecutor4080DurableCheckpointError::InvalidValue {
            field: String::from(
                "psion_executor_4080_durable_checkpoint.merged_portable_bundle_artifact_digest",
            ),
            detail: String::from(
                "merged report portable-bundle artifact digest drifted from the committed bundle bytes",
            ),
        });
    }

    let deferred_request = PortableModelImportRequest::new()
        .with_materialization_policy(TensorMaterializationPolicy::Deferred);
    let deferred_plan = PortableModelBundle::plan_safetensors_import(
        portable_bundle_bytes.as_slice(),
        &deferred_request,
    )?;
    let imported_bundle = PortableModelBundle::import_safetensors(portable_bundle_bytes.as_slice())?;
    if imported_bundle.state_dict.digest != merged_report.merged_portable_bundle_state_dict_digest {
        return Err(PsionExecutor4080DurableCheckpointError::InvalidValue {
            field: String::from(
                "psion_executor_4080_durable_checkpoint.merged_portable_bundle_state_dict_digest",
            ),
            detail: String::from(
                "imported portable bundle state-dict digest drifted from the merged-artifact report",
            ),
        });
    }
    let checklist_rows = vec![
        PsionExecutor4080DurableCheckpointChecklistRow {
            checklist_id: String::from("pointer_receipt_green"),
            status: String::from("green"),
            detail: format!(
                "The coordinator window plan retains checkpoint pointer digest `{}` with checkpoint ref `{}` at step {}.",
                checkpoint_pointer.pointer_digest,
                checkpoint_pointer.checkpoint.checkpoint_ref,
                checkpoint_pointer.checkpoint.step,
            ),
        },
        PsionExecutor4080DurableCheckpointChecklistRow {
            checklist_id: String::from("submission_resume_anchor_green"),
            status: String::from("green"),
            detail: format!(
                "Both worker submissions preserve pointer digest `{}` as both source and target, so the admitted rerun has one explicit resume anchor instead of ad hoc restart semantics.",
                checkpoint_pointer.pointer_digest,
            ),
        },
        PsionExecutor4080DurableCheckpointChecklistRow {
            checklist_id: String::from("control_plane_readback_green"),
            status: String::from("green"),
            detail: format!(
                "The merged portable bundle at `{}` imports on the control plane through deferred plan digest `{}` and reproduces state-dict digest `{}`.",
                MERGED_PORTABLE_BUNDLE_PATH,
                deferred_plan.plan_digest,
                imported_bundle.state_dict.digest,
            ),
        },
    ];

    let mut packet = PsionExecutor4080DurableCheckpointPacket {
        schema_version: String::from(PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_4080_durable_checkpoint_v1"),
        worker_profile_id: String::from(LOCAL_4080_PROFILE_ID),
        control_plane_profile_id: String::from(LOCAL_TAILNET_CONTROL_PROFILE_ID),
        remote_launch_packet_ref: String::from(PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH),
        remote_launch_packet_sha256: hex::encode(Sha256::digest(&remote_launch_packet_bytes)),
        retained_bundle_ref: String::from(TAILNET_RUN_BUNDLE_PATH),
        retained_bundle_sha256: hex::encode(Sha256::digest(&bundle_bytes)),
        coordinator_report_ref: String::from(TAILNET_COORDINATOR_REPORT_PATH),
        coordinator_report_sha256: hex::encode(Sha256::digest(&coordinator_report_bytes)),
        contributor_report_ref: String::from(TAILNET_CONTRIBUTOR_REPORT_PATH),
        contributor_report_sha256: hex::encode(Sha256::digest(&contributor_report_bytes)),
        merged_artifact_report_ref: String::from(MERGED_ARTIFACT_REPORT_PATH),
        merged_artifact_report_sha256: hex::encode(Sha256::digest(&merged_report_bytes)),
        portable_bundle_ref: String::from(MERGED_PORTABLE_BUNDLE_PATH),
        portable_bundle_sha256: hex::encode(Sha256::digest(&portable_bundle_bytes)),
        run_id: String::from(EXPECTED_RUN_ID),
        topology_contract_digest: bundle.topology_contract_digest,
        workflow_plan_digest: bundle.workflow_plan_digest,
        checkpoint_family: contributor_receipt.checkpoint_family.clone(),
        checkpoint_scope_kind: checkpoint_pointer.scope.kind.clone(),
        checkpoint_scope_id: checkpoint_pointer.scope.scope_id.clone(),
        checkpoint_ref: checkpoint_pointer.checkpoint.checkpoint_ref.clone(),
        checkpoint_manifest_digest: checkpoint_pointer.manifest_digest.clone(),
        checkpoint_pointer_digest: checkpoint_pointer.pointer_digest.clone(),
        checkpoint_step: checkpoint_pointer.checkpoint.step,
        linux_submission_receipt_digest: linux_submission.receipt_digest.clone(),
        mac_submission_receipt_digest: mac_submission.receipt_digest.clone(),
        local_contributor_receipt_digest: merged_report.local_contributor_receipt_digest,
        remote_contributor_receipt_digest: merged_report.remote_contributor_receipt_digest,
        initial_state_dict_digest: contributor_receipt.initial_state_dict_digest.clone(),
        final_state_dict_digest: contributor_receipt.final_state_dict_digest.clone(),
        merged_portable_bundle_artifact_digest: merged_report
            .merged_portable_bundle_artifact_digest,
        merged_portable_bundle_state_dict_digest: merged_report
            .merged_portable_bundle_state_dict_digest,
        deferred_import_plan_digest: deferred_plan.plan_digest.clone(),
        deferred_tensor_count: deferred_plan.deferred_tensor_count(),
        imported_model_family: imported_bundle.state_dict.model_family.clone(),
        imported_revision: imported_bundle.state_dict.revision.clone(),
        imported_checkpoint_family: imported_bundle.state_dict.checkpoint_family.clone(),
        imported_state_dict_digest: imported_bundle.state_dict.digest.clone(),
        compatibility_contract_digest: imported_bundle
            .compatibility_contract()
            .contract_digest,
        validator_summary_digest: bundle.validator_summary_digest,
        promotion_receipt_digest: bundle.promotion_receipt_digest,
        checklist_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_REMOTE_LAUNCH_DOC_PATH),
            String::from(MIXED_BACKEND_CHECKPOINT_REFERENCE_DOC_PATH),
            String::from(MODEL_IO_REFERENCE_DOC_PATH),
            String::from(TAILNET_RUN_BUNDLE_PATH),
            String::from(TAILNET_COORDINATOR_REPORT_PATH),
            String::from(TAILNET_CONTRIBUTOR_REPORT_PATH),
            String::from(MERGED_ARTIFACT_REPORT_PATH),
            String::from(MERGED_PORTABLE_BUNDLE_PATH),
        ],
        summary: format!(
            "The admitted 4080 lane now has one durable-checkpoint packet. The retained rerun keeps checkpoint pointer digest `{}` explicit in the control-plane window plan and in both worker submissions, the Linux 4080 contributor receipt stays on checkpoint family `{}`, and the control plane can re-read the merged portable bundle through deferred plan digest `{}` without inventing a second checkpoint path.",
            checkpoint_pointer.pointer_digest,
            contributor_receipt.checkpoint_family,
            deferred_plan.plan_digest,
        ),
        packet_digest: String::new(),
    };
    if bundle.bundle_sha256 != hex::encode(Sha256::digest(&bundle_bytes)) {
        packet.summary.push_str(
            " The in-band run-bundle digest and the file SHA remain distinct and both stay recorded.",
        );
    }
    packet.packet_digest = stable_durable_checkpoint_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

/// Write the retained 4080 durable-checkpoint packet.
pub fn write_builtin_executor_4080_durable_checkpoint_packet(
    workspace_root: &Path,
) -> Result<PsionExecutor4080DurableCheckpointPacket, PsionExecutor4080DurableCheckpointError> {
    let packet = builtin_executor_4080_durable_checkpoint_packet(workspace_root)?;
    let path = workspace_root.join(PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutor4080DurableCheckpointError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(&path, serde_json::to_vec_pretty(&packet)?).map_err(|error| {
        PsionExecutor4080DurableCheckpointError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(packet)
}

fn read_bytes(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, PsionExecutor4080DurableCheckpointError> {
    let path = workspace_root.join(relative_path);
    fs::read(&path).map_err(|error| PsionExecutor4080DurableCheckpointError::Read {
        path: path.display().to_string(),
        error,
    })
}

fn stable_durable_checkpoint_packet_digest(
    packet: &PsionExecutor4080DurableCheckpointPacket,
) -> String {
    let mut canonical = packet.clone();
    canonical.packet_digest.clear();
    stable_digest(b"psion_executor_4080_durable_checkpoint|", &canonical)
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutor4080DurableCheckpointError> {
    if value.trim().is_empty() {
        return Err(PsionExecutor4080DurableCheckpointError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

/// Errors emitted by the retained 4080 durable-checkpoint packet.
#[derive(Debug, Error)]
pub enum PsionExecutor4080DurableCheckpointError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected `{expected}`, got `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("failed to read `{path}`: {error}")]
    Read {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to parse `{path}`: {error}")]
    Parse {
        path: String,
        #[source]
        error: serde_json::Error,
    },
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to write `{path}`: {error}")]
    Write {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to encode packet: {0}")]
    Encode(#[from] serde_json::Error),
    #[error(transparent)]
    ModelIo(#[from] ModelIoError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_executor_4080_durable_checkpoint_packet_is_valid() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet = builtin_executor_4080_durable_checkpoint_packet(workspace_root.as_path())
            .expect("build durable checkpoint packet");
        packet.validate().expect("validate durable checkpoint packet");
        assert_eq!(packet.run_id, EXPECTED_RUN_ID);
        assert_eq!(
            packet.worker_profile_id,
            LOCAL_4080_PROFILE_ID,
            "packet should stay bound to the admitted 4080 profile"
        );
        assert_eq!(
            packet.control_plane_profile_id,
            LOCAL_TAILNET_CONTROL_PROFILE_ID,
            "packet should stay bound to the admitted Tailnet control-plane profile"
        );
        assert_eq!(
            packet.checkpoint_pointer_digest,
            "dd1aa85c355eb43934a20afe7e4204b3ed82bb85f4fe392dfde45229f4e434f8"
        );
    }

    #[test]
    fn executor_4080_durable_checkpoint_fixture_matches_committed_truth() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet = builtin_executor_4080_durable_checkpoint_packet(workspace_root.as_path())
            .expect("build durable checkpoint packet");
        let fixture_path =
            workspace_root.join(PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH);
        let fixture_bytes = fs::read(&fixture_path).expect("read durable checkpoint fixture");
        let fixture: PsionExecutor4080DurableCheckpointPacket =
            serde_json::from_slice(&fixture_bytes).expect("decode durable checkpoint fixture");
        assert_eq!(
            fixture, packet,
            "durable checkpoint fixture drifted; rerun the generator example"
        );
    }

    #[test]
    fn write_executor_4080_durable_checkpoint_packet_persists_current_truth() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet =
            write_builtin_executor_4080_durable_checkpoint_packet(workspace_root.as_path())
                .expect("write durable checkpoint packet");
        packet.validate().expect("validate written durable checkpoint packet");
    }
}
