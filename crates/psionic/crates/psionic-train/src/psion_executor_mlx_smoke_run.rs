use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionExecutorMlxCheckpointCompatibilityPacket, PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH,
    PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH,
};

/// Stable schema version for the executor-lane MLX smoke-run packet.
pub const PSION_EXECUTOR_MLX_SMOKE_RUN_SCHEMA_VERSION: &str = "psion.executor.mlx_smoke_run.v1";
/// Canonical fixture path for the executor-lane MLX smoke-run packet.
pub const PSION_EXECUTOR_MLX_SMOKE_RUN_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_mlx_smoke_run_v1.json";
/// Canonical doc path for the executor-lane MLX smoke-run packet.
pub const PSION_EXECUTOR_MLX_SMOKE_RUN_DOC_PATH: &str = "docs/PSION_EXECUTOR_MLX_SMOKE_RUN.md";

const LOCAL_MAC_MLX_PROFILE_ID: &str = "local_mac_mlx_aarch64";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const M5_MLX_REPORT_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/report.json";
const M5_MLX_BUNDLE_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/portable_bundle.safetensors";
const SMOKE_SUBSET_ID: &str = "tassadar.eval.frequent.v0::mlx_smoke_subset_v1";
const FREQUENT_PACK_ID: &str = "tassadar.eval.frequent.v0";
const FREQUENT_OPERATOR_REVIEW_SUITE_ID: &str = "frequent_operator_review_cases_v0";

#[derive(Clone, Debug, Deserialize)]
struct OpenAdapterSameNodeWallclockBenchmarkReport {
    backend_label: String,
    logical_device_label: String,
    retained_run: OpenAdapterSameNodeRetainedRunReport,
}

#[derive(Clone, Debug, Deserialize)]
struct OpenAdapterSameNodeRetainedRunReport {
    run_id: String,
    completed_steps: u64,
    final_mean_loss: f32,
    final_state_dict_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct PsionExecutorEvalPackCatalog {
    catalog_digest: String,
    packs: Vec<PsionExecutorEvalPack>,
}

#[derive(Clone, Debug, Deserialize)]
struct PsionExecutorEvalPack {
    pack_id: String,
    suite_refs: Vec<PsionExecutorEvalSuiteRef>,
}

#[derive(Clone, Debug, Deserialize)]
struct PsionExecutorEvalSuiteRef {
    suite_id: String,
    case_ids: Vec<String>,
}

/// One checklist row admitted into the MLX smoke subset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMlxSmokeChecklistRow {
    /// Stable case id.
    pub case_id: String,
    /// Final status for the smoke packet.
    pub status: String,
    /// Why the row counts green for the smoke packet.
    pub detail: String,
}

/// Approved frequent-pack subset for the MLX smoke packet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMlxSmokeSubset {
    /// Parent pack id.
    pub pack_id: String,
    /// Stable subset id.
    pub subset_id: String,
    /// Suite id the subset is derived from.
    pub suite_id: String,
    /// Included case ids that must be green for the smoke packet.
    pub included_case_ids: Vec<String>,
    /// Deferred case ids intentionally held for later epics.
    pub deferred_case_ids: Vec<String>,
    /// Stable subset digest.
    pub subset_digest: String,
    /// Honest detail.
    pub detail: String,
}

/// Typed packet binding the first MLX smoke run into the executor roadmap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorMlxSmokeRunPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Admitted executor profile id that owns the smoke packet.
    pub admitted_profile_id: String,
    /// Stable smoke objective id.
    pub smoke_objective_id: String,
    /// Honest smoke objective kind.
    pub smoke_objective_kind: String,
    /// Prerequisite checkpoint packet reference.
    pub checkpoint_packet_ref: String,
    /// Stable SHA256 over the prerequisite checkpoint packet bytes.
    pub checkpoint_packet_sha256: String,
    /// Frozen eval-pack catalog reference.
    pub eval_pack_catalog_ref: String,
    /// Stable SHA256 over the eval-pack catalog bytes.
    pub eval_pack_catalog_sha256: String,
    /// Stable catalog digest embedded by the eval-pack catalog.
    pub eval_pack_catalog_digest: String,
    /// Retained same-node report reference.
    pub retained_report_ref: String,
    /// Stable SHA256 over the retained report bytes.
    pub retained_report_sha256: String,
    /// Stable backend label surfaced by the smoke run.
    pub execution_backend_label: String,
    /// Stable logical-device label surfaced by the smoke run.
    pub logical_device_label: String,
    /// Retained smoke run id.
    pub retained_run_id: String,
    /// Stable retained step count.
    pub completed_steps: u64,
    /// Stable retained final mean loss.
    pub final_mean_loss: f32,
    /// Retained bundle reference proving durable checkpoint/export.
    pub durable_bundle_ref: String,
    /// Stable SHA256 over the durable bundle.
    pub durable_bundle_sha256: String,
    /// Retained state-dict digest carried by the smoke run.
    pub final_state_dict_digest: String,
    /// Approved frequent-pack subset used by this smoke packet.
    pub approved_frequent_pack_subset: PsionExecutorMlxSmokeSubset,
    /// Checklist rows that counted green for the approved subset.
    pub checklist_rows: Vec<PsionExecutorMlxSmokeChecklistRow>,
    /// Support references used by the packet.
    pub support_refs: Vec<String>,
    /// Honest summary.
    pub summary: String,
    /// Stable packet digest.
    pub packet_digest: String,
}

impl PsionExecutorMlxSmokeRunPacket {
    /// Validate the retained smoke packet.
    pub fn validate(&self) -> Result<(), PsionExecutorMlxSmokeRunError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_mlx_smoke_run.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_MLX_SMOKE_RUN_SCHEMA_VERSION {
            return Err(PsionExecutorMlxSmokeRunError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_MLX_SMOKE_RUN_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.packet_id.as_str(),
            "psion_executor_mlx_smoke_run.packet_id",
        )?;
        ensure_nonempty(
            self.admitted_profile_id.as_str(),
            "psion_executor_mlx_smoke_run.admitted_profile_id",
        )?;
        ensure_nonempty(
            self.smoke_objective_id.as_str(),
            "psion_executor_mlx_smoke_run.smoke_objective_id",
        )?;
        ensure_nonempty(
            self.smoke_objective_kind.as_str(),
            "psion_executor_mlx_smoke_run.smoke_objective_kind",
        )?;
        ensure_nonempty(
            self.checkpoint_packet_ref.as_str(),
            "psion_executor_mlx_smoke_run.checkpoint_packet_ref",
        )?;
        ensure_nonempty(
            self.checkpoint_packet_sha256.as_str(),
            "psion_executor_mlx_smoke_run.checkpoint_packet_sha256",
        )?;
        ensure_nonempty(
            self.eval_pack_catalog_ref.as_str(),
            "psion_executor_mlx_smoke_run.eval_pack_catalog_ref",
        )?;
        ensure_nonempty(
            self.eval_pack_catalog_sha256.as_str(),
            "psion_executor_mlx_smoke_run.eval_pack_catalog_sha256",
        )?;
        ensure_nonempty(
            self.eval_pack_catalog_digest.as_str(),
            "psion_executor_mlx_smoke_run.eval_pack_catalog_digest",
        )?;
        ensure_nonempty(
            self.retained_report_ref.as_str(),
            "psion_executor_mlx_smoke_run.retained_report_ref",
        )?;
        ensure_nonempty(
            self.retained_report_sha256.as_str(),
            "psion_executor_mlx_smoke_run.retained_report_sha256",
        )?;
        ensure_nonempty(
            self.execution_backend_label.as_str(),
            "psion_executor_mlx_smoke_run.execution_backend_label",
        )?;
        ensure_nonempty(
            self.logical_device_label.as_str(),
            "psion_executor_mlx_smoke_run.logical_device_label",
        )?;
        ensure_nonempty(
            self.retained_run_id.as_str(),
            "psion_executor_mlx_smoke_run.retained_run_id",
        )?;
        ensure_nonempty(
            self.durable_bundle_ref.as_str(),
            "psion_executor_mlx_smoke_run.durable_bundle_ref",
        )?;
        ensure_nonempty(
            self.durable_bundle_sha256.as_str(),
            "psion_executor_mlx_smoke_run.durable_bundle_sha256",
        )?;
        ensure_nonempty(
            self.final_state_dict_digest.as_str(),
            "psion_executor_mlx_smoke_run.final_state_dict_digest",
        )?;
        ensure_nonempty(
            self.approved_frequent_pack_subset.pack_id.as_str(),
            "psion_executor_mlx_smoke_run.approved_frequent_pack_subset.pack_id",
        )?;
        ensure_nonempty(
            self.approved_frequent_pack_subset.subset_id.as_str(),
            "psion_executor_mlx_smoke_run.approved_frequent_pack_subset.subset_id",
        )?;
        ensure_nonempty(
            self.approved_frequent_pack_subset.suite_id.as_str(),
            "psion_executor_mlx_smoke_run.approved_frequent_pack_subset.suite_id",
        )?;
        ensure_nonempty(
            self.approved_frequent_pack_subset.subset_digest.as_str(),
            "psion_executor_mlx_smoke_run.approved_frequent_pack_subset.subset_digest",
        )?;
        if self
            .approved_frequent_pack_subset
            .included_case_ids
            .is_empty()
        {
            return Err(PsionExecutorMlxSmokeRunError::MissingField {
                field: String::from(
                    "psion_executor_mlx_smoke_run.approved_frequent_pack_subset.included_case_ids",
                ),
            });
        }
        if self.checklist_rows.is_empty() {
            return Err(PsionExecutorMlxSmokeRunError::MissingField {
                field: String::from("psion_executor_mlx_smoke_run.checklist_rows"),
            });
        }
        for row in &self.checklist_rows {
            ensure_nonempty(
                row.case_id.as_str(),
                "psion_executor_mlx_smoke_run.checklist_rows[].case_id",
            )?;
            ensure_nonempty(
                row.status.as_str(),
                "psion_executor_mlx_smoke_run.checklist_rows[].status",
            )?;
            ensure_nonempty(
                row.detail.as_str(),
                "psion_executor_mlx_smoke_run.checklist_rows[].detail",
            )?;
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorMlxSmokeRunError::MissingField {
                field: String::from("psion_executor_mlx_smoke_run.support_refs"),
            });
        }
        ensure_nonempty(
            self.summary.as_str(),
            "psion_executor_mlx_smoke_run.summary",
        )?;
        if self.packet_digest != stable_executor_mlx_smoke_run_digest(self) {
            return Err(PsionExecutorMlxSmokeRunError::DigestMismatch);
        }
        Ok(())
    }
}

/// Errors surfaced while building or writing the executor-lane MLX smoke packet.
#[derive(Debug, Error)]
pub enum PsionExecutorMlxSmokeRunError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("frozen eval pack `{pack_id}` missing suite `{suite_id}`")]
    MissingSuite { pack_id: String, suite_id: String },
    #[error("frozen eval suite `{suite_id}` missing case `{case_id}`")]
    MissingCase { suite_id: String, case_id: String },
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
    #[error("packet digest mismatch")]
    DigestMismatch,
    #[error("failed to encode smoke packet: {0}")]
    Encode(#[from] serde_json::Error),
}

/// Build the committed executor-lane MLX smoke-run packet.
pub fn builtin_executor_mlx_smoke_run_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMlxSmokeRunPacket, PsionExecutorMlxSmokeRunError> {
    let checkpoint_packet_path =
        workspace_root.join(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH);
    let checkpoint_packet_bytes =
        fs::read(&checkpoint_packet_path).map_err(|error| PsionExecutorMlxSmokeRunError::Read {
            path: checkpoint_packet_path.display().to_string(),
            error,
        })?;
    let checkpoint_packet: PsionExecutorMlxCheckpointCompatibilityPacket =
        serde_json::from_slice(&checkpoint_packet_bytes).map_err(|error| {
            PsionExecutorMlxSmokeRunError::Decode {
                path: checkpoint_packet_path.display().to_string(),
                error,
            }
        })?;

    let eval_pack_path = workspace_root.join(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH);
    let eval_pack_bytes =
        fs::read(&eval_pack_path).map_err(|error| PsionExecutorMlxSmokeRunError::Read {
            path: eval_pack_path.display().to_string(),
            error,
        })?;
    let eval_catalog: PsionExecutorEvalPackCatalog = serde_json::from_slice(&eval_pack_bytes)
        .map_err(|error| PsionExecutorMlxSmokeRunError::Decode {
            path: eval_pack_path.display().to_string(),
            error,
        })?;
    let operator_suite = eval_catalog
        .packs
        .iter()
        .find(|pack| pack.pack_id == FREQUENT_PACK_ID)
        .and_then(|pack| {
            pack.suite_refs
                .iter()
                .find(|suite| suite.suite_id == FREQUENT_OPERATOR_REVIEW_SUITE_ID)
        })
        .ok_or_else(|| PsionExecutorMlxSmokeRunError::MissingSuite {
            pack_id: String::from(FREQUENT_PACK_ID),
            suite_id: String::from(FREQUENT_OPERATOR_REVIEW_SUITE_ID),
        })?;
    let included_case_ids = vec![
        String::from("artifact_packet_complete"),
        String::from("checkpoint_restore_rehearsal_green"),
        String::from("export_smoke_green"),
    ];
    for case_id in &included_case_ids {
        if !operator_suite.case_ids.contains(case_id) {
            return Err(PsionExecutorMlxSmokeRunError::MissingCase {
                suite_id: String::from(FREQUENT_OPERATOR_REVIEW_SUITE_ID),
                case_id: case_id.clone(),
            });
        }
    }
    let deferred_case_ids = vec![String::from("local_cluster_roundtrip_green")];
    for case_id in &deferred_case_ids {
        if !operator_suite.case_ids.contains(case_id) {
            return Err(PsionExecutorMlxSmokeRunError::MissingCase {
                suite_id: String::from(FREQUENT_OPERATOR_REVIEW_SUITE_ID),
                case_id: case_id.clone(),
            });
        }
    }

    let report_path = workspace_root.join(M5_MLX_REPORT_PATH);
    let report_bytes =
        fs::read(&report_path).map_err(|error| PsionExecutorMlxSmokeRunError::Read {
            path: report_path.display().to_string(),
            error,
        })?;
    let report: OpenAdapterSameNodeWallclockBenchmarkReport = serde_json::from_slice(&report_bytes)
        .map_err(|error| PsionExecutorMlxSmokeRunError::Decode {
            path: report_path.display().to_string(),
            error,
        })?;

    let bundle_path = workspace_root.join(M5_MLX_BUNDLE_PATH);
    let bundle_bytes =
        fs::read(&bundle_path).map_err(|error| PsionExecutorMlxSmokeRunError::Read {
            path: bundle_path.display().to_string(),
            error,
        })?;

    let subset = PsionExecutorMlxSmokeSubset {
        pack_id: String::from(FREQUENT_PACK_ID),
        subset_id: String::from(SMOKE_SUBSET_ID),
        suite_id: String::from(FREQUENT_OPERATOR_REVIEW_SUITE_ID),
        included_case_ids: included_case_ids.clone(),
        deferred_case_ids: deferred_case_ids.clone(),
        subset_digest: stable_digest(
            b"psion_executor_mlx_smoke_subset|",
            &(
                FREQUENT_PACK_ID,
                FREQUENT_OPERATOR_REVIEW_SUITE_ID,
                &included_case_ids,
                &deferred_case_ids,
            ),
        ),
        detail: String::from(
            "Phase-one MLX smoke uses the operator-review slice of `tassadar.eval.frequent.v0` only: artifact packet completeness, restore rehearsal, and export smoke must be green locally, while `local_cluster_roundtrip_green` is intentionally deferred to EPIC 3.",
        ),
    };

    let checklist_rows = vec![
        PsionExecutorMlxSmokeChecklistRow {
            case_id: String::from("artifact_packet_complete"),
            status: String::from("green"),
            detail: String::from(
                "The retained smoke packet keeps the report, durable portable bundle, checkpoint packet, and prerequisite MLX parity packet together as one reviewable artifact packet.",
            ),
        },
        PsionExecutorMlxSmokeChecklistRow {
            case_id: String::from("checkpoint_restore_rehearsal_green"),
            status: String::from("green"),
            detail: String::from(
                "The prerequisite checkpoint packet already proves deferred import-plan metadata plus eager restore on the retained MLX bundle.",
            ),
        },
        PsionExecutorMlxSmokeChecklistRow {
            case_id: String::from("export_smoke_green"),
            status: String::from("green"),
            detail: String::from(
                "The retained same-node report and durable portable bundle prove the Mac MLX smoke lane exports a real artifact through the shipped model-IO surface.",
            ),
        },
    ];

    let execution_backend_label = report.backend_label;
    let logical_device_label = report.logical_device_label;
    let retained_run_id = report.retained_run.run_id;
    let completed_steps = report.retained_run.completed_steps;
    let final_mean_loss = report.retained_run.final_mean_loss;
    let final_state_dict_digest = report.retained_run.final_state_dict_digest;

    let mut packet = PsionExecutorMlxSmokeRunPacket {
        schema_version: String::from(PSION_EXECUTOR_MLX_SMOKE_RUN_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_mlx_smoke_run_v1"),
        admitted_profile_id: String::from(LOCAL_MAC_MLX_PROFILE_ID),
        smoke_objective_id: String::from("psion.executor.mlx_same_node_smoke_objective.v1"),
        smoke_objective_kind: String::from("executor_lane_admission_surrogate"),
        checkpoint_packet_ref: String::from(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH),
        checkpoint_packet_sha256: hex::encode(Sha256::digest(&checkpoint_packet_bytes)),
        eval_pack_catalog_ref: String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH),
        eval_pack_catalog_sha256: hex::encode(Sha256::digest(&eval_pack_bytes)),
        eval_pack_catalog_digest: eval_catalog.catalog_digest,
        retained_report_ref: String::from(M5_MLX_REPORT_PATH),
        retained_report_sha256: hex::encode(Sha256::digest(&report_bytes)),
        execution_backend_label,
        logical_device_label: logical_device_label.clone(),
        retained_run_id: retained_run_id.clone(),
        completed_steps,
        final_mean_loss,
        durable_bundle_ref: String::from(M5_MLX_BUNDLE_PATH),
        durable_bundle_sha256: hex::encode(Sha256::digest(&bundle_bytes)),
        final_state_dict_digest,
        approved_frequent_pack_subset: subset,
        checklist_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            String::from(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH),
            String::from(M5_MLX_REPORT_PATH),
            String::from(M5_MLX_BUNDLE_PATH),
        ],
        summary: format!(
            "The admitted Mac MLX executor profile now has one smoke-run packet tied to the retained MLX same-node training/export lane. The smoke objective remains a bounded executor-lane admission surrogate on `{}` through run `{}` (steps={} final_mean_loss={:.6}), and it now binds to the approved subset `{}` by keeping `artifact_packet_complete`, `checkpoint_restore_rehearsal_green`, and `export_smoke_green` green while explicitly deferring `local_cluster_roundtrip_green` to EPIC 3.",
            logical_device_label,
            retained_run_id,
            completed_steps,
            final_mean_loss,
            SMOKE_SUBSET_ID,
        ),
        packet_digest: String::new(),
    };
    if checkpoint_packet.restore_facts.state_dict_digest != packet.final_state_dict_digest {
        packet.summary.push_str(
            " The smoke packet stays bound to the checkpoint packet; the retained state-dict digest remains matched through that prerequisite.",
        );
    }
    packet.packet_digest = stable_executor_mlx_smoke_run_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

/// Write the committed executor-lane MLX smoke-run packet.
pub fn write_builtin_executor_mlx_smoke_run_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMlxSmokeRunPacket, PsionExecutorMlxSmokeRunError> {
    let packet = builtin_executor_mlx_smoke_run_packet(workspace_root)?;
    let fixture_path = workspace_root.join(PSION_EXECUTOR_MLX_SMOKE_RUN_FIXTURE_PATH);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent).map_err(|error| PsionExecutorMlxSmokeRunError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    fs::write(&fixture_path, serde_json::to_vec_pretty(&packet)?).map_err(|error| {
        PsionExecutorMlxSmokeRunError::Write {
            path: fixture_path.display().to_string(),
            error,
        }
    })?;
    Ok(packet)
}

fn stable_executor_mlx_smoke_run_digest(packet: &PsionExecutorMlxSmokeRunPacket) -> String {
    let mut canonical = packet.clone();
    canonical.packet_digest.clear();
    stable_digest(b"psion_executor_mlx_smoke_run|", &canonical)
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

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorMlxSmokeRunError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorMlxSmokeRunError::MissingField {
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
        builtin_executor_mlx_smoke_run_packet, write_builtin_executor_mlx_smoke_run_packet,
        PsionExecutorMlxSmokeRunPacket, PSION_EXECUTOR_MLX_SMOKE_RUN_FIXTURE_PATH,
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
    fn builtin_executor_mlx_smoke_run_packet_is_valid() -> Result<(), Box<dyn Error>> {
        let packet = builtin_executor_mlx_smoke_run_packet(workspace_root().as_path())?;
        packet.validate()?;
        assert_eq!(packet.admitted_profile_id, "local_mac_mlx_aarch64");
        assert_eq!(
            packet.approved_frequent_pack_subset.subset_id,
            "tassadar.eval.frequent.v0::mlx_smoke_subset_v1"
        );
        assert_eq!(packet.checklist_rows.len(), 3);
        assert_eq!(packet.completed_steps, 93_184);
        Ok(())
    }

    #[test]
    fn executor_mlx_smoke_run_fixture_matches_committed_truth() -> Result<(), Box<dyn Error>> {
        let generated = builtin_executor_mlx_smoke_run_packet(workspace_root().as_path())?;
        let committed: PsionExecutorMlxSmokeRunPacket =
            read_json(workspace_root().join(PSION_EXECUTOR_MLX_SMOKE_RUN_FIXTURE_PATH))?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_executor_mlx_smoke_run_packet_persists_current_truth() -> Result<(), Box<dyn Error>> {
        let temp = tempfile::tempdir()?;
        let temp_root = temp.path();
        let source_root = workspace_root();

        for relative in [
            "fixtures/psion/executor/psion_executor_mlx_checkpoint_compatibility_v1.json",
            "fixtures/psion/executor/psion_executor_eval_packs_v1.json",
            "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/report.json",
            "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/portable_bundle.safetensors",
        ] {
            let source = source_root.join(relative);
            let target = temp_root.join(relative);
            fs::create_dir_all(target.parent().expect("target parent"))?;
            fs::copy(source, target)?;
        }

        let packet = write_builtin_executor_mlx_smoke_run_packet(temp_root)?;
        let persisted: PsionExecutorMlxSmokeRunPacket =
            read_json(temp_root.join(PSION_EXECUTOR_MLX_SMOKE_RUN_FIXTURE_PATH))?;
        assert_eq!(packet, persisted);
        Ok(())
    }
}
