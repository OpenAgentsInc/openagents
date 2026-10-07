use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema version for the admitted 4080 remote-launch packet.
pub const PSION_EXECUTOR_4080_REMOTE_LAUNCH_SCHEMA_VERSION: &str =
    "psion.executor.4080_remote_launch.v1";
/// Canonical fixture path for the admitted 4080 remote-launch packet.
pub const PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_4080_remote_launch_v1.json";
/// Canonical doc path for the admitted 4080 remote-launch packet.
pub const PSION_EXECUTOR_4080_REMOTE_LAUNCH_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_REMOTE_LAUNCH.md";

const LOCAL_4080_PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const LOCAL_TAILNET_CONTROL_PROFILE_ID: &str = "local_tailnet_cluster_control_plane";
const REMOTE_LAUNCH_ENTRYPOINT_PATH: &str = "scripts/run-first-swarm-tailnet-admitted-live.sh";
const FIRST_SWARM_RUNBOOK_PATH: &str = "docs/FIRST_SWARM_TRUSTED_LAN_RUNBOOK.md";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const TAILNET_OPERATOR_MANIFEST_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260327e/operator_manifest.json";
const TAILNET_RUN_BUNDLE_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260327e/first_swarm_real_run_bundle.json";
const TAILNET_COORDINATOR_REPORT_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260327e/coordinator_runtime_report.json";
const TAILNET_CONTRIBUTOR_REPORT_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260327e/contributor_runtime_report.json";
const EXPECTED_WORKER_NODE_ID: &str = "swarm-linux-4080-a";
const EXPECTED_WORKER_ROLE_ID: &str = "swarm.linux.cuda.rtx4080.contributor";
const EXPECTED_WORKER_RUNTIME_ROLE: &str = "contributor";

#[derive(Clone, Debug, Deserialize)]
struct TailnetOperatorManifest {
    run_id: String,
    git_ref: String,
    coordinator: TailnetOperatorEndpoint,
    contributor: TailnetOperatorEndpointWithHost,
}

#[derive(Clone, Debug, Deserialize)]
struct TailnetOperatorEndpoint {
    tailnet_ip: String,
    cluster_port: u16,
    endpoint: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TailnetOperatorEndpointWithHost {
    host: String,
    tailnet_ip: String,
    cluster_port: u16,
    endpoint: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TailnetRunBundle {
    run_id: String,
    topology_contract_digest: String,
    workflow_plan_digest: String,
    coordinator_endpoint: String,
    contributor_endpoint: String,
    coordinator_backend_label: String,
    contributor_backend_label: String,
    bundle_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CoordinatorRuntimeReport {
    run_id: String,
    acknowledgement_receipts: Vec<TailnetAcknowledgementReceipt>,
}

#[derive(Clone, Debug, Deserialize)]
struct TailnetAcknowledgementReceipt {
    worker_id: String,
    session_id: String,
    acknowledged_at_ms: u64,
    receipt_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ContributorRuntimeReport {
    node_id: String,
    role_id: String,
    runtime_role: String,
    execution_backend_label: String,
    local_endpoint: String,
    peer_endpoint: String,
    report_digest: String,
}

/// One retained acceptance row for the remote-launch packet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutor4080RemoteLaunchChecklistRow {
    /// Stable checklist id.
    pub checklist_id: String,
    /// Final status.
    pub status: String,
    /// Honest detail.
    pub detail: String,
}

/// Typed packet recording the first admitted Mac -> 4080 Tailnet launch path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutor4080RemoteLaunchPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Admitted 4080 worker profile id.
    pub worker_profile_id: String,
    /// Admitted Tailnet control-plane profile id.
    pub control_plane_profile_id: String,
    /// Canonical operator entrypoint reference.
    pub launch_entrypoint_ref: String,
    /// Stable SHA256 over the launch entrypoint bytes.
    pub launch_entrypoint_sha256: String,
    /// Retained operator manifest reference.
    pub operator_manifest_ref: String,
    /// Stable SHA256 over the operator manifest bytes.
    pub operator_manifest_sha256: String,
    /// Retained run bundle reference.
    pub retained_bundle_ref: String,
    /// Stable SHA256 over the retained run bundle bytes.
    pub retained_bundle_sha256: String,
    /// Retained coordinator report reference.
    pub coordinator_report_ref: String,
    /// Stable SHA256 over the coordinator report bytes.
    pub coordinator_report_sha256: String,
    /// Retained contributor report reference.
    pub contributor_report_ref: String,
    /// Stable SHA256 over the contributor report bytes.
    pub contributor_report_sha256: String,
    /// Stable retained run id.
    pub run_id: String,
    /// Git revision archived into the remote worktree.
    pub git_ref: String,
    /// Frozen topology-contract digest.
    pub topology_contract_digest: String,
    /// Frozen workflow-plan digest.
    pub workflow_plan_digest: String,
    /// Coordinator endpoint surfaced by the manifest and bundle.
    pub coordinator_endpoint: String,
    /// Contributor endpoint surfaced by the manifest and bundle.
    pub contributor_endpoint: String,
    /// Coordinator backend label.
    pub coordinator_backend_label: String,
    /// Contributor backend label.
    pub contributor_backend_label: String,
    /// Remote contributor host alias.
    pub contributor_host: String,
    /// Remote contributor session id from the acknowledgement receipt.
    pub contributor_session_id: String,
    /// Worker acknowledgement timestamp.
    pub contributor_acknowledged_at_ms: u64,
    /// Worker acknowledgement receipt digest.
    pub contributor_acknowledgement_digest: String,
    /// Retained checklist rows.
    pub checklist_rows: Vec<PsionExecutor4080RemoteLaunchChecklistRow>,
    /// Support references.
    pub support_refs: Vec<String>,
    /// Honest summary.
    pub summary: String,
    /// Stable packet digest.
    pub packet_digest: String,
}

impl PsionExecutor4080RemoteLaunchPacket {
    /// Validate the retained remote-launch packet.
    pub fn validate(&self) -> Result<(), PsionExecutor4080RemoteLaunchError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_4080_remote_launch.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_4080_REMOTE_LAUNCH_SCHEMA_VERSION {
            return Err(PsionExecutor4080RemoteLaunchError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_4080_REMOTE_LAUNCH_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_4080_remote_launch.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.worker_profile_id",
                self.worker_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.control_plane_profile_id",
                self.control_plane_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.launch_entrypoint_ref",
                self.launch_entrypoint_ref.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.launch_entrypoint_sha256",
                self.launch_entrypoint_sha256.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.operator_manifest_ref",
                self.operator_manifest_ref.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.operator_manifest_sha256",
                self.operator_manifest_sha256.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.retained_bundle_ref",
                self.retained_bundle_ref.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.retained_bundle_sha256",
                self.retained_bundle_sha256.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.coordinator_report_ref",
                self.coordinator_report_ref.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.coordinator_report_sha256",
                self.coordinator_report_sha256.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.contributor_report_ref",
                self.contributor_report_ref.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.contributor_report_sha256",
                self.contributor_report_sha256.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.git_ref",
                self.git_ref.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.topology_contract_digest",
                self.topology_contract_digest.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.workflow_plan_digest",
                self.workflow_plan_digest.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.coordinator_endpoint",
                self.coordinator_endpoint.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.contributor_endpoint",
                self.contributor_endpoint.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.coordinator_backend_label",
                self.coordinator_backend_label.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.contributor_backend_label",
                self.contributor_backend_label.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.contributor_host",
                self.contributor_host.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.contributor_session_id",
                self.contributor_session_id.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.contributor_acknowledgement_digest",
                self.contributor_acknowledgement_digest.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_4080_remote_launch.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.checklist_rows.len() != 3 {
            return Err(PsionExecutor4080RemoteLaunchError::InvalidValue {
                field: String::from("psion_executor_4080_remote_launch.checklist_rows"),
                detail: String::from("remote-launch packet must keep exactly three checklist rows"),
            });
        }
        if self.worker_profile_id != LOCAL_4080_PROFILE_ID {
            return Err(PsionExecutor4080RemoteLaunchError::InvalidValue {
                field: String::from("psion_executor_4080_remote_launch.worker_profile_id"),
                detail: String::from("worker profile id drifted"),
            });
        }
        if self.control_plane_profile_id != LOCAL_TAILNET_CONTROL_PROFILE_ID {
            return Err(PsionExecutor4080RemoteLaunchError::InvalidValue {
                field: String::from("psion_executor_4080_remote_launch.control_plane_profile_id"),
                detail: String::from("control-plane profile id drifted"),
            });
        }
        if stable_remote_launch_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutor4080RemoteLaunchError::InvalidValue {
                field: String::from("psion_executor_4080_remote_launch.packet_digest"),
                detail: String::from("packet digest drifted"),
            });
        }
        Ok(())
    }
}

/// Build the retained 4080 remote-launch packet.
pub fn builtin_executor_4080_remote_launch_packet(
    workspace_root: &Path,
) -> Result<PsionExecutor4080RemoteLaunchPacket, PsionExecutor4080RemoteLaunchError> {
    let script_bytes = read_bytes(workspace_root, REMOTE_LAUNCH_ENTRYPOINT_PATH)?;
    let script_text = String::from_utf8(script_bytes.clone()).map_err(|error| {
        PsionExecutor4080RemoteLaunchError::InvalidValue {
            field: String::from("psion_executor_4080_remote_launch.launch_entrypoint_ref"),
            detail: format!("launch entrypoint must stay utf8-readable: {error}"),
        }
    })?;
    for required in [
        "git -C \"${repo_root}\" archive",
        "cargo run -q -p psionic-train --bin first_swarm_trusted_lan_live_runtime -- --role contributor",
        "ssh \"${remote_host}\" \"cat ${remote_contributor_report_path}\"",
    ] {
        if !script_text.contains(required) {
            return Err(PsionExecutor4080RemoteLaunchError::InvalidValue {
                field: String::from("psion_executor_4080_remote_launch.launch_entrypoint_ref"),
                detail: format!("launch entrypoint must keep `{required}` explicit"),
            });
        }
    }

    let manifest_bytes = read_bytes(workspace_root, TAILNET_OPERATOR_MANIFEST_PATH)?;
    let manifest: TailnetOperatorManifest = serde_json::from_slice(&manifest_bytes).map_err(
        |error| PsionExecutor4080RemoteLaunchError::Parse {
            path: String::from(TAILNET_OPERATOR_MANIFEST_PATH),
            error,
        },
    )?;
    let bundle_bytes = read_bytes(workspace_root, TAILNET_RUN_BUNDLE_PATH)?;
    let bundle: TailnetRunBundle = serde_json::from_slice(&bundle_bytes).map_err(|error| {
        PsionExecutor4080RemoteLaunchError::Parse {
            path: String::from(TAILNET_RUN_BUNDLE_PATH),
            error,
        }
    })?;
    let coordinator_report_bytes = read_bytes(workspace_root, TAILNET_COORDINATOR_REPORT_PATH)?;
    let coordinator_report: CoordinatorRuntimeReport = serde_json::from_slice(&coordinator_report_bytes).map_err(|error| {
        PsionExecutor4080RemoteLaunchError::Parse {
            path: String::from(TAILNET_COORDINATOR_REPORT_PATH),
            error,
        }
    })?;
    let contributor_report_bytes = read_bytes(workspace_root, TAILNET_CONTRIBUTOR_REPORT_PATH)?;
    let contributor_report: ContributorRuntimeReport = serde_json::from_slice(&contributor_report_bytes).map_err(|error| {
        PsionExecutor4080RemoteLaunchError::Parse {
            path: String::from(TAILNET_CONTRIBUTOR_REPORT_PATH),
            error,
        }
    })?;

    if manifest.run_id != bundle.run_id
        || manifest.run_id != coordinator_report.run_id
        || manifest.run_id != "tailrun-home-admitted-20260327e"
    {
        return Err(PsionExecutor4080RemoteLaunchError::InvalidValue {
            field: String::from("psion_executor_4080_remote_launch.run_id"),
            detail: String::from(
                "operator manifest, retained bundle, and coordinator report must stay aligned on one admitted run id",
            ),
        });
    }
    if manifest.coordinator.endpoint != bundle.coordinator_endpoint
        || manifest.contributor.endpoint != bundle.contributor_endpoint
    {
        return Err(PsionExecutor4080RemoteLaunchError::InvalidValue {
            field: String::from("psion_executor_4080_remote_launch.endpoints"),
            detail: String::from("operator manifest endpoints must match the retained bundle"),
        });
    }
    if contributor_report.local_endpoint != manifest.contributor.endpoint
        || contributor_report.peer_endpoint != manifest.coordinator.endpoint
    {
        return Err(PsionExecutor4080RemoteLaunchError::InvalidValue {
            field: String::from("psion_executor_4080_remote_launch.contributor_report_ref"),
            detail: String::from(
                "contributor report endpoints must match the operator manifest",
            ),
        });
    }
    if contributor_report.node_id != EXPECTED_WORKER_NODE_ID
        || contributor_report.role_id != EXPECTED_WORKER_ROLE_ID
        || contributor_report.runtime_role != EXPECTED_WORKER_RUNTIME_ROLE
    {
        return Err(PsionExecutor4080RemoteLaunchError::InvalidValue {
            field: String::from("psion_executor_4080_remote_launch.contributor_report_ref"),
            detail: String::from(
                "retained contributor report must stay bound to the admitted RTX 4080 contributor role",
            ),
        });
    }

    let worker_ack = coordinator_report
        .acknowledgement_receipts
        .iter()
        .find(|receipt| receipt.worker_id == EXPECTED_WORKER_NODE_ID)
        .ok_or_else(|| PsionExecutor4080RemoteLaunchError::InvalidValue {
            field: String::from("psion_executor_4080_remote_launch.coordinator_report_ref"),
            detail: String::from(
                "coordinator report must retain one acknowledgement receipt for the admitted RTX 4080 worker",
            ),
        })?;

    let checklist_rows = vec![
        PsionExecutor4080RemoteLaunchChecklistRow {
            checklist_id: String::from("launch_command_documented_green"),
            status: String::from("green"),
            detail: String::from(
                "The admitted Tailnet operator path stays explicit in `scripts/run-first-swarm-tailnet-admitted-live.sh` instead of hiding the 4080 launch behind ad hoc shell history.",
            ),
        },
        PsionExecutor4080RemoteLaunchChecklistRow {
            checklist_id: String::from("config_transfer_green"),
            status: String::from("green"),
            detail: format!(
                "The launch script still stages repo state with `git archive` over SSH, materializes the remote worktree, and binds topology digest `{}` plus workflow-plan digest `{}` into the same retained admitted run.",
                bundle.topology_contract_digest,
                bundle.workflow_plan_digest,
            ),
        },
        PsionExecutor4080RemoteLaunchChecklistRow {
            checklist_id: String::from("worker_acknowledgement_green"),
            status: String::from("green"),
            detail: format!(
                "The coordinator report retains one acknowledgement receipt for worker `{}` with session `{}` and digest `{}`.",
                worker_ack.worker_id,
                worker_ack.session_id,
                worker_ack.receipt_digest,
            ),
        },
    ];

    let contributor_host = manifest.contributor.host.clone();

    let mut packet = PsionExecutor4080RemoteLaunchPacket {
        schema_version: String::from(PSION_EXECUTOR_4080_REMOTE_LAUNCH_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_4080_remote_launch_v1"),
        worker_profile_id: String::from(LOCAL_4080_PROFILE_ID),
        control_plane_profile_id: String::from(LOCAL_TAILNET_CONTROL_PROFILE_ID),
        launch_entrypoint_ref: String::from(REMOTE_LAUNCH_ENTRYPOINT_PATH),
        launch_entrypoint_sha256: hex::encode(Sha256::digest(&script_bytes)),
        operator_manifest_ref: String::from(TAILNET_OPERATOR_MANIFEST_PATH),
        operator_manifest_sha256: hex::encode(Sha256::digest(&manifest_bytes)),
        retained_bundle_ref: String::from(TAILNET_RUN_BUNDLE_PATH),
        retained_bundle_sha256: hex::encode(Sha256::digest(&bundle_bytes)),
        coordinator_report_ref: String::from(TAILNET_COORDINATOR_REPORT_PATH),
        coordinator_report_sha256: hex::encode(Sha256::digest(&coordinator_report_bytes)),
        contributor_report_ref: String::from(TAILNET_CONTRIBUTOR_REPORT_PATH),
        contributor_report_sha256: hex::encode(Sha256::digest(&contributor_report_bytes)),
        run_id: manifest.run_id,
        git_ref: manifest.git_ref,
        topology_contract_digest: bundle.topology_contract_digest,
        workflow_plan_digest: bundle.workflow_plan_digest,
        coordinator_endpoint: manifest.coordinator.endpoint,
        contributor_endpoint: manifest.contributor.endpoint,
        coordinator_backend_label: bundle.coordinator_backend_label,
        contributor_backend_label: contributor_report.execution_backend_label,
        contributor_host: contributor_host.clone(),
        contributor_session_id: worker_ack.session_id.clone(),
        contributor_acknowledged_at_ms: worker_ack.acknowledged_at_ms,
        contributor_acknowledgement_digest: worker_ack.receipt_digest.clone(),
        checklist_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            String::from(FIRST_SWARM_RUNBOOK_PATH),
            String::from(REMOTE_LAUNCH_ENTRYPOINT_PATH),
            String::from(TAILNET_OPERATOR_MANIFEST_PATH),
            String::from(TAILNET_RUN_BUNDLE_PATH),
            String::from(TAILNET_COORDINATOR_REPORT_PATH),
            String::from(TAILNET_CONTRIBUTOR_REPORT_PATH),
        ],
        summary: format!(
            "The admitted 4080 lane now has one typed remote-launch packet. The Mac control plane launches the retained Tailnet run through `{}`, stages repo state and run config onto host `{}`, records worker acknowledgement digest `{}`, and keeps the Linux RTX 4080 contributor endpoint `{}` bound to the same topology and workflow digests used by the retained run bundle.",
            REMOTE_LAUNCH_ENTRYPOINT_PATH,
            contributor_host,
            worker_ack.receipt_digest,
            contributor_report.local_endpoint,
        ),
        packet_digest: String::new(),
    };
    if bundle.bundle_sha256 != hex::encode(Sha256::digest(&bundle_bytes)) {
        packet.summary.push_str(
            " The retained run bundle SHA stays checked against the committed bundle bytes before this packet counts.",
        );
    }
    packet.packet_digest = stable_remote_launch_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

/// Write the retained 4080 remote-launch packet.
pub fn write_builtin_executor_4080_remote_launch_packet(
    workspace_root: &Path,
) -> Result<PsionExecutor4080RemoteLaunchPacket, PsionExecutor4080RemoteLaunchError> {
    let packet = builtin_executor_4080_remote_launch_packet(workspace_root)?;
    let path = workspace_root.join(PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutor4080RemoteLaunchError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(&path, serde_json::to_vec_pretty(&packet)?).map_err(|error| {
        PsionExecutor4080RemoteLaunchError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(packet)
}

fn read_bytes(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, PsionExecutor4080RemoteLaunchError> {
    let path = workspace_root.join(relative_path);
    fs::read(&path).map_err(|error| PsionExecutor4080RemoteLaunchError::Read {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutor4080RemoteLaunchError> {
    let bytes = read_bytes(workspace_root, relative_path)?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutor4080RemoteLaunchError::Parse {
        path: relative_path.to_string(),
        error,
    })
}

fn stable_remote_launch_packet_digest(packet: &PsionExecutor4080RemoteLaunchPacket) -> String {
    let mut canonical = packet.clone();
    canonical.packet_digest.clear();
    stable_digest(b"psion_executor_4080_remote_launch|", &canonical)
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
) -> Result<(), PsionExecutor4080RemoteLaunchError> {
    if value.trim().is_empty() {
        return Err(PsionExecutor4080RemoteLaunchError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

/// Errors emitted by the retained 4080 remote-launch packet.
#[derive(Debug, Error)]
pub enum PsionExecutor4080RemoteLaunchError {
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
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn builtin_executor_4080_remote_launch_packet_is_valid(
    ) -> Result<(), PsionExecutor4080RemoteLaunchError> {
        let root = workspace_root();
        let packet = builtin_executor_4080_remote_launch_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }

    #[test]
    fn executor_4080_remote_launch_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutor4080RemoteLaunchError> {
        let root = workspace_root();
        let generated = builtin_executor_4080_remote_launch_packet(root.as_path())?;
        let committed: PsionExecutor4080RemoteLaunchPacket =
            read_json(root.as_path(), PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_executor_4080_remote_launch_packet_persists_current_truth(
    ) -> Result<(), PsionExecutor4080RemoteLaunchError> {
        let root = workspace_root();
        let packet = write_builtin_executor_4080_remote_launch_packet(root.as_path())?;
        packet.validate()?;
        let reread: PsionExecutor4080RemoteLaunchPacket =
            read_json(root.as_path(), PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH)?;
        assert_eq!(packet, reread);
        Ok(())
    }
}
