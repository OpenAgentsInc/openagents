use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ModelIoError, PortableModelBundle, PortableModelImportRequest, TensorMaterializationPolicy,
    PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_FIXTURE_PATH,
};

/// Stable schema version for the executor-lane MLX checkpoint compatibility packet.
pub const PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_SCHEMA_VERSION: &str =
    "psion.executor.mlx_checkpoint_compatibility.v1";
/// Canonical fixture path for the executor-lane MLX checkpoint compatibility packet.
pub const PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_mlx_checkpoint_compatibility_v1.json";
/// Canonical doc path for the executor-lane MLX checkpoint compatibility packet.
pub const PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY.md";

const LOCAL_MAC_MLX_PROFILE_ID: &str = "local_mac_mlx_aarch64";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const MODEL_IO_REFERENCE_DOC_PATH: &str = "docs/MODEL_IO_REFERENCE.md";
const M5_MLX_REPORT_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/report.json";
const M5_MLX_BUNDLE_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/portable_bundle.safetensors";

#[derive(Clone, Debug, Deserialize)]
struct OpenAdapterSameNodeWallclockBenchmarkReport {
    schema_version: String,
    backend_label: String,
    logical_device_kind: String,
    logical_device_label: String,
    retained_run: OpenAdapterSameNodeRetainedRunReport,
}

#[derive(Clone, Debug, Deserialize)]
struct OpenAdapterSameNodeRetainedRunReport {
    run_id: String,
    checkpoint_family: String,
    completed_steps: u64,
    final_state_dict_digest: String,
}

/// Retained model-IO restore facts for the executor-lane MLX checkpoint packet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorMlxRestoreFacts {
    /// Restored model family from the portable bundle.
    pub model_family: String,
    /// Restored revision from the portable bundle.
    pub revision: String,
    /// Restored checkpoint family from the portable bundle metadata.
    pub checkpoint_family: String,
    /// Restored state-dict digest from the portable bundle metadata.
    pub state_dict_digest: String,
    /// Restored training-group count.
    pub training_group_count: usize,
    /// Restored tensor count.
    pub tensor_count: usize,
    /// Restored tokenizer contract digest.
    pub tokenizer_contract_digest: String,
}

/// Typed packet binding the retained MLX checkpoint/export bundle into the executor roadmap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorMlxCheckpointCompatibilityPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Admitted executor profile id that owns the packet.
    pub admitted_profile_id: String,
    /// Forward/load parity packet used as the prerequisite boundary.
    pub forward_load_parity_ref: String,
    /// Stable SHA256 over the prerequisite packet bytes.
    pub forward_load_parity_sha256: String,
    /// Retained same-node report path.
    pub retained_report_ref: String,
    /// Stable SHA256 over the retained same-node report bytes.
    pub retained_report_sha256: String,
    /// Schema version carried by the retained same-node report.
    pub retained_report_schema_version: String,
    /// Stable backend label surfaced by the retained same-node report.
    pub execution_backend_label: String,
    /// Stable logical-device kind surfaced by the retained same-node report.
    pub logical_device_kind: String,
    /// Stable logical-device label surfaced by the retained same-node report.
    pub logical_device_label: String,
    /// Retained same-node run id.
    pub retained_run_id: String,
    /// Stable checkpoint family retained by the MLX run.
    pub checkpoint_family: String,
    /// Completed step count retained by the MLX run.
    pub completed_steps: u64,
    /// Retained portable bundle path.
    pub portable_bundle_ref: String,
    /// Stable SHA256 over the retained portable bundle bytes.
    pub portable_bundle_sha256: String,
    /// Deferred import-plan digest proving metadata-only compatibility.
    pub deferred_import_plan_digest: String,
    /// Deferred import-plan tensor count.
    pub deferred_import_tensor_count: usize,
    /// Deferred import-plan deferred tensor count.
    pub deferred_tensor_count: usize,
    /// Eager restore facts proving the bundle materializes back into canonical training groups.
    pub restore_facts: PsionExecutorMlxRestoreFacts,
    /// Stable compatibility-contract digest for the restored bundle.
    pub compatibility_contract_digest: String,
    /// Stable compatibility signature lines for audits and ledger packets.
    pub compatibility_signature_lines: Vec<String>,
    /// Support references used by the packet.
    pub support_refs: Vec<String>,
    /// Honest summary.
    pub summary: String,
    /// Stable packet digest.
    pub packet_digest: String,
}

impl PsionExecutorMlxCheckpointCompatibilityPacket {
    /// Validate the retained packet.
    pub fn validate(&self) -> Result<(), PsionExecutorMlxCheckpointCompatibilityError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_SCHEMA_VERSION {
            return Err(
                PsionExecutorMlxCheckpointCompatibilityError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        ensure_nonempty(
            self.packet_id.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.packet_id",
        )?;
        ensure_nonempty(
            self.admitted_profile_id.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.admitted_profile_id",
        )?;
        ensure_nonempty(
            self.forward_load_parity_ref.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.forward_load_parity_ref",
        )?;
        ensure_nonempty(
            self.forward_load_parity_sha256.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.forward_load_parity_sha256",
        )?;
        ensure_nonempty(
            self.retained_report_ref.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.retained_report_ref",
        )?;
        ensure_nonempty(
            self.retained_report_sha256.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.retained_report_sha256",
        )?;
        ensure_nonempty(
            self.retained_report_schema_version.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.retained_report_schema_version",
        )?;
        ensure_nonempty(
            self.execution_backend_label.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.execution_backend_label",
        )?;
        ensure_nonempty(
            self.logical_device_kind.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.logical_device_kind",
        )?;
        ensure_nonempty(
            self.logical_device_label.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.logical_device_label",
        )?;
        ensure_nonempty(
            self.retained_run_id.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.retained_run_id",
        )?;
        ensure_nonempty(
            self.checkpoint_family.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.checkpoint_family",
        )?;
        ensure_nonempty(
            self.portable_bundle_ref.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.portable_bundle_ref",
        )?;
        ensure_nonempty(
            self.portable_bundle_sha256.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.portable_bundle_sha256",
        )?;
        ensure_nonempty(
            self.deferred_import_plan_digest.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.deferred_import_plan_digest",
        )?;
        ensure_nonempty(
            self.restore_facts.model_family.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.restore_facts.model_family",
        )?;
        ensure_nonempty(
            self.restore_facts.revision.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.restore_facts.revision",
        )?;
        ensure_nonempty(
            self.restore_facts.checkpoint_family.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.restore_facts.checkpoint_family",
        )?;
        ensure_nonempty(
            self.restore_facts.state_dict_digest.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.restore_facts.state_dict_digest",
        )?;
        ensure_nonempty(
            self.restore_facts.tokenizer_contract_digest.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.restore_facts.tokenizer_contract_digest",
        )?;
        ensure_nonempty(
            self.compatibility_contract_digest.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.compatibility_contract_digest",
        )?;
        if self.compatibility_signature_lines.is_empty() {
            return Err(PsionExecutorMlxCheckpointCompatibilityError::MissingField {
                field: String::from(
                    "psion_executor_mlx_checkpoint_compatibility.compatibility_signature_lines",
                ),
            });
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorMlxCheckpointCompatibilityError::MissingField {
                field: String::from("psion_executor_mlx_checkpoint_compatibility.support_refs"),
            });
        }
        ensure_nonempty(
            self.summary.as_str(),
            "psion_executor_mlx_checkpoint_compatibility.summary",
        )?;
        if self.packet_digest != stable_executor_mlx_checkpoint_compatibility_digest(self) {
            return Err(PsionExecutorMlxCheckpointCompatibilityError::DigestMismatch);
        }
        Ok(())
    }
}

/// Errors surfaced while building or writing the executor-lane MLX checkpoint packet.
#[derive(Debug, Error)]
pub enum PsionExecutorMlxCheckpointCompatibilityError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("checkpoint report at `{path}` disagrees with restored bundle state-dict digest: report=`{report_digest}` restored=`{restored_digest}`")]
    StateDictDigestMismatch {
        path: String,
        report_digest: String,
        restored_digest: String,
    },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to decode JSON `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error("model-io boundary failed: {0}")]
    ModelIo(#[from] ModelIoError),
    #[error("failed to encode checkpoint packet: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("packet digest mismatch")]
    DigestMismatch,
}

/// Build the committed executor-lane MLX checkpoint compatibility packet.
pub fn builtin_executor_mlx_checkpoint_compatibility_packet(
    workspace_root: &Path,
) -> Result<
    PsionExecutorMlxCheckpointCompatibilityPacket,
    PsionExecutorMlxCheckpointCompatibilityError,
> {
    let forward_packet_path =
        workspace_root.join(PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_FIXTURE_PATH);
    let forward_packet_bytes = fs::read(&forward_packet_path).map_err(|error| {
        PsionExecutorMlxCheckpointCompatibilityError::Read {
            path: forward_packet_path.display().to_string(),
            error,
        }
    })?;
    let report_path = workspace_root.join(M5_MLX_REPORT_PATH);
    let report_bytes = fs::read(&report_path).map_err(|error| {
        PsionExecutorMlxCheckpointCompatibilityError::Read {
            path: report_path.display().to_string(),
            error,
        }
    })?;
    let report: OpenAdapterSameNodeWallclockBenchmarkReport = serde_json::from_slice(&report_bytes)
        .map_err(
            |error| PsionExecutorMlxCheckpointCompatibilityError::Decode {
                path: report_path.display().to_string(),
                error,
            },
        )?;
    let bundle_path = workspace_root.join(M5_MLX_BUNDLE_PATH);
    let bundle_bytes = fs::read(&bundle_path).map_err(|error| {
        PsionExecutorMlxCheckpointCompatibilityError::Read {
            path: bundle_path.display().to_string(),
            error,
        }
    })?;

    let deferred_request = PortableModelImportRequest::new()
        .with_materialization_policy(TensorMaterializationPolicy::Deferred);
    let deferred_plan =
        PortableModelBundle::plan_safetensors_import(bundle_bytes.as_slice(), &deferred_request)?;
    let restored_bundle = PortableModelBundle::import_safetensors(bundle_bytes.as_slice())?;
    let restored_groups = restored_bundle.to_training_groups()?;
    if restored_bundle.state_dict.digest != report.retained_run.final_state_dict_digest {
        return Err(
            PsionExecutorMlxCheckpointCompatibilityError::StateDictDigestMismatch {
                path: report_path.display().to_string(),
                report_digest: report.retained_run.final_state_dict_digest.clone(),
                restored_digest: restored_bundle.state_dict.digest.clone(),
            },
        );
    }
    let compatibility = restored_bundle.compatibility_contract();
    let retained_checkpoint_family = report.retained_run.checkpoint_family.clone();
    let mut packet = PsionExecutorMlxCheckpointCompatibilityPacket {
        schema_version: String::from(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_mlx_checkpoint_compatibility_v1"),
        admitted_profile_id: String::from(LOCAL_MAC_MLX_PROFILE_ID),
        forward_load_parity_ref: String::from(PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_FIXTURE_PATH),
        forward_load_parity_sha256: hex::encode(Sha256::digest(&forward_packet_bytes)),
        retained_report_ref: String::from(M5_MLX_REPORT_PATH),
        retained_report_sha256: hex::encode(Sha256::digest(&report_bytes)),
        retained_report_schema_version: report.schema_version,
        execution_backend_label: report.backend_label,
        logical_device_kind: report.logical_device_kind,
        logical_device_label: report.logical_device_label,
        retained_run_id: report.retained_run.run_id,
        checkpoint_family: retained_checkpoint_family.clone(),
        completed_steps: report.retained_run.completed_steps,
        portable_bundle_ref: String::from(M5_MLX_BUNDLE_PATH),
        portable_bundle_sha256: hex::encode(Sha256::digest(&bundle_bytes)),
        deferred_import_plan_digest: deferred_plan.plan_digest.clone(),
        deferred_import_tensor_count: deferred_plan.tensor_count(),
        deferred_tensor_count: deferred_plan.deferred_tensor_count(),
        restore_facts: PsionExecutorMlxRestoreFacts {
            model_family: restored_bundle.state_dict.model_family.clone(),
            revision: restored_bundle.state_dict.revision.clone(),
            checkpoint_family: restored_bundle.state_dict.checkpoint_family.clone(),
            state_dict_digest: restored_bundle.state_dict.digest.clone(),
            training_group_count: restored_groups.len(),
            tensor_count: restored_bundle.state_dict.tensors.len(),
            tokenizer_contract_digest: restored_bundle.tokenizer.contract_digest(),
        },
        compatibility_contract_digest: compatibility.contract_digest.clone(),
        compatibility_signature_lines: compatibility.stable_signature_lines(),
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            String::from(MODEL_IO_REFERENCE_DOC_PATH),
            String::from(M5_MLX_REPORT_PATH),
            String::from(M5_MLX_BUNDLE_PATH),
        ],
        summary: format!(
            "The admitted Mac MLX executor profile now has one checkpoint compatibility packet built from `{}` and `{}`. The retained MLX run kept checkpoint family `{}` through {} steps, the portable bundle imports through a deferred plan digest `{}`, eager restore reproduces state-dict digest `{}`, and the compatibility contract stays explicit instead of hiding model-IO boundaries.",
            M5_MLX_REPORT_PATH,
            M5_MLX_BUNDLE_PATH,
            retained_checkpoint_family,
            report.retained_run.completed_steps,
            deferred_plan.plan_digest,
            restored_bundle.state_dict.digest,
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_executor_mlx_checkpoint_compatibility_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

/// Write the committed executor-lane MLX checkpoint compatibility packet.
pub fn write_builtin_executor_mlx_checkpoint_compatibility_packet(
    workspace_root: &Path,
) -> Result<
    PsionExecutorMlxCheckpointCompatibilityPacket,
    PsionExecutorMlxCheckpointCompatibilityError,
> {
    let packet = builtin_executor_mlx_checkpoint_compatibility_packet(workspace_root)?;
    let fixture_path =
        workspace_root.join(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorMlxCheckpointCompatibilityError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(&fixture_path, serde_json::to_vec_pretty(&packet)?).map_err(|error| {
        PsionExecutorMlxCheckpointCompatibilityError::Write {
            path: fixture_path.display().to_string(),
            error,
        }
    })?;
    Ok(packet)
}

fn stable_executor_mlx_checkpoint_compatibility_digest(
    packet: &PsionExecutorMlxCheckpointCompatibilityPacket,
) -> String {
    let mut canonical = packet.clone();
    canonical.packet_digest.clear();
    stable_digest(b"psion_executor_mlx_checkpoint_compatibility|", &canonical)
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
) -> Result<(), PsionExecutorMlxCheckpointCompatibilityError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorMlxCheckpointCompatibilityError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs, path::PathBuf};

    use serde::de::DeserializeOwned;

    use super::{
        builtin_executor_mlx_checkpoint_compatibility_packet,
        write_builtin_executor_mlx_checkpoint_compatibility_packet,
        PsionExecutorMlxCheckpointCompatibilityPacket,
        PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH,
    };

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn read_json<T>(path: PathBuf) -> Result<T, Box<dyn Error>>
    where
        T: DeserializeOwned,
    {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    #[test]
    fn builtin_executor_mlx_checkpoint_compatibility_packet_is_valid() -> Result<(), Box<dyn Error>>
    {
        let packet =
            builtin_executor_mlx_checkpoint_compatibility_packet(workspace_root().as_path())?;
        packet.validate()?;
        assert_eq!(packet.admitted_profile_id, "local_mac_mlx_aarch64");
        assert_eq!(
            packet.execution_backend_label,
            "open_adapter_backend.mlx.metal.gpt_oss_lm_head"
        );
        assert!(packet.deferred_tensor_count > 0);
        assert_eq!(
            packet.restore_facts.state_dict_digest,
            "8e4bdfd3cd6c7a99cc574725a53b3e4e30a7b43e9813139be06e0b516b3065e8"
        );
        Ok(())
    }

    #[test]
    fn executor_mlx_checkpoint_compatibility_fixture_matches_committed_truth(
    ) -> Result<(), Box<dyn Error>> {
        let generated =
            builtin_executor_mlx_checkpoint_compatibility_packet(workspace_root().as_path())?;
        let committed: PsionExecutorMlxCheckpointCompatibilityPacket = read_json(
            workspace_root().join(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH),
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_executor_mlx_checkpoint_compatibility_packet_persists_current_truth(
    ) -> Result<(), Box<dyn Error>> {
        let temp = tempfile::tempdir()?;
        let temp_root = temp.path();
        let source_root = workspace_root();

        let source_forward_packet = source_root
            .join("fixtures/psion/executor/psion_executor_mlx_forward_load_parity_v1.json");
        let temp_forward_packet = temp_root
            .join("fixtures/psion/executor/psion_executor_mlx_forward_load_parity_v1.json");
        fs::create_dir_all(temp_forward_packet.parent().expect("forward packet parent"))?;
        fs::copy(source_forward_packet, &temp_forward_packet)?;

        let source_report =
            source_root.join("fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/report.json");
        let temp_report =
            temp_root.join("fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/report.json");
        fs::create_dir_all(temp_report.parent().expect("report parent"))?;
        fs::copy(source_report, &temp_report)?;

        let source_bundle =
            source_root.join("fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/portable_bundle.safetensors");
        let temp_bundle =
            temp_root.join("fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/portable_bundle.safetensors");
        fs::create_dir_all(temp_bundle.parent().expect("bundle parent"))?;
        fs::copy(source_bundle, &temp_bundle)?;

        let packet = write_builtin_executor_mlx_checkpoint_compatibility_packet(temp_root)?;
        let persisted: PsionExecutorMlxCheckpointCompatibilityPacket =
            read_json(temp_root.join(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH))?;
        assert_eq!(packet, persisted);
        Ok(())
    }
}
