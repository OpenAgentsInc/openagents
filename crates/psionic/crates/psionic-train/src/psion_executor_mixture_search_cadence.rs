use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_canonical_mixture_packet,
    builtin_executor_local_cluster_review_workflow_packet,
    builtin_executor_local_cluster_run_registration_packet,
    PsionExecutorCanonicalMixtureError, PsionExecutorLocalClusterReviewWorkflowError,
    PsionExecutorLocalClusterRunRegistrationError, PsionExecutorLocalClusterCandidateStatus,
    PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH, PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE_SCHEMA_VERSION: &str =
    "psion.executor.mixture_search_cadence.v1";
pub const PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_mixture_search_cadence_v1.json";
pub const PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE.md";

const CADENCE_ID: &str = "psion_executor_weekly_mixture_search_cadence_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const MAX_NEW_MIXTURE_VERSIONS_PER_REVIEW: u32 = 1;
const MAX_CONCURRENT_REGISTERED_RUNS_BEFORE_LANE_HEALTH: u32 = 2;

#[derive(Debug, Error)]
pub enum PsionExecutorMixtureSearchCadenceError {
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
    Mixture(#[from] PsionExecutorCanonicalMixtureError),
    #[error(transparent)]
    Registration(#[from] PsionExecutorLocalClusterRunRegistrationError),
    #[error(transparent)]
    ReviewWorkflow(#[from] PsionExecutorLocalClusterReviewWorkflowError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMixtureSearchCadenceRow {
    pub registration_id: String,
    pub run_id: String,
    pub admitted_profile_id: String,
    pub candidate_status: PsionExecutorLocalClusterCandidateStatus,
    pub mixture_version_id: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMixtureSearchCadencePacket {
    pub schema_version: String,
    pub cadence_id: String,
    pub review_workflow_ref: String,
    pub review_workflow_digest: String,
    pub registration_packet_ref: String,
    pub registration_packet_digest: String,
    pub canonical_mixture_ref: String,
    pub canonical_mixture_digest: String,
    pub current_review_window_id: String,
    pub active_mixture_version_id: String,
    pub max_new_mixture_versions_per_review: u32,
    pub max_concurrent_registered_runs_before_lane_health: u32,
    pub current_lane_health_status: String,
    pub current_lane_health_block_ids: Vec<String>,
    pub registered_rows: Vec<PsionExecutorMixtureSearchCadenceRow>,
    pub versioning_rule: String,
    pub concurrency_rule: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorMixtureSearchCadenceRow {
    fn validate(&self) -> Result<(), PsionExecutorMixtureSearchCadenceError> {
        for (field, value) in [
            (
                "psion_executor_mixture_search_cadence.registered_rows[].registration_id",
                self.registration_id.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.registered_rows[].run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.registered_rows[].admitted_profile_id",
                self.admitted_profile_id.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.registered_rows[].mixture_version_id",
                self.mixture_version_id.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.registered_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if stable_registered_row_digest(self) != self.row_digest {
            return Err(PsionExecutorMixtureSearchCadenceError::DigestMismatch {
                field: String::from("psion_executor_mixture_search_cadence.registered_rows[].row_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorMixtureSearchCadencePacket {
    pub fn validate(&self) -> Result<(), PsionExecutorMixtureSearchCadenceError> {
        if self.schema_version != PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE_SCHEMA_VERSION {
            return Err(PsionExecutorMixtureSearchCadenceError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            ("psion_executor_mixture_search_cadence.cadence_id", self.cadence_id.as_str()),
            (
                "psion_executor_mixture_search_cadence.review_workflow_ref",
                self.review_workflow_ref.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.review_workflow_digest",
                self.review_workflow_digest.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.registration_packet_ref",
                self.registration_packet_ref.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.registration_packet_digest",
                self.registration_packet_digest.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.canonical_mixture_ref",
                self.canonical_mixture_ref.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.canonical_mixture_digest",
                self.canonical_mixture_digest.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.current_review_window_id",
                self.current_review_window_id.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.active_mixture_version_id",
                self.active_mixture_version_id.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.current_lane_health_status",
                self.current_lane_health_status.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.versioning_rule",
                self.versioning_rule.as_str(),
            ),
            (
                "psion_executor_mixture_search_cadence.concurrency_rule",
                self.concurrency_rule.as_str(),
            ),
            ("psion_executor_mixture_search_cadence.summary", self.summary.as_str()),
            (
                "psion_executor_mixture_search_cadence.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.max_new_mixture_versions_per_review != MAX_NEW_MIXTURE_VERSIONS_PER_REVIEW {
            return Err(PsionExecutorMixtureSearchCadenceError::InvalidValue {
                field: String::from(
                    "psion_executor_mixture_search_cadence.max_new_mixture_versions_per_review",
                ),
                detail: String::from("executor mixture cadence stays limited to one new version per weekly review"),
            });
        }
        if self.max_concurrent_registered_runs_before_lane_health
            != MAX_CONCURRENT_REGISTERED_RUNS_BEFORE_LANE_HEALTH
        {
            return Err(PsionExecutorMixtureSearchCadenceError::InvalidValue {
                field: String::from(
                    "psion_executor_mixture_search_cadence.max_concurrent_registered_runs_before_lane_health",
                ),
                detail: String::from("executor mixture cadence stays limited to two concurrent registered runs before lane health clears"),
            });
        }
        if self.registered_rows.is_empty() || self.support_refs.is_empty() {
            return Err(PsionExecutorMixtureSearchCadenceError::MissingField {
                field: String::from("psion_executor_mixture_search_cadence.required_arrays"),
            });
        }
        if self.registered_rows.len() as u32 > self.max_concurrent_registered_runs_before_lane_health {
            return Err(PsionExecutorMixtureSearchCadenceError::InvalidValue {
                field: String::from("psion_executor_mixture_search_cadence.registered_rows"),
                detail: format!(
                    "registered run count {} exceeded the current pre-lane-health ceiling {}",
                    self.registered_rows.len(),
                    self.max_concurrent_registered_runs_before_lane_health
                ),
            });
        }
        for row in &self.registered_rows {
            row.validate()?;
            if row.mixture_version_id != self.active_mixture_version_id {
                return Err(PsionExecutorMixtureSearchCadenceError::InvalidValue {
                    field: String::from("psion_executor_mixture_search_cadence.registered_rows[].mixture_version_id"),
                    detail: format!(
                        "row `{}` drifted from active mixture version `{}`",
                        row.registration_id, self.active_mixture_version_id
                    ),
                });
            }
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorMixtureSearchCadenceError::DigestMismatch {
                field: String::from("psion_executor_mixture_search_cadence.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn build_executor_mixture_search_cadence_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMixtureSearchCadencePacket, PsionExecutorMixtureSearchCadenceError> {
    let canonical_mixture = builtin_executor_canonical_mixture_packet(workspace_root)?;
    let review_workflow = builtin_executor_local_cluster_review_workflow_packet(workspace_root)?;
    let registration = builtin_executor_local_cluster_run_registration_packet(workspace_root)?;
    let registered_rows = registration
        .registration_rows
        .iter()
        .map(|row| {
            let mut cadence_row = PsionExecutorMixtureSearchCadenceRow {
                registration_id: row.registration_id.clone(),
                run_id: row.run_id.clone(),
                admitted_profile_id: row.admitted_profile_id.clone(),
                candidate_status: row.candidate_status.clone(),
                mixture_version_id: row.mixture_version_id.clone(),
                row_digest: String::new(),
            };
            cadence_row.row_digest = stable_registered_row_digest(&cadence_row);
            cadence_row
        })
        .collect::<Vec<_>>();
    let current_lane_health_status = review_workflow
        .current_decisions
        .iter()
        .find(|decision| decision.review_kind == "baseline_review")
        .map(|decision| decision.status.clone())
        .unwrap_or_else(|| String::from("review_unknown"));
    let current_lane_health_block_ids = review_workflow
        .current_decisions
        .iter()
        .find(|decision| decision.review_kind == "baseline_review")
        .map(|decision| decision.cited_block_ids.clone())
        .unwrap_or_default();

    let mut packet = PsionExecutorMixtureSearchCadencePacket {
        schema_version: String::from(PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE_SCHEMA_VERSION),
        cadence_id: String::from(CADENCE_ID),
        review_workflow_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH),
        review_workflow_digest: review_workflow.workflow_digest,
        registration_packet_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH),
        registration_packet_digest: registration.packet_digest,
        canonical_mixture_ref: String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH),
        canonical_mixture_digest: canonical_mixture.packet_digest,
        current_review_window_id: String::from("2026-W14"),
        active_mixture_version_id: canonical_mixture.mixture_id,
        max_new_mixture_versions_per_review: MAX_NEW_MIXTURE_VERSIONS_PER_REVIEW,
        max_concurrent_registered_runs_before_lane_health: MAX_CONCURRENT_REGISTERED_RUNS_BEFORE_LANE_HEALTH,
        current_lane_health_status,
        current_lane_health_block_ids,
        registered_rows,
        versioning_rule: String::from(
            "At most one new executor mixture version may be admitted in a weekly review window. Any new version must be named and reviewed against the canonical frozen-pack workflow before another version is opened.",
        ),
        concurrency_rule: String::from(
            "Before lane health clears, the executor lane may keep at most two concurrently registered runs on the active mixture version. Additional parallel experiments are blocked until the baseline weekly review no longer reports active promotion blocks.",
        ),
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH),
        ],
        summary: String::from(
            "The executor lane now has one canonical weekly mixture-search cadence packet. Registration rows carry the active mixture version, only one new mixture version may open per weekly review, and the pre-lane-health concurrency ceiling is explicitly limited to two registered runs on the active mixture.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_executor_mixture_search_cadence_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMixtureSearchCadencePacket, PsionExecutorMixtureSearchCadenceError> {
    let packet = build_executor_mixture_search_cadence_packet(workspace_root)?;
    write_json_fixture(workspace_root, PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE_FIXTURE_PATH, &packet)?;
    Ok(packet)
}

fn stable_registered_row_digest(row: &PsionExecutorMixtureSearchCadenceRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("cadence row serialization should succeed"),
    ))
}

fn stable_packet_digest(packet: &PsionExecutorMixtureSearchCadencePacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("cadence packet serialization should succeed"),
    ))
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorMixtureSearchCadenceError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorMixtureSearchCadenceError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorMixtureSearchCadenceError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorMixtureSearchCadenceError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorMixtureSearchCadenceError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorMixtureSearchCadenceError::Parse {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorMixtureSearchCadenceError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorMixtureSearchCadenceError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
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
    fn built_executor_mixture_search_cadence_packet_is_valid(
    ) -> Result<(), PsionExecutorMixtureSearchCadenceError> {
        let root = workspace_root();
        let packet = build_executor_mixture_search_cadence_packet(root.as_path())?;
        packet.validate()?;
        assert_eq!(packet.registered_rows.len(), 2);
        assert_eq!(packet.max_new_mixture_versions_per_review, 1);
        assert_eq!(packet.max_concurrent_registered_runs_before_lane_health, 2);
        Ok(())
    }

    #[test]
    fn executor_mixture_search_cadence_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorMixtureSearchCadenceError> {
        let root = workspace_root();
        let expected: PsionExecutorMixtureSearchCadencePacket =
            read_json(root.as_path(), PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE_FIXTURE_PATH)?;
        let actual = build_executor_mixture_search_cadence_packet(root.as_path())?;
        if expected != actual {
            return Err(PsionExecutorMixtureSearchCadenceError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn concurrency_ceiling_stays_enforced(
    ) -> Result<(), PsionExecutorMixtureSearchCadenceError> {
        let root = workspace_root();
        let mut packet = build_executor_mixture_search_cadence_packet(root.as_path())?;
        let extra = packet.registered_rows[0].clone();
        packet.registered_rows.push(extra);
        let error = packet.validate().expect_err("too many rows should fail");
        assert!(matches!(
            error,
            PsionExecutorMixtureSearchCadenceError::InvalidValue { field, .. }
            if field == "psion_executor_mixture_search_cadence.registered_rows"
        ));
        Ok(())
    }
}
