use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_local_cluster_run_registration_packet,
    PsionExecutorLocalClusterRunRegistrationError, PsionExecutorLocalClusterRunRegistrationPacket,
    PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_SCHEMA_VERSION: &str =
    "psion.executor.phase_two_preflight_checklist.v1";
pub const PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_phase_two_preflight_checklist_v1.json";
pub const PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST.md";

const CHECKLIST_ID: &str = "psion_executor_phase_two_preflight_checklist_v1";
const BLOCKING_RULE: &str = "any_red_item_blocks_launch";
const MLX_RUN_TYPE_ID: &str = "mlx_decision_grade";
const CUDA_RUN_TYPE_ID: &str = "cuda_4080_decision_grade";
const LOCAL_MAC_MLX_PROFILE_ID: &str = "local_mac_mlx_aarch64";
const LOCAL_4080_PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const PSION_EXECUTOR_EVAL_PACKS_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const PSION_EXECUTOR_MLX_SMOKE_RUN_DOC_PATH: &str = "docs/PSION_EXECUTOR_MLX_SMOKE_RUN.md";
const PSION_EXECUTOR_4080_SMOKE_RUN_DOC_PATH: &str = "docs/PSION_EXECUTOR_4080_SMOKE_RUN.md";
const PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY.md";
const PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER.md";
const PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MAC_EXPORT_INSPECTION.md";
const PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT.md";

#[derive(Debug, Error)]
pub enum PsionExecutorPhaseTwoPreflightChecklistError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to parse `{path}`: {error}")]
    Parse {
        path: String,
        error: serde_json::Error,
    },
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("schema version mismatch: expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Registration(#[from] PsionExecutorLocalClusterRunRegistrationError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorPhaseTwoPreflightChecklistItem {
    pub item_id: String,
    pub category_id: String,
    pub blocking_color: String,
    pub applies_to_run_types: Vec<String>,
    pub required_evidence_refs: Vec<String>,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorPhaseTwoPreflightRunTypeRow {
    pub run_type_id: String,
    pub admitted_profile_id: String,
    pub current_registration_id: String,
    pub current_run_id: String,
    pub required_item_ids: Vec<String>,
    pub launch_blocked_if_any_red: bool,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorPhaseTwoPreflightChecklistPacket {
    pub schema_version: String,
    pub checklist_id: String,
    pub registration_packet_ref: String,
    pub registration_packet_digest: String,
    pub blocking_rule: String,
    pub checklist_items: Vec<PsionExecutorPhaseTwoPreflightChecklistItem>,
    pub run_type_rows: Vec<PsionExecutorPhaseTwoPreflightRunTypeRow>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorPhaseTwoPreflightChecklistItem {
    fn validate(&self) -> Result<(), PsionExecutorPhaseTwoPreflightChecklistError> {
        for (field, value) in [
            (
                "psion_executor_phase_two_preflight_checklist.checklist_items[].item_id",
                self.item_id.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.checklist_items[].category_id",
                self.category_id.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.checklist_items[].blocking_color",
                self.blocking_color.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.checklist_items[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.checklist_items[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.blocking_color != "red_blocks_launch" {
            return Err(PsionExecutorPhaseTwoPreflightChecklistError::InvalidValue {
                field: String::from(
                    "psion_executor_phase_two_preflight_checklist.checklist_items[].blocking_color",
                ),
                detail: String::from("every checklist item must stay a red launch blocker"),
            });
        }
        if self.applies_to_run_types.is_empty() || self.required_evidence_refs.is_empty() {
            return Err(PsionExecutorPhaseTwoPreflightChecklistError::MissingField {
                field: String::from(
                    "psion_executor_phase_two_preflight_checklist.checklist_items[].required_arrays",
                ),
            });
        }
        for run_type_id in &self.applies_to_run_types {
            ensure_nonempty(
                run_type_id.as_str(),
                "psion_executor_phase_two_preflight_checklist.checklist_items[].applies_to_run_types[]",
            )?;
        }
        for evidence_ref in &self.required_evidence_refs {
            ensure_nonempty(
                evidence_ref.as_str(),
                "psion_executor_phase_two_preflight_checklist.checklist_items[].required_evidence_refs[]",
            )?;
        }
        if stable_checklist_item_digest(self) != self.row_digest {
            return Err(PsionExecutorPhaseTwoPreflightChecklistError::DigestMismatch {
                field: String::from(
                    "psion_executor_phase_two_preflight_checklist.checklist_items[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorPhaseTwoPreflightRunTypeRow {
    fn validate(
        &self,
        checklist_item_ids: &[String],
    ) -> Result<(), PsionExecutorPhaseTwoPreflightChecklistError> {
        for (field, value) in [
            (
                "psion_executor_phase_two_preflight_checklist.run_type_rows[].run_type_id",
                self.run_type_id.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.run_type_rows[].admitted_profile_id",
                self.admitted_profile_id.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.run_type_rows[].current_registration_id",
                self.current_registration_id.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.run_type_rows[].current_run_id",
                self.current_run_id.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.run_type_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.run_type_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if !self.launch_blocked_if_any_red {
            return Err(PsionExecutorPhaseTwoPreflightChecklistError::InvalidValue {
                field: String::from(
                    "psion_executor_phase_two_preflight_checklist.run_type_rows[].launch_blocked_if_any_red",
                ),
                detail: String::from("phase-two run types must stay hard-blocked by red items"),
            });
        }
        if self.required_item_ids.is_empty() {
            return Err(PsionExecutorPhaseTwoPreflightChecklistError::MissingField {
                field: String::from(
                    "psion_executor_phase_two_preflight_checklist.run_type_rows[].required_item_ids",
                ),
            });
        }
        for item_id in &self.required_item_ids {
            ensure_nonempty(
                item_id.as_str(),
                "psion_executor_phase_two_preflight_checklist.run_type_rows[].required_item_ids[]",
            )?;
            if !checklist_item_ids.iter().any(|candidate| candidate == item_id) {
                return Err(PsionExecutorPhaseTwoPreflightChecklistError::InvalidValue {
                    field: String::from(
                        "psion_executor_phase_two_preflight_checklist.run_type_rows[].required_item_ids",
                    ),
                    detail: format!("unknown checklist item `{item_id}`"),
                });
            }
        }
        if stable_run_type_row_digest(self) != self.row_digest {
            return Err(PsionExecutorPhaseTwoPreflightChecklistError::DigestMismatch {
                field: String::from(
                    "psion_executor_phase_two_preflight_checklist.run_type_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorPhaseTwoPreflightChecklistPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorPhaseTwoPreflightChecklistError> {
        if self.schema_version != PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_SCHEMA_VERSION {
            return Err(PsionExecutorPhaseTwoPreflightChecklistError::SchemaVersionMismatch {
                expected: String::from(
                    PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_SCHEMA_VERSION,
                ),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_phase_two_preflight_checklist.checklist_id",
                self.checklist_id.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.registration_packet_ref",
                self.registration_packet_ref.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.registration_packet_digest",
                self.registration_packet_digest.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.blocking_rule",
                self.blocking_rule.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_phase_two_preflight_checklist.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.blocking_rule != BLOCKING_RULE {
            return Err(PsionExecutorPhaseTwoPreflightChecklistError::InvalidValue {
                field: String::from(
                    "psion_executor_phase_two_preflight_checklist.blocking_rule",
                ),
                detail: String::from("pre-flight blocking rule must stay explicit"),
            });
        }
        if self.checklist_items.len() != 7 || self.run_type_rows.len() != 2 {
            return Err(PsionExecutorPhaseTwoPreflightChecklistError::InvalidValue {
                field: String::from(
                    "psion_executor_phase_two_preflight_checklist.required_counts",
                ),
                detail: String::from(
                    "phase-two checklist must stay frozen to seven items and two admitted run types",
                ),
            });
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorPhaseTwoPreflightChecklistError::MissingField {
                field: String::from(
                    "psion_executor_phase_two_preflight_checklist.support_refs",
                ),
            });
        }
        let checklist_item_ids = self
            .checklist_items
            .iter()
            .map(|row| row.item_id.clone())
            .collect::<Vec<_>>();
        for item in &self.checklist_items {
            item.validate()?;
        }
        for run_type in &self.run_type_rows {
            run_type.validate(&checklist_item_ids)?;
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorPhaseTwoPreflightChecklistError::DigestMismatch {
                field: String::from(
                    "psion_executor_phase_two_preflight_checklist.packet_digest",
                ),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_phase_two_preflight_checklist_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorPhaseTwoPreflightChecklistPacket, PsionExecutorPhaseTwoPreflightChecklistError>
{
    let registration = builtin_executor_local_cluster_run_registration_packet(workspace_root)?;

    let checklist_items = vec![
        build_checklist_item(
            "device_health",
            "device_health",
            vec![MLX_RUN_TYPE_ID, CUDA_RUN_TYPE_ID],
            vec![PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE_DOC_PATH],
            "The admitted Mac and 4080 profiles must still be healthy and available before any phase-two launch starts.",
        ),
        build_checklist_item(
            "path_integrity",
            "paths",
            vec![MLX_RUN_TYPE_ID, CUDA_RUN_TYPE_ID],
            vec![
                PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE_DOC_PATH,
                PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH,
            ],
            "Model path, checkpoint path, log path, and export target path must be resolved before launch.",
        ),
        build_checklist_item(
            "dry_batch",
            "dry_batch",
            vec![MLX_RUN_TYPE_ID, CUDA_RUN_TYPE_ID],
            vec![
                PSION_EXECUTOR_MLX_SMOKE_RUN_DOC_PATH,
                PSION_EXECUTOR_4080_SMOKE_RUN_DOC_PATH,
            ],
            "Each admitted run class must have a retained dry-batch or smoke-run proof before decision-grade work starts.",
        ),
        build_checklist_item(
            "eval_attach",
            "eval_attach",
            vec![MLX_RUN_TYPE_ID, CUDA_RUN_TYPE_ID],
            vec![
                PSION_EXECUTOR_EVAL_PACKS_DOC_PATH,
                PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_DOC_PATH,
            ],
            "Certified eval packs and the checkpoint-time eval attach path must be explicit before launch.",
        ),
        build_checklist_item(
            "resume_rehearsal",
            "resume_rehearsal",
            vec![MLX_RUN_TYPE_ID, CUDA_RUN_TYPE_ID],
            vec![
                PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_DOC_PATH,
                PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH,
            ],
            "Resume and recovery posture must already be rehearsed on the admitted hardware path.",
        ),
        build_checklist_item(
            "ledger_fields",
            "ledger_fields",
            vec![MLX_RUN_TYPE_ID, CUDA_RUN_TYPE_ID],
            vec![
                PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH,
                PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH,
            ],
            "Run registration and ledger fields must be fully known before launch so long runs cannot hide missing facts.",
        ),
        build_checklist_item(
            "export_plan",
            "export_plan",
            vec![MLX_RUN_TYPE_ID, CUDA_RUN_TYPE_ID],
            vec![PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH],
            "Every phase-two run must name its export inspection and replacement-validation target before launch.",
        ),
    ];

    let required_item_ids = checklist_items
        .iter()
        .map(|row| row.item_id.clone())
        .collect::<Vec<_>>();
    let run_type_rows = vec![
        build_run_type_row(
            &registration,
            MLX_RUN_TYPE_ID,
            LOCAL_MAC_MLX_PROFILE_ID,
            &required_item_ids,
            "The admitted MLX decision-grade path is blocked if any pre-flight item remains red.",
        )?,
        build_run_type_row(
            &registration,
            CUDA_RUN_TYPE_ID,
            LOCAL_4080_PROFILE_ID,
            &required_item_ids,
            "The admitted 4080 decision-grade path is blocked if any pre-flight item remains red.",
        )?,
    ];

    let mut packet = PsionExecutorPhaseTwoPreflightChecklistPacket {
        schema_version: String::from(PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_SCHEMA_VERSION),
        checklist_id: String::from(CHECKLIST_ID),
        registration_packet_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH),
        registration_packet_digest: registration.packet_digest,
        blocking_rule: String::from(BLOCKING_RULE),
        checklist_items,
        run_type_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACKS_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH),
            String::from(PSION_EXECUTOR_MLX_SMOKE_RUN_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_SMOKE_RUN_DOC_PATH),
            String::from(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH),
            String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_DOC_PATH),
        ],
        summary: String::from(
            "The admitted executor lane now has one phase-two pre-flight checklist packet. Device health, paths, dry-batch proof, eval attach, resume rehearsal, ledger fields, and export plan are frozen as red launch blockers for the admitted MLX and 4080 decision-grade run types.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_phase_two_preflight_checklist_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorPhaseTwoPreflightChecklistPacket, PsionExecutorPhaseTwoPreflightChecklistError>
{
    let packet = builtin_executor_phase_two_preflight_checklist_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_checklist_item(
    item_id: &str,
    category_id: &str,
    applies_to_run_types: Vec<&str>,
    required_evidence_refs: Vec<&str>,
    detail: &str,
) -> PsionExecutorPhaseTwoPreflightChecklistItem {
    let mut row = PsionExecutorPhaseTwoPreflightChecklistItem {
        item_id: String::from(item_id),
        category_id: String::from(category_id),
        blocking_color: String::from("red_blocks_launch"),
        applies_to_run_types: applies_to_run_types
            .into_iter()
            .map(String::from)
            .collect(),
        required_evidence_refs: required_evidence_refs
            .into_iter()
            .map(String::from)
            .collect(),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_checklist_item_digest(&row);
    row
}

fn build_run_type_row(
    registration: &PsionExecutorLocalClusterRunRegistrationPacket,
    run_type_id: &str,
    admitted_profile_id: &str,
    required_item_ids: &[String],
    detail: &str,
) -> Result<PsionExecutorPhaseTwoPreflightRunTypeRow, PsionExecutorPhaseTwoPreflightChecklistError>
{
    let registration_row = registration
        .registration_rows
        .iter()
        .find(|row| row.run_type_id == run_type_id && row.admitted_profile_id == admitted_profile_id)
        .ok_or_else(|| PsionExecutorPhaseTwoPreflightChecklistError::MissingField {
            field: format!(
                "psion_executor_phase_two_preflight_checklist.registration_row.{run_type_id}"
            ),
        })?;
    let mut row = PsionExecutorPhaseTwoPreflightRunTypeRow {
        run_type_id: String::from(run_type_id),
        admitted_profile_id: String::from(admitted_profile_id),
        current_registration_id: registration_row.registration_id.clone(),
        current_run_id: registration_row.run_id.clone(),
        required_item_ids: required_item_ids.to_vec(),
        launch_blocked_if_any_red: true,
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_run_type_row_digest(&row);
    Ok(row)
}

fn stable_checklist_item_digest(row: &PsionExecutorPhaseTwoPreflightChecklistItem) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    digest_json(&clone)
}

fn stable_run_type_row_digest(row: &PsionExecutorPhaseTwoPreflightRunTypeRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    digest_json(&clone)
}

fn stable_packet_digest(packet: &PsionExecutorPhaseTwoPreflightChecklistPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    digest_json(&clone)
}

fn digest_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize digest");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorPhaseTwoPreflightChecklistError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorPhaseTwoPreflightChecklistError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    fixture_path: &str,
    value: &T,
) -> Result<(), PsionExecutorPhaseTwoPreflightChecklistError> {
    let path = workspace_root.join(fixture_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorPhaseTwoPreflightChecklistError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let payload = serde_json::to_string_pretty(value)?;
    fs::write(&path, payload).map_err(|error| PsionExecutorPhaseTwoPreflightChecklistError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn read_json_fixture<T: DeserializeOwned>(
    workspace_root: &Path,
    fixture_path: &str,
) -> Result<T, PsionExecutorPhaseTwoPreflightChecklistError> {
    let path = workspace_root.join(fixture_path);
    let payload = fs::read_to_string(&path).map_err(|error| {
        PsionExecutorPhaseTwoPreflightChecklistError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_str(&payload).map_err(|error| PsionExecutorPhaseTwoPreflightChecklistError::Parse {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
    }

    #[test]
    fn builtin_preflight_packet_is_valid() {
        let packet = builtin_executor_phase_two_preflight_checklist_packet(workspace_root())
            .expect("build preflight packet");
        packet.validate().expect("packet validates");
    }

    #[test]
    fn preflight_fixture_matches_committed_truth() {
        let expected = builtin_executor_phase_two_preflight_checklist_packet(workspace_root())
            .expect("build expected packet");
        let fixture: PsionExecutorPhaseTwoPreflightChecklistPacket = read_json_fixture(
            workspace_root(),
            PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST_FIXTURE_PATH,
        )
        .expect("read committed fixture");
        assert_eq!(fixture, expected);
    }

    #[test]
    fn preflight_rule_stays_red_blocking_for_both_run_types() {
        let packet = builtin_executor_phase_two_preflight_checklist_packet(workspace_root())
            .expect("build preflight packet");
        assert_eq!(packet.blocking_rule, BLOCKING_RULE);
        assert_eq!(packet.run_type_rows.len(), 2);
        assert!(packet.run_type_rows.iter().all(|row| row.launch_blocked_if_any_red));
        assert!(packet
            .checklist_items
            .iter()
            .all(|row| row.blocking_color == "red_blocks_launch"));
    }

    #[test]
    fn preflight_packet_covers_admitted_mlx_and_4080_profiles() {
        let packet = builtin_executor_phase_two_preflight_checklist_packet(workspace_root())
            .expect("build preflight packet");
        let profile_ids = packet
            .run_type_rows
            .iter()
            .map(|row| row.admitted_profile_id.as_str())
            .collect::<Vec<_>>();
        assert!(profile_ids.contains(&LOCAL_MAC_MLX_PROFILE_ID));
        assert!(profile_ids.contains(&LOCAL_4080_PROFILE_ID));
    }
}
