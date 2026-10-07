use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FirstSwarmMacMlxBringupReport, SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH,
    SWARM_MAC_MLX_BRINGUP_SCOPE_WINDOW,
};

/// Stable schema version for the executor-lane MLX forward/load parity packet.
pub const PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_SCHEMA_VERSION: &str =
    "psion.executor.mlx_forward_load_parity.v1";
/// Canonical fixture path for the executor-lane MLX forward/load parity packet.
pub const PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_mlx_forward_load_parity_v1.json";
/// Canonical doc path for the executor-lane MLX forward/load parity packet.
pub const PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY.md";

const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const SWARM_MLX_BRINGUP_ENTRYPOINT_SOURCE: &str =
    "crates/psionic-train/src/bin/swarm_mac_mlx_bringup.rs";
const LOCAL_MAC_MLX_PROFILE_ID: &str = "local_mac_mlx_aarch64";

/// One bounded forward proof retained for the first executor-lane MLX packet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorMlxForwardProbe {
    /// Admitted op slice exercised by the proof.
    pub admitted_ops: Vec<String>,
    /// Logical device used by the proof.
    pub device_id: String,
    /// Stream id used by the proof.
    pub stream_id: u32,
    /// Evaluated output values for the bounded proof.
    pub output: Vec<f32>,
    /// Stable digest over the retained eval receipt.
    pub eval_receipt_digest: String,
    /// Human-readable summary.
    pub detail: String,
}

/// One explicit parity gap that remains outside the admitted MLX slice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMlxParityGap {
    /// Stable gap id.
    pub gap_id: String,
    /// Short category label.
    pub gap_kind: String,
    /// Operator-facing detail.
    pub detail: String,
}

/// Typed packet binding the shipped MLX bring-up lane into the executor roadmap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorMlxForwardLoadParityPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Admitted executor profile id that owns this packet.
    pub admitted_profile_id: String,
    /// Retained swarm scope window the packet is derived from.
    pub retained_scope_window: String,
    /// Concrete shipped entrypoint command used for the MLX lane.
    pub shipped_entrypoint_command: String,
    /// Concrete shipped entrypoint source file.
    pub shipped_entrypoint_source: String,
    /// Retained report path.
    pub bringup_report_ref: String,
    /// Stable SHA256 over the retained bring-up report bytes.
    pub bringup_report_sha256: String,
    /// Stable report digest embedded by the bring-up report.
    pub bringup_report_digest: String,
    /// Concrete backend label admitted for the packet.
    pub execution_backend_label: String,
    /// Stable logical-device kind surfaced by the load lane.
    pub logical_device_kind: String,
    /// Stable logical-device label surfaced by the load lane.
    pub logical_device_label: String,
    /// Stable adapter family used by the bounded converted-equivalent load lane.
    pub adapter_family: String,
    /// Stable checkpoint family emitted by the bounded converted-equivalent load lane.
    pub checkpoint_family: String,
    /// Stable adapter artifact digest emitted by the bounded converted-equivalent load lane.
    pub adapter_artifact_digest: String,
    /// Stable final state-dict digest emitted by the bounded converted-equivalent load lane.
    pub final_state_dict_digest: String,
    /// Retained forward probe for the admitted MLX slice.
    pub forward_probe: PsionExecutorMlxForwardProbe,
    /// Explicit parity gaps retained instead of hidden.
    pub explicit_parity_gaps: Vec<PsionExecutorMlxParityGap>,
    /// Support references used by this packet.
    pub support_refs: Vec<String>,
    /// Honest summary.
    pub summary: String,
    /// Stable packet digest.
    pub packet_digest: String,
}

impl PsionExecutorMlxForwardLoadParityPacket {
    /// Validate the retained packet.
    pub fn validate(&self) -> Result<(), PsionExecutorMlxForwardLoadParityError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_mlx_forward_load_parity.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_SCHEMA_VERSION {
            return Err(
                PsionExecutorMlxForwardLoadParityError::SchemaVersionMismatch {
                    expected: String::from(PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        ensure_nonempty(
            self.packet_id.as_str(),
            "psion_executor_mlx_forward_load_parity.packet_id",
        )?;
        ensure_nonempty(
            self.admitted_profile_id.as_str(),
            "psion_executor_mlx_forward_load_parity.admitted_profile_id",
        )?;
        ensure_nonempty(
            self.retained_scope_window.as_str(),
            "psion_executor_mlx_forward_load_parity.retained_scope_window",
        )?;
        ensure_nonempty(
            self.shipped_entrypoint_command.as_str(),
            "psion_executor_mlx_forward_load_parity.shipped_entrypoint_command",
        )?;
        ensure_nonempty(
            self.shipped_entrypoint_source.as_str(),
            "psion_executor_mlx_forward_load_parity.shipped_entrypoint_source",
        )?;
        ensure_nonempty(
            self.bringup_report_ref.as_str(),
            "psion_executor_mlx_forward_load_parity.bringup_report_ref",
        )?;
        ensure_nonempty(
            self.bringup_report_sha256.as_str(),
            "psion_executor_mlx_forward_load_parity.bringup_report_sha256",
        )?;
        ensure_nonempty(
            self.bringup_report_digest.as_str(),
            "psion_executor_mlx_forward_load_parity.bringup_report_digest",
        )?;
        ensure_nonempty(
            self.execution_backend_label.as_str(),
            "psion_executor_mlx_forward_load_parity.execution_backend_label",
        )?;
        ensure_nonempty(
            self.logical_device_kind.as_str(),
            "psion_executor_mlx_forward_load_parity.logical_device_kind",
        )?;
        ensure_nonempty(
            self.logical_device_label.as_str(),
            "psion_executor_mlx_forward_load_parity.logical_device_label",
        )?;
        ensure_nonempty(
            self.adapter_family.as_str(),
            "psion_executor_mlx_forward_load_parity.adapter_family",
        )?;
        ensure_nonempty(
            self.checkpoint_family.as_str(),
            "psion_executor_mlx_forward_load_parity.checkpoint_family",
        )?;
        ensure_nonempty(
            self.adapter_artifact_digest.as_str(),
            "psion_executor_mlx_forward_load_parity.adapter_artifact_digest",
        )?;
        ensure_nonempty(
            self.final_state_dict_digest.as_str(),
            "psion_executor_mlx_forward_load_parity.final_state_dict_digest",
        )?;
        if self.forward_probe.admitted_ops.is_empty() {
            return Err(PsionExecutorMlxForwardLoadParityError::MissingField {
                field: String::from(
                    "psion_executor_mlx_forward_load_parity.forward_probe.admitted_ops",
                ),
            });
        }
        ensure_nonempty(
            self.forward_probe.device_id.as_str(),
            "psion_executor_mlx_forward_load_parity.forward_probe.device_id",
        )?;
        ensure_nonempty(
            self.forward_probe.eval_receipt_digest.as_str(),
            "psion_executor_mlx_forward_load_parity.forward_probe.eval_receipt_digest",
        )?;
        ensure_nonempty(
            self.forward_probe.detail.as_str(),
            "psion_executor_mlx_forward_load_parity.forward_probe.detail",
        )?;
        if self.forward_probe.output.is_empty() {
            return Err(PsionExecutorMlxForwardLoadParityError::MissingField {
                field: String::from("psion_executor_mlx_forward_load_parity.forward_probe.output"),
            });
        }
        if self.explicit_parity_gaps.is_empty() {
            return Err(PsionExecutorMlxForwardLoadParityError::MissingField {
                field: String::from("psion_executor_mlx_forward_load_parity.explicit_parity_gaps"),
            });
        }
        for gap in &self.explicit_parity_gaps {
            ensure_nonempty(
                gap.gap_id.as_str(),
                "psion_executor_mlx_forward_load_parity.explicit_parity_gaps[].gap_id",
            )?;
            ensure_nonempty(
                gap.gap_kind.as_str(),
                "psion_executor_mlx_forward_load_parity.explicit_parity_gaps[].gap_kind",
            )?;
            ensure_nonempty(
                gap.detail.as_str(),
                "psion_executor_mlx_forward_load_parity.explicit_parity_gaps[].detail",
            )?;
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorMlxForwardLoadParityError::MissingField {
                field: String::from("psion_executor_mlx_forward_load_parity.support_refs"),
            });
        }
        ensure_nonempty(
            self.summary.as_str(),
            "psion_executor_mlx_forward_load_parity.summary",
        )?;
        if self.packet_digest != stable_executor_mlx_forward_load_parity_digest(self) {
            return Err(PsionExecutorMlxForwardLoadParityError::DigestMismatch);
        }
        Ok(())
    }
}

/// Errors surfaced while building or writing the executor-lane MLX parity packet.
#[derive(Debug, Error)]
pub enum PsionExecutorMlxForwardLoadParityError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("bring-up report at `{path}` is missing one required field: {detail}")]
    MissingBringupField { path: String, detail: String },
    #[error("packet digest mismatch")]
    DigestMismatch,
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to decode report `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to encode parity packet: {0}")]
    Encode(#[from] serde_json::Error),
}

/// Build the committed executor-lane MLX forward/load parity packet.
pub fn builtin_executor_mlx_forward_load_parity_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMlxForwardLoadParityPacket, PsionExecutorMlxForwardLoadParityError> {
    let report_path = workspace_root.join(SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH);
    let report_bytes =
        fs::read(&report_path).map_err(|error| PsionExecutorMlxForwardLoadParityError::Read {
            path: report_path.display().to_string(),
            error,
        })?;
    let report: FirstSwarmMacMlxBringupReport =
        serde_json::from_slice(&report_bytes).map_err(|error| {
            PsionExecutorMlxForwardLoadParityError::Decode {
                path: report_path.display().to_string(),
                error,
            }
        })?;
    let forward_probe = report.metal_eval_probe.clone().ok_or_else(|| {
        PsionExecutorMlxForwardLoadParityError::MissingBringupField {
            path: report_path.display().to_string(),
            detail: String::from("metal_eval_probe"),
        }
    })?;
    let overfit_gate = report.overfit_gate.clone().ok_or_else(|| {
        PsionExecutorMlxForwardLoadParityError::MissingBringupField {
            path: report_path.display().to_string(),
            detail: String::from("overfit_gate"),
        }
    })?;
    let mut packet = PsionExecutorMlxForwardLoadParityPacket {
        schema_version: String::from(PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_mlx_forward_load_parity_v1"),
        admitted_profile_id: String::from(LOCAL_MAC_MLX_PROFILE_ID),
        retained_scope_window: String::from(SWARM_MAC_MLX_BRINGUP_SCOPE_WINDOW),
        shipped_entrypoint_command: report.psionic_entrypoint.clone(),
        shipped_entrypoint_source: String::from(SWARM_MLX_BRINGUP_ENTRYPOINT_SOURCE),
        bringup_report_ref: String::from(SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH),
        bringup_report_sha256: hex::encode(Sha256::digest(&report_bytes)),
        bringup_report_digest: report.report_digest.clone(),
        execution_backend_label: overfit_gate.execution_backend_label.clone(),
        logical_device_kind: overfit_gate.logical_device_kind.clone(),
        logical_device_label: overfit_gate.logical_device_label.clone(),
        adapter_family: overfit_gate.adapter_family.clone(),
        checkpoint_family: overfit_gate.contributor_receipt.checkpoint_family.clone(),
        adapter_artifact_digest: overfit_gate.adapter_artifact_digest.clone(),
        final_state_dict_digest: overfit_gate.final_state_dict_digest.clone(),
        forward_probe: PsionExecutorMlxForwardProbe {
            admitted_ops: forward_probe.admitted_ops.clone(),
            device_id: forward_probe.device_id.clone(),
            stream_id: forward_probe.stream_id,
            output: forward_probe.output.clone(),
            eval_receipt_digest: forward_probe.eval_receipt_digest.clone(),
            detail: String::from(
                "The shipped `swarm_mac_mlx_bringup` entrypoint now proves one bounded MLX forward surface (`constant -> matmul -> add`) and one bounded converted-equivalent load path through the open-adapter MLX backend on `metal:0`.",
            ),
        },
        explicit_parity_gaps: vec![
            PsionExecutorMlxParityGap {
                gap_id: String::from("reshape_out_of_slice_refusal"),
                gap_kind: String::from("forward_surface_gap"),
                detail: forward_probe.out_of_slice_refusal.clone(),
            },
            PsionExecutorMlxParityGap {
                gap_id: String::from("bf16_precision_refusal"),
                gap_kind: String::from("precision_gap"),
                detail: overfit_gate.unsupported_precision_refusal.clone(),
            },
        ],
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            String::from(SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH),
        ],
        summary: format!(
            "The admitted Mac MLX executor profile now has one retained forward/load parity packet grounded in `{}`. The concrete shipped entrypoint is `{}`; it exercised admitted ops {:?}, loaded the bounded converted-equivalent `{}` adapter family on `{}`, and kept explicit parity gaps for reshape-backed graphs and bf16 mixed precision instead of hiding them.",
            SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH,
            report.psionic_entrypoint,
            forward_probe.admitted_ops,
            overfit_gate.adapter_family,
            overfit_gate.logical_device_label,
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_executor_mlx_forward_load_parity_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

/// Write the committed executor-lane MLX forward/load parity packet.
pub fn write_builtin_executor_mlx_forward_load_parity_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMlxForwardLoadParityPacket, PsionExecutorMlxForwardLoadParityError> {
    let packet = builtin_executor_mlx_forward_load_parity_packet(workspace_root)?;
    let fixture_path = workspace_root.join(PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_FIXTURE_PATH);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorMlxForwardLoadParityError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(&fixture_path, serde_json::to_vec_pretty(&packet)?).map_err(|error| {
        PsionExecutorMlxForwardLoadParityError::Write {
            path: fixture_path.display().to_string(),
            error,
        }
    })?;
    Ok(packet)
}

fn stable_executor_mlx_forward_load_parity_digest(
    packet: &PsionExecutorMlxForwardLoadParityPacket,
) -> String {
    let mut canonical = packet.clone();
    canonical.packet_digest.clear();
    stable_digest(b"psion_executor_mlx_forward_load_parity|", &canonical)
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

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorMlxForwardLoadParityError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorMlxForwardLoadParityError::MissingField {
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
        builtin_executor_mlx_forward_load_parity_packet,
        write_builtin_executor_mlx_forward_load_parity_packet,
        PsionExecutorMlxForwardLoadParityPacket,
        PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_FIXTURE_PATH,
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
    fn builtin_executor_mlx_forward_load_parity_packet_is_valid() -> Result<(), Box<dyn Error>> {
        let packet = builtin_executor_mlx_forward_load_parity_packet(workspace_root().as_path())?;
        packet.validate()?;
        assert_eq!(packet.admitted_profile_id, "local_mac_mlx_aarch64");
        assert_eq!(
            packet.execution_backend_label,
            "open_adapter_backend.mlx.metal.gpt_oss_lm_head"
        );
        assert_eq!(packet.forward_probe.output, vec![1.5, 2.5]);
        assert_eq!(packet.explicit_parity_gaps.len(), 2);
        Ok(())
    }

    #[test]
    fn executor_mlx_forward_load_parity_fixture_matches_committed_truth(
    ) -> Result<(), Box<dyn Error>> {
        let generated =
            builtin_executor_mlx_forward_load_parity_packet(workspace_root().as_path())?;
        let committed: PsionExecutorMlxForwardLoadParityPacket =
            read_json(workspace_root().join(PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_FIXTURE_PATH))?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_executor_mlx_forward_load_parity_packet_persists_current_truth(
    ) -> Result<(), Box<dyn Error>> {
        let temp = tempfile::tempdir()?;
        let temp_root = temp.path();
        let source_root = workspace_root();
        let source_report =
            source_root.join("fixtures/swarm/reports/swarm_mac_mlx_bringup_v1.json");
        let temp_report = temp_root.join("fixtures/swarm/reports/swarm_mac_mlx_bringup_v1.json");
        fs::create_dir_all(temp_report.parent().expect("temp report parent"))?;
        fs::copy(source_report, &temp_report)?;
        let packet = write_builtin_executor_mlx_forward_load_parity_packet(temp_root)?;
        let persisted: PsionExecutorMlxForwardLoadParityPacket =
            read_json(temp_root.join(PSION_EXECUTOR_MLX_FORWARD_LOAD_PARITY_FIXTURE_PATH))?;
        assert_eq!(packet, persisted);
        Ok(())
    }
}
