use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    load_psionic_train_grouped_stage_transport, PsionicTrainGroupedReplicaStageAssignment,
    PsionicTrainGroupedReplicaStageTransportArtifacts, PsionicTrainInvocationManifest,
    PsionicTrainOutcomeKind, TrainingExecutionValidatorDisposition,
};

pub const PSIONIC_TRAIN_GROUPED_STAGE_EXECUTION_SUMMARY_SCHEMA_VERSION: &str =
    "psionic.train.grouped_replica_stage_execution_summary.v1";
pub const PSIONIC_TRAIN_GROUPED_STAGE_REPLAY_EVIDENCE_SCHEMA_VERSION: &str =
    "psionic.train.grouped_replica_stage_replay_evidence.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainGroupedReplicaStageExecutionSummary {
    pub schema_version: String,
    pub lane_id: String,
    pub network_id: Option<String>,
    pub run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub contribution_id: String,
    pub node_pubkey: String,
    pub grouped_stage_assignment: PsionicTrainGroupedReplicaStageAssignment,
    pub outcome: PsionicTrainOutcomeKind,
    pub input_transport_path: Option<String>,
    pub input_transport_digest: Option<String>,
    pub input_payload_digest: Option<String>,
    pub output_transport_path: Option<String>,
    pub output_transport_digest: Option<String>,
    pub output_payload_path: Option<String>,
    pub output_payload_digest: Option<String>,
    pub detail: String,
    pub execution_summary_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsionicTrainGroupedReplicaStageExecutionSummaryArtifacts {
    pub grouped_stage_execution_summary_path: String,
    pub grouped_stage_execution_summary_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainGroupedReplicaStageReplayEvidence {
    pub schema_version: String,
    pub lane_id: String,
    pub network_id: Option<String>,
    pub validator_run_id: String,
    pub challenged_run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub challenge_id: String,
    pub validator_node_pubkey: String,
    pub challenged_node_pubkey: String,
    pub contribution_id: String,
    pub contribution_digest: String,
    pub artifact_manifest_digest: String,
    pub grouped_stage_assignment: PsionicTrainGroupedReplicaStageAssignment,
    pub execution_summary_path: String,
    pub execution_summary_digest: String,
    pub input_transport_digest: Option<String>,
    pub output_transport_digest: Option<String>,
    pub disposition: TrainingExecutionValidatorDisposition,
    pub reason_codes: Vec<String>,
    pub score_bps: u16,
    pub detail: String,
    pub replay_evidence_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsionicTrainGroupedReplicaStageReplayEvidenceArtifacts {
    pub grouped_stage_replay_evidence_path: String,
    pub grouped_stage_replay_evidence_digest: String,
}

#[derive(Debug, Error)]
pub enum PsionicTrainGroupedReplicaEvidenceError {
    #[error("failed to read `{path}`: {detail}")]
    Read { path: String, detail: String },
    #[error("failed to write `{path}`: {detail}")]
    Write { path: String, detail: String },
    #[error("failed to parse `{path}`: {detail}")]
    Parse { path: String, detail: String },
    #[error("grouped-replica stage evidence is invalid: {detail}")]
    Invalid { detail: String },
    #[error("grouped-replica stage evidence drifted: {detail}")]
    ArtifactDigestMismatch { detail: String },
    #[error("grouped-replica stage evidence is stale: {detail}")]
    StaleAssignment { detail: String },
}

impl PsionicTrainGroupedReplicaStageExecutionSummary {
    #[must_use]
    pub fn stable_execution_summary_digest(&self) -> String {
        let mut digest_basis = self.clone();
        digest_basis.execution_summary_digest.clear();
        stable_digest(
            b"psionic_train_grouped_stage_execution_summary|",
            &digest_basis,
        )
    }

    pub fn validate(&self) -> Result<(), PsionicTrainGroupedReplicaEvidenceError> {
        if self.schema_version != PSIONIC_TRAIN_GROUPED_STAGE_EXECUTION_SUMMARY_SCHEMA_VERSION {
            return Err(PsionicTrainGroupedReplicaEvidenceError::Invalid {
                detail: format!(
                    "grouped stage execution summary schema version must stay `{}` but was `{}`",
                    PSIONIC_TRAIN_GROUPED_STAGE_EXECUTION_SUMMARY_SCHEMA_VERSION,
                    self.schema_version
                ),
            });
        }
        require_nonempty(self.lane_id.as_str(), "execution summary lane_id")?;
        require_nonempty(self.run_id.as_str(), "execution summary run_id")?;
        require_nonempty(self.window_id.as_str(), "execution summary window_id")?;
        require_nonempty(
            self.assignment_id.as_str(),
            "execution summary assignment_id",
        )?;
        require_nonempty(
            self.contribution_id.as_str(),
            "execution summary contribution_id",
        )?;
        require_nonempty(self.node_pubkey.as_str(), "execution summary node_pubkey")?;
        require_nonempty(self.detail.as_str(), "execution summary detail")?;
        require_transport_pair(
            self.input_transport_path.as_deref(),
            self.input_transport_digest.as_deref(),
            self.input_payload_digest.as_deref(),
            "input",
        )?;
        require_transport_pair(
            self.output_transport_path.as_deref(),
            self.output_transport_digest.as_deref(),
            self.output_payload_digest.as_deref(),
            "output",
        )?;
        if self.grouped_stage_assignment.upstream_stage_id.is_some()
            != self.input_transport_path.is_some()
        {
            return Err(PsionicTrainGroupedReplicaEvidenceError::Invalid {
                detail: String::from(
                    "grouped stage execution summary input transport posture drifted from the stage assignment",
                ),
            });
        }
        if self.grouped_stage_assignment.downstream_stage_id.is_some()
            != self.output_transport_path.is_some()
        {
            return Err(PsionicTrainGroupedReplicaEvidenceError::Invalid {
                detail: String::from(
                    "grouped stage execution summary output transport posture drifted from the stage assignment",
                ),
            });
        }
        if self.execution_summary_digest != self.stable_execution_summary_digest() {
            return Err(
                PsionicTrainGroupedReplicaEvidenceError::ArtifactDigestMismatch {
                    detail: String::from("grouped stage execution summary digest drifted"),
                },
            );
        }
        Ok(())
    }
}

impl PsionicTrainGroupedReplicaStageReplayEvidence {
    #[must_use]
    pub fn stable_replay_evidence_digest(&self) -> String {
        let mut digest_basis = self.clone();
        digest_basis.replay_evidence_digest.clear();
        stable_digest(
            b"psionic_train_grouped_stage_replay_evidence|",
            &digest_basis,
        )
    }

    pub fn validate(&self) -> Result<(), PsionicTrainGroupedReplicaEvidenceError> {
        if self.schema_version != PSIONIC_TRAIN_GROUPED_STAGE_REPLAY_EVIDENCE_SCHEMA_VERSION {
            return Err(PsionicTrainGroupedReplicaEvidenceError::Invalid {
                detail: format!(
                    "grouped stage replay evidence schema version must stay `{}` but was `{}`",
                    PSIONIC_TRAIN_GROUPED_STAGE_REPLAY_EVIDENCE_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        require_nonempty(self.lane_id.as_str(), "replay evidence lane_id")?;
        require_nonempty(
            self.validator_run_id.as_str(),
            "replay evidence validator_run_id",
        )?;
        require_nonempty(
            self.challenged_run_id.as_str(),
            "replay evidence challenged_run_id",
        )?;
        require_nonempty(self.window_id.as_str(), "replay evidence window_id")?;
        require_nonempty(self.assignment_id.as_str(), "replay evidence assignment_id")?;
        require_nonempty(self.challenge_id.as_str(), "replay evidence challenge_id")?;
        require_nonempty(
            self.validator_node_pubkey.as_str(),
            "replay evidence validator_node_pubkey",
        )?;
        require_nonempty(
            self.challenged_node_pubkey.as_str(),
            "replay evidence challenged_node_pubkey",
        )?;
        require_nonempty(
            self.contribution_id.as_str(),
            "replay evidence contribution_id",
        )?;
        require_nonempty(
            self.contribution_digest.as_str(),
            "replay evidence contribution_digest",
        )?;
        require_nonempty(
            self.artifact_manifest_digest.as_str(),
            "replay evidence artifact_manifest_digest",
        )?;
        require_nonempty(
            self.execution_summary_path.as_str(),
            "replay evidence execution_summary_path",
        )?;
        require_nonempty(
            self.execution_summary_digest.as_str(),
            "replay evidence execution_summary_digest",
        )?;
        require_nonempty(self.detail.as_str(), "replay evidence detail")?;
        if self.reason_codes.is_empty() {
            return Err(PsionicTrainGroupedReplicaEvidenceError::Invalid {
                detail: String::from(
                    "grouped stage replay evidence must carry at least one reason code",
                ),
            });
        }
        if self.replay_evidence_digest != self.stable_replay_evidence_digest() {
            return Err(
                PsionicTrainGroupedReplicaEvidenceError::ArtifactDigestMismatch {
                    detail: String::from("grouped stage replay evidence digest drifted"),
                },
            );
        }
        Ok(())
    }
}

pub fn persist_psionic_train_grouped_stage_execution_summary(
    manifest: &PsionicTrainInvocationManifest,
    run_id: &str,
    contribution_id: &str,
    contribution_root: &Path,
    outcome: PsionicTrainOutcomeKind,
    detail: &str,
    grouped_stage_output_transport: Option<&PsionicTrainGroupedReplicaStageTransportArtifacts>,
) -> Result<
    Option<PsionicTrainGroupedReplicaStageExecutionSummaryArtifacts>,
    PsionicTrainGroupedReplicaEvidenceError,
> {
    let Some(grouped_stage_assignment) = manifest.grouped_stage_assignment.as_ref() else {
        return Ok(None);
    };
    let Some(window_id) = manifest.coordination.window_id.as_deref() else {
        return Ok(None);
    };
    let Some(assignment_id) = manifest.coordination.assignment_id.as_deref() else {
        return Ok(None);
    };
    let Some(node_pubkey) = manifest.coordination.node_pubkey.as_deref() else {
        return Ok(None);
    };
    let input_transport = manifest
        .grouped_stage_input_transport
        .as_ref()
        .and_then(|value| value.materialized_path.as_deref())
        .map(|path| load_psionic_train_grouped_stage_transport(Path::new(path)))
        .transpose()
        .map_err(map_transport_error)?;
    let summary_path = contribution_root.join("grouped_stage_execution_summary.json");
    let mut summary = PsionicTrainGroupedReplicaStageExecutionSummary {
        schema_version: String::from(PSIONIC_TRAIN_GROUPED_STAGE_EXECUTION_SUMMARY_SCHEMA_VERSION),
        lane_id: manifest.lane_id.clone(),
        network_id: manifest.coordination.network_id.clone(),
        run_id: String::from(run_id),
        window_id: String::from(window_id),
        assignment_id: String::from(assignment_id),
        contribution_id: String::from(contribution_id),
        node_pubkey: String::from(node_pubkey),
        grouped_stage_assignment: grouped_stage_assignment.clone(),
        outcome,
        input_transport_path: manifest
            .grouped_stage_input_transport
            .as_ref()
            .and_then(|value| value.materialized_path.clone()),
        input_transport_digest: input_transport
            .as_ref()
            .map(|value| value.envelope.transport_digest.clone()),
        input_payload_digest: input_transport
            .as_ref()
            .map(|value| value.payload.payload_digest.clone()),
        output_transport_path: grouped_stage_output_transport
            .map(|value| value.grouped_stage_output_transport_path.clone()),
        output_transport_digest: grouped_stage_output_transport
            .map(|value| value.grouped_stage_output_transport_digest.clone()),
        output_payload_path: grouped_stage_output_transport
            .map(|value| value.grouped_stage_output_payload_path.clone()),
        output_payload_digest: grouped_stage_output_transport
            .map(|value| value.grouped_stage_output_payload_digest.clone()),
        detail: detail.to_string(),
        execution_summary_digest: String::new(),
    };
    summary.execution_summary_digest = summary.stable_execution_summary_digest();
    summary.validate()?;
    write_json(summary_path.as_path(), &summary)?;
    Ok(Some(
        PsionicTrainGroupedReplicaStageExecutionSummaryArtifacts {
            grouped_stage_execution_summary_path: summary_path.display().to_string(),
            grouped_stage_execution_summary_digest: summary.execution_summary_digest,
        },
    ))
}

pub fn load_psionic_train_grouped_stage_execution_summary(
    path: &Path,
) -> Result<PsionicTrainGroupedReplicaStageExecutionSummary, PsionicTrainGroupedReplicaEvidenceError>
{
    let summary: PsionicTrainGroupedReplicaStageExecutionSummary = read_json(path)?;
    summary.validate()?;
    Ok(summary)
}

pub fn persist_psionic_train_grouped_stage_replay_evidence(
    path: &Path,
    mut evidence: PsionicTrainGroupedReplicaStageReplayEvidence,
) -> Result<
    PsionicTrainGroupedReplicaStageReplayEvidenceArtifacts,
    PsionicTrainGroupedReplicaEvidenceError,
> {
    evidence.replay_evidence_digest = evidence.stable_replay_evidence_digest();
    evidence.validate()?;
    write_json(path, &evidence)?;
    Ok(PsionicTrainGroupedReplicaStageReplayEvidenceArtifacts {
        grouped_stage_replay_evidence_path: path.display().to_string(),
        grouped_stage_replay_evidence_digest: evidence.replay_evidence_digest,
    })
}

pub fn load_psionic_train_grouped_stage_replay_evidence(
    path: &Path,
) -> Result<PsionicTrainGroupedReplicaStageReplayEvidence, PsionicTrainGroupedReplicaEvidenceError>
{
    let evidence: PsionicTrainGroupedReplicaStageReplayEvidence = read_json(path)?;
    evidence.validate()?;
    Ok(evidence)
}

fn map_transport_error(
    error: crate::PsionicTrainGroupedReplicaTransportError,
) -> PsionicTrainGroupedReplicaEvidenceError {
    match error {
        crate::PsionicTrainGroupedReplicaTransportError::Read { path, detail } => {
            PsionicTrainGroupedReplicaEvidenceError::Read { path, detail }
        }
        crate::PsionicTrainGroupedReplicaTransportError::Write { path, detail } => {
            PsionicTrainGroupedReplicaEvidenceError::Write { path, detail }
        }
        crate::PsionicTrainGroupedReplicaTransportError::Parse { path, detail } => {
            PsionicTrainGroupedReplicaEvidenceError::Parse { path, detail }
        }
        crate::PsionicTrainGroupedReplicaTransportError::Invalid { detail } => {
            PsionicTrainGroupedReplicaEvidenceError::Invalid { detail }
        }
        crate::PsionicTrainGroupedReplicaTransportError::ArtifactDigestMismatch { detail } => {
            PsionicTrainGroupedReplicaEvidenceError::ArtifactDigestMismatch { detail }
        }
        crate::PsionicTrainGroupedReplicaTransportError::StaleAssignment { detail } => {
            PsionicTrainGroupedReplicaEvidenceError::StaleAssignment { detail }
        }
    }
}

fn require_nonempty(
    value: &str,
    label: &str,
) -> Result<(), PsionicTrainGroupedReplicaEvidenceError> {
    if value.trim().is_empty() {
        return Err(PsionicTrainGroupedReplicaEvidenceError::Invalid {
            detail: format!("{label} must not be empty"),
        });
    }
    Ok(())
}

fn require_transport_pair(
    path: Option<&str>,
    transport_digest: Option<&str>,
    payload_digest: Option<&str>,
    label: &str,
) -> Result<(), PsionicTrainGroupedReplicaEvidenceError> {
    let declared_count = [path, transport_digest, payload_digest]
        .iter()
        .filter(|value| value.is_some())
        .count();
    if declared_count != 0 && declared_count != 3 {
        return Err(PsionicTrainGroupedReplicaEvidenceError::Invalid {
            detail: format!(
                "grouped stage {label} transport path and digests must be declared together"
            ),
        });
    }
    if let Some(path) = path {
        require_nonempty(
            path,
            format!("grouped stage {label} transport path").as_str(),
        )?;
        require_nonempty(
            transport_digest.unwrap_or(""),
            format!("grouped stage {label} transport digest").as_str(),
        )?;
        require_nonempty(
            payload_digest.unwrap_or(""),
            format!("grouped stage {label} payload digest").as_str(),
        )?;
    }
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut digest = Sha256::new();
    digest.update(prefix);
    digest.update(
        serde_json::to_vec(value).expect("grouped stage evidence should serialize for digest"),
    );
    format!("{:x}", digest.finalize())
}

fn read_json<T: DeserializeOwned>(
    path: &Path,
) -> Result<T, PsionicTrainGroupedReplicaEvidenceError> {
    let bytes = fs::read(path).map_err(|error| PsionicTrainGroupedReplicaEvidenceError::Read {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionicTrainGroupedReplicaEvidenceError::Parse {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

fn write_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), PsionicTrainGroupedReplicaEvidenceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionicTrainGroupedReplicaEvidenceError::Write {
                path: parent.display().to_string(),
                detail: error.to_string(),
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        PsionicTrainGroupedReplicaEvidenceError::Write {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    fs::write(path, bytes).map_err(|error| PsionicTrainGroupedReplicaEvidenceError::Write {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}
