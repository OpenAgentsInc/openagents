use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_local_cluster_review_workflow_packet,
    PsionExecutorLocalClusterReviewWorkflowError,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_CONTINUE_RESTART_POLICY_SCHEMA_VERSION: &str =
    "psion.executor.continue_restart_policy.v1";
pub const PSION_EXECUTOR_CONTINUE_RESTART_POLICY_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_continue_restart_policy_v1.json";
pub const PSION_EXECUTOR_CONTINUE_RESTART_POLICY_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_CONTINUE_RESTART_POLICY.md";

const POLICY_ID: &str = "psion_executor_continue_restart_policy_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_OWNERSHIP_DOC_PATH: &str = "docs/PSION_EXECUTOR_OWNERSHIP.md";
const PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_CHECKLIST.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP.md";
const PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MAC_EXPORT_INSPECTION.md";

#[derive(Debug, Error)]
pub enum PsionExecutorContinueRestartPolicyError {
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
    ReviewWorkflow(#[from] PsionExecutorLocalClusterReviewWorkflowError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorContinueRestartIssueClassRow {
    pub incident_class_id: String,
    pub default_action: String,
    pub owner_role: String,
    pub required_evidence_refs: Vec<String>,
    pub review_requirement: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorContinueRestartPolicyPacket {
    pub schema_version: String,
    pub policy_id: String,
    pub review_workflow_ref: String,
    pub review_workflow_digest: String,
    pub ownership_ref: String,
    pub incident_rows: Vec<PsionExecutorContinueRestartIssueClassRow>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorContinueRestartIssueClassRow {
    fn validate(&self) -> Result<(), PsionExecutorContinueRestartPolicyError> {
        for (field, value) in [
            (
                "psion_executor_continue_restart_policy.incident_rows[].incident_class_id",
                self.incident_class_id.as_str(),
            ),
            (
                "psion_executor_continue_restart_policy.incident_rows[].default_action",
                self.default_action.as_str(),
            ),
            (
                "psion_executor_continue_restart_policy.incident_rows[].owner_role",
                self.owner_role.as_str(),
            ),
            (
                "psion_executor_continue_restart_policy.incident_rows[].review_requirement",
                self.review_requirement.as_str(),
            ),
            (
                "psion_executor_continue_restart_policy.incident_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_continue_restart_policy.incident_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.required_evidence_refs.is_empty() {
            return Err(PsionExecutorContinueRestartPolicyError::MissingField {
                field: String::from(
                    "psion_executor_continue_restart_policy.incident_rows[].required_evidence_refs",
                ),
            });
        }
        for reference in &self.required_evidence_refs {
            ensure_nonempty(
                reference.as_str(),
                "psion_executor_continue_restart_policy.incident_rows[].required_evidence_refs[]",
            )?;
        }
        if stable_incident_row_digest(self) != self.row_digest {
            return Err(PsionExecutorContinueRestartPolicyError::DigestMismatch {
                field: String::from(
                    "psion_executor_continue_restart_policy.incident_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorContinueRestartPolicyPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorContinueRestartPolicyError> {
        if self.schema_version != PSION_EXECUTOR_CONTINUE_RESTART_POLICY_SCHEMA_VERSION {
            return Err(PsionExecutorContinueRestartPolicyError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_CONTINUE_RESTART_POLICY_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_continue_restart_policy.policy_id",
                self.policy_id.as_str(),
            ),
            (
                "psion_executor_continue_restart_policy.review_workflow_ref",
                self.review_workflow_ref.as_str(),
            ),
            (
                "psion_executor_continue_restart_policy.review_workflow_digest",
                self.review_workflow_digest.as_str(),
            ),
            (
                "psion_executor_continue_restart_policy.ownership_ref",
                self.ownership_ref.as_str(),
            ),
            (
                "psion_executor_continue_restart_policy.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_continue_restart_policy.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.incident_rows.len() != 6 || self.support_refs.is_empty() {
            return Err(PsionExecutorContinueRestartPolicyError::InvalidValue {
                field: String::from(
                    "psion_executor_continue_restart_policy.required_counts",
                ),
                detail: String::from(
                    "incident policy must stay frozen to six incident classes plus support refs",
                ),
            });
        }
        for row in &self.incident_rows {
            row.validate()?;
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorContinueRestartPolicyError::DigestMismatch {
                field: String::from("psion_executor_continue_restart_policy.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_continue_restart_policy_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorContinueRestartPolicyPacket, PsionExecutorContinueRestartPolicyError> {
    let review_workflow = builtin_executor_local_cluster_review_workflow_packet(workspace_root)?;
    let incident_rows = vec![
        build_incident_row(
            "launch_drift",
            "restart_after_preflight_recheck",
            "executor_lane_operator",
            vec![
                PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_DOC_PATH,
                PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
            ],
            "must_be_logged_in_weekly_review",
            "Launch drift means the run no longer matches admitted pre-flight truth, so the run must restart only after the pre-flight checklist returns green.",
        ),
        build_incident_row(
            "transient_interruption",
            "continue_from_last_green_checkpoint",
            "executor_lane_operator",
            vec![
                PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH,
                PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
            ],
            "must_be_logged_in_weekly_review",
            "Transient interruption keeps the run if the last green checkpoint is intact and the recovery path remains within policy.",
        ),
        build_incident_row(
            "missing_facts",
            "hold_until_facts_repaired",
            "review_cadence_owner",
            vec![
                PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_DOC_PATH,
                PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
            ],
            "must_be_logged_in_weekly_review",
            "Missing facts do not permit a continue decision; the lane holds until registration, ledger, eval, or export facts are repaired.",
        ),
        build_incident_row(
            "throughput_degradation",
            "continue_under_review_no_promotion",
            "review_cadence_owner",
            vec![
                PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_DOC_PATH,
                PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
            ],
            "must_be_logged_in_weekly_review",
            "Throughput degradation may continue only as a non-promotable run while weekly review tracks the degradation explicitly.",
        ),
        build_incident_row(
            "non_finite_loss",
            "restart_from_last_green_checkpoint",
            "executor_lane_operator",
            vec![
                PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_DOC_PATH,
                PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
            ],
            "must_be_logged_in_weekly_review",
            "Non-finite loss invalidates the live training state and forces a restart from the last green checkpoint with the incident preserved for review.",
        ),
        build_incident_row(
            "export_failure",
            "stop_and_repair_before_review",
            "export_validation_owner",
            vec![
                PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH,
                PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
            ],
            "must_be_logged_in_weekly_review",
            "Export failure blocks closeout and must be repaired before the weekly review can count the run as complete.",
        ),
    ];

    let mut packet = PsionExecutorContinueRestartPolicyPacket {
        schema_version: String::from(PSION_EXECUTOR_CONTINUE_RESTART_POLICY_SCHEMA_VERSION),
        policy_id: String::from(POLICY_ID),
        review_workflow_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH),
        review_workflow_digest: review_workflow.workflow_digest,
        ownership_ref: String::from(PSION_EXECUTOR_OWNERSHIP_DOC_PATH),
        incident_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_OWNERSHIP_DOC_PATH),
            String::from(PSION_EXECUTOR_PHASE_TWO_PREFLIGHT_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH),
            String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH),
        ],
        summary: String::from(
            "The admitted executor lane now has one continue-vs-restart incident policy packet. Launch drift, transient interruption, missing facts, throughput degradation, non-finite loss, and export failure now carry explicit default action, owner role, required evidence, and weekly-review logging requirements.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_continue_restart_policy_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorContinueRestartPolicyPacket, PsionExecutorContinueRestartPolicyError> {
    let packet = builtin_executor_continue_restart_policy_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_CONTINUE_RESTART_POLICY_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_incident_row(
    incident_class_id: &str,
    default_action: &str,
    owner_role: &str,
    required_evidence_refs: Vec<&str>,
    review_requirement: &str,
    detail: &str,
) -> PsionExecutorContinueRestartIssueClassRow {
    let mut row = PsionExecutorContinueRestartIssueClassRow {
        incident_class_id: String::from(incident_class_id),
        default_action: String::from(default_action),
        owner_role: String::from(owner_role),
        required_evidence_refs: required_evidence_refs
            .into_iter()
            .map(String::from)
            .collect(),
        review_requirement: String::from(review_requirement),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_incident_row_digest(&row);
    row
}

fn stable_incident_row_digest(row: &PsionExecutorContinueRestartIssueClassRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    digest_json(&clone)
}

fn stable_packet_digest(packet: &PsionExecutorContinueRestartPolicyPacket) -> String {
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
) -> Result<(), PsionExecutorContinueRestartPolicyError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorContinueRestartPolicyError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    fixture_path: &str,
    value: &T,
) -> Result<(), PsionExecutorContinueRestartPolicyError> {
    let path = workspace_root.join(fixture_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorContinueRestartPolicyError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let payload = serde_json::to_string_pretty(value)?;
    fs::write(&path, payload).map_err(|error| PsionExecutorContinueRestartPolicyError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn read_json_fixture<T: DeserializeOwned>(
    workspace_root: &Path,
    fixture_path: &str,
) -> Result<T, PsionExecutorContinueRestartPolicyError> {
    let path = workspace_root.join(fixture_path);
    let payload = fs::read_to_string(&path).map_err(|error| {
        PsionExecutorContinueRestartPolicyError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_str(&payload).map_err(|error| PsionExecutorContinueRestartPolicyError::Parse {
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
    fn builtin_continue_restart_policy_is_valid() {
        let packet = builtin_executor_continue_restart_policy_packet(workspace_root())
            .expect("build policy packet");
        packet.validate().expect("packet validates");
    }

    #[test]
    fn continue_restart_policy_fixture_matches_committed_truth() {
        let expected = builtin_executor_continue_restart_policy_packet(workspace_root())
            .expect("build expected policy packet");
        let fixture: PsionExecutorContinueRestartPolicyPacket = read_json_fixture(
            workspace_root(),
            PSION_EXECUTOR_CONTINUE_RESTART_POLICY_FIXTURE_PATH,
        )
        .expect("read committed fixture");
        assert_eq!(fixture, expected);
    }

    #[test]
    fn incident_policy_keeps_six_canonical_issue_classes() {
        let packet = builtin_executor_continue_restart_policy_packet(workspace_root())
            .expect("build policy packet");
        assert_eq!(packet.incident_rows.len(), 6);
        let ids = packet
            .incident_rows
            .iter()
            .map(|row| row.incident_class_id.as_str())
            .collect::<Vec<_>>();
        for expected in [
            "launch_drift",
            "transient_interruption",
            "missing_facts",
            "throughput_degradation",
            "non_finite_loss",
            "export_failure",
        ] {
            assert!(ids.contains(&expected));
        }
    }

    #[test]
    fn incident_policy_stays_bound_to_weekly_review() {
        let packet = builtin_executor_continue_restart_policy_packet(workspace_root())
            .expect("build policy packet");
        assert_eq!(
            packet.review_workflow_ref,
            PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH
        );
        assert!(packet
            .incident_rows
            .iter()
            .all(|row| row.review_requirement == "must_be_logged_in_weekly_review"));
    }
}
