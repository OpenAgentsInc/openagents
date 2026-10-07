use std::{fs, path::Path};

use psionic_runtime::{ClusterShardHandoffKind, ClusterTransportClass};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::PsionicTrainInvocationManifest;

pub const PSIONIC_TRAIN_GROUPED_STAGE_PAYLOAD_SCHEMA_VERSION: &str =
    "psionic.train.grouped_replica_stage_payload.v1";
pub const PSIONIC_TRAIN_GROUPED_STAGE_TRANSPORT_SCHEMA_VERSION: &str =
    "psionic.train.grouped_replica_stage_transport.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainGroupedReplicaStageTransportPayload {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub replica_id: String,
    pub stage_id: String,
    pub node_pubkey: String,
    pub detail: String,
    pub payload_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainGroupedReplicaStageTransportEnvelope {
    pub schema_version: String,
    pub lane_id: String,
    pub network_id: Option<String>,
    pub run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub replica_id: String,
    pub from_stage_id: String,
    pub to_stage_id: String,
    pub handoff_kind: ClusterShardHandoffKind,
    pub transport: ClusterTransportClass,
    pub payload_path: String,
    pub payload_sha256: String,
    pub payload_bytes: u64,
    pub source_stage_assignment_digest: String,
    pub detail: String,
    pub transport_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsionicTrainGroupedReplicaStageTransportArtifacts {
    pub grouped_stage_output_transport_path: String,
    pub grouped_stage_output_transport_digest: String,
    pub grouped_stage_output_payload_path: String,
    pub grouped_stage_output_payload_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsionicTrainGroupedReplicaStageLoadedTransport {
    pub envelope: PsionicTrainGroupedReplicaStageTransportEnvelope,
    pub payload: PsionicTrainGroupedReplicaStageTransportPayload,
}

#[derive(Debug, Error)]
pub enum PsionicTrainGroupedReplicaTransportError {
    #[error("failed to read `{path}`: {detail}")]
    Read { path: String, detail: String },
    #[error("failed to write `{path}`: {detail}")]
    Write { path: String, detail: String },
    #[error("failed to parse `{path}`: {detail}")]
    Parse { path: String, detail: String },
    #[error("grouped-replica stage transport is invalid: {detail}")]
    Invalid { detail: String },
    #[error("grouped-replica stage transport drifted: {detail}")]
    ArtifactDigestMismatch { detail: String },
    #[error("grouped-replica stage transport is stale: {detail}")]
    StaleAssignment { detail: String },
}

impl PsionicTrainGroupedReplicaStageTransportPayload {
    #[must_use]
    pub fn stable_payload_digest(&self) -> String {
        let mut digest_basis = self.clone();
        digest_basis.payload_digest.clear();
        stable_digest(b"psionic_train_grouped_stage_payload|", &digest_basis)
    }

    pub fn validate(&self) -> Result<(), PsionicTrainGroupedReplicaTransportError> {
        if self.schema_version != PSIONIC_TRAIN_GROUPED_STAGE_PAYLOAD_SCHEMA_VERSION {
            return Err(PsionicTrainGroupedReplicaTransportError::Invalid {
                detail: format!(
                    "grouped stage payload schema version must stay `{}` but was `{}`",
                    PSIONIC_TRAIN_GROUPED_STAGE_PAYLOAD_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        require_nonempty(self.lane_id.as_str(), "payload lane_id")?;
        require_nonempty(self.run_id.as_str(), "payload run_id")?;
        require_nonempty(self.window_id.as_str(), "payload window_id")?;
        require_nonempty(self.assignment_id.as_str(), "payload assignment_id")?;
        require_nonempty(self.replica_id.as_str(), "payload replica_id")?;
        require_nonempty(self.stage_id.as_str(), "payload stage_id")?;
        require_nonempty(self.node_pubkey.as_str(), "payload node_pubkey")?;
        require_nonempty(self.detail.as_str(), "payload detail")?;
        if self.payload_digest != self.stable_payload_digest() {
            return Err(
                PsionicTrainGroupedReplicaTransportError::ArtifactDigestMismatch {
                    detail: String::from("grouped stage payload digest drifted"),
                },
            );
        }
        Ok(())
    }
}

impl PsionicTrainGroupedReplicaStageTransportEnvelope {
    #[must_use]
    pub fn stable_transport_digest(&self) -> String {
        let mut digest_basis = self.clone();
        digest_basis.transport_digest.clear();
        stable_digest(b"psionic_train_grouped_stage_transport|", &digest_basis)
    }

    pub fn validate(&self) -> Result<(), PsionicTrainGroupedReplicaTransportError> {
        if self.schema_version != PSIONIC_TRAIN_GROUPED_STAGE_TRANSPORT_SCHEMA_VERSION {
            return Err(PsionicTrainGroupedReplicaTransportError::Invalid {
                detail: format!(
                    "grouped stage transport schema version must stay `{}` but was `{}`",
                    PSIONIC_TRAIN_GROUPED_STAGE_TRANSPORT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        require_nonempty(self.lane_id.as_str(), "transport lane_id")?;
        require_nonempty(self.run_id.as_str(), "transport run_id")?;
        require_nonempty(self.window_id.as_str(), "transport window_id")?;
        require_nonempty(self.assignment_id.as_str(), "transport assignment_id")?;
        require_nonempty(self.replica_id.as_str(), "transport replica_id")?;
        require_nonempty(self.from_stage_id.as_str(), "transport from_stage_id")?;
        require_nonempty(self.to_stage_id.as_str(), "transport to_stage_id")?;
        require_nonempty(self.payload_path.as_str(), "transport payload_path")?;
        require_nonempty(self.payload_sha256.as_str(), "transport payload_sha256")?;
        require_nonempty(
            self.source_stage_assignment_digest.as_str(),
            "transport source_stage_assignment_digest",
        )?;
        require_nonempty(self.detail.as_str(), "transport detail")?;
        if self.from_stage_id == self.to_stage_id {
            return Err(PsionicTrainGroupedReplicaTransportError::Invalid {
                detail: String::from("grouped stage transport must move between distinct stages"),
            });
        }
        if self.payload_bytes == 0 {
            return Err(PsionicTrainGroupedReplicaTransportError::Invalid {
                detail: String::from("grouped stage transport payload_bytes must be non-zero"),
            });
        }
        if self.transport_digest != self.stable_transport_digest() {
            return Err(
                PsionicTrainGroupedReplicaTransportError::ArtifactDigestMismatch {
                    detail: String::from("grouped stage transport digest drifted"),
                },
            );
        }
        Ok(())
    }
}

pub fn validate_psionic_train_grouped_stage_input_transport(
    manifest: &PsionicTrainInvocationManifest,
) -> Result<
    Option<PsionicTrainGroupedReplicaStageTransportEnvelope>,
    PsionicTrainGroupedReplicaTransportError,
> {
    let Some(transport_path) = manifest
        .grouped_stage_input_transport
        .as_ref()
        .map(|value| {
            value.require_materialized_path("invocation_manifest.grouped_stage_input_transport")
        })
        .transpose()
        .map_err(|detail| PsionicTrainGroupedReplicaTransportError::Invalid { detail })?
    else {
        return Ok(None);
    };
    let stage_assignment = manifest.grouped_stage_assignment.as_ref().ok_or_else(|| {
        PsionicTrainGroupedReplicaTransportError::Invalid {
            detail: String::from(
                "grouped stage input transport requires one grouped stage assignment",
            ),
        }
    })?;
    let loaded_transport = load_psionic_train_grouped_stage_transport(Path::new(transport_path))?;
    let envelope = loaded_transport.envelope;
    if envelope.lane_id != manifest.lane_id {
        return Err(
            PsionicTrainGroupedReplicaTransportError::ArtifactDigestMismatch {
                detail: String::from("grouped stage input transport lane_id drifted"),
            },
        );
    }
    if envelope.network_id != manifest.coordination.network_id {
        return Err(PsionicTrainGroupedReplicaTransportError::StaleAssignment {
            detail: String::from("grouped stage input transport network_id drifted"),
        });
    }
    if manifest.run_id.as_deref() != Some(envelope.run_id.as_str()) {
        return Err(PsionicTrainGroupedReplicaTransportError::StaleAssignment {
            detail: format!(
                "grouped stage input transport targets run `{}` but the manifest run is `{}`",
                envelope.run_id,
                manifest.run_id.as_deref().unwrap_or("")
            ),
        });
    }
    if manifest.coordination.window_id.as_deref() != Some(envelope.window_id.as_str()) {
        return Err(PsionicTrainGroupedReplicaTransportError::StaleAssignment {
            detail: format!(
                "grouped stage input transport targets window `{}` but the manifest window is `{}`",
                envelope.window_id,
                manifest.coordination.window_id.as_deref().unwrap_or("")
            ),
        });
    }
    if manifest.coordination.assignment_id.as_deref() != Some(envelope.assignment_id.as_str()) {
        return Err(PsionicTrainGroupedReplicaTransportError::StaleAssignment {
            detail: format!(
                "grouped stage input transport targets assignment `{}` but the manifest assignment is `{}`",
                envelope.assignment_id,
                manifest.coordination.assignment_id.as_deref().unwrap_or("")
            ),
        });
    }
    if envelope.replica_id != stage_assignment.replica_id {
        return Err(PsionicTrainGroupedReplicaTransportError::StaleAssignment {
            detail: format!(
                "grouped stage input transport targets replica `{}` but the manifest stage assignment uses `{}`",
                envelope.replica_id, stage_assignment.replica_id
            ),
        });
    }
    if envelope.to_stage_id != stage_assignment.stage_id {
        return Err(PsionicTrainGroupedReplicaTransportError::StaleAssignment {
            detail: format!(
                "grouped stage input transport targets stage `{}` but the manifest stage is `{}`",
                envelope.to_stage_id, stage_assignment.stage_id
            ),
        });
    }
    if stage_assignment.upstream_stage_id.as_deref() != Some(envelope.from_stage_id.as_str()) {
        return Err(PsionicTrainGroupedReplicaTransportError::StaleAssignment {
            detail: format!(
                "grouped stage input transport comes from `{}` but the manifest expects upstream stage `{}`",
                envelope.from_stage_id,
                stage_assignment.upstream_stage_id.as_deref().unwrap_or("")
            ),
        });
    }
    if envelope.source_stage_assignment_digest.is_empty() {
        return Err(PsionicTrainGroupedReplicaTransportError::Invalid {
            detail: String::from(
                "grouped stage input transport must carry source_stage_assignment_digest",
            ),
        });
    }
    Ok(Some(envelope))
}

pub fn load_psionic_train_grouped_stage_transport(
    transport_path: &Path,
) -> Result<PsionicTrainGroupedReplicaStageLoadedTransport, PsionicTrainGroupedReplicaTransportError>
{
    let envelope: PsionicTrainGroupedReplicaStageTransportEnvelope = load_json(transport_path)?;
    envelope.validate()?;
    let payload = validate_transport_payload(transport_path, &envelope)?;
    Ok(PsionicTrainGroupedReplicaStageLoadedTransport { envelope, payload })
}

pub fn persist_psionic_train_grouped_stage_output_transport(
    manifest: &PsionicTrainInvocationManifest,
    run_id: &str,
    contribution_root: &Path,
) -> Result<
    Option<PsionicTrainGroupedReplicaStageTransportArtifacts>,
    PsionicTrainGroupedReplicaTransportError,
> {
    let Some(stage_assignment) = manifest.grouped_stage_assignment.as_ref() else {
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
    let Some(downstream_stage_id) = stage_assignment.downstream_stage_id.as_deref() else {
        return Ok(None);
    };

    let payload_path = contribution_root.join("grouped_stage_output_payload.json");
    let transport_path = contribution_root.join("grouped_stage_output_transport.json");

    let mut payload = PsionicTrainGroupedReplicaStageTransportPayload {
        schema_version: String::from(PSIONIC_TRAIN_GROUPED_STAGE_PAYLOAD_SCHEMA_VERSION),
        lane_id: manifest.lane_id.clone(),
        run_id: run_id.to_string(),
        window_id: window_id.to_string(),
        assignment_id: assignment_id.to_string(),
        replica_id: stage_assignment.replica_id.clone(),
        stage_id: stage_assignment.stage_id.clone(),
        node_pubkey: node_pubkey.to_string(),
        detail: String::from(
            "Grouped stage output payload retains one deterministic upstream handoff packet for the next assigned stage.",
        ),
        payload_digest: String::new(),
    };
    payload.payload_digest = payload.stable_payload_digest();
    payload.validate()?;
    write_json(payload_path.as_path(), &payload)?;

    let payload_bytes = fs::read(payload_path.as_path()).map_err(|error| {
        PsionicTrainGroupedReplicaTransportError::Read {
            path: payload_path.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    let mut envelope = PsionicTrainGroupedReplicaStageTransportEnvelope {
        schema_version: String::from(PSIONIC_TRAIN_GROUPED_STAGE_TRANSPORT_SCHEMA_VERSION),
        lane_id: manifest.lane_id.clone(),
        network_id: manifest.coordination.network_id.clone(),
        run_id: run_id.to_string(),
        window_id: window_id.to_string(),
        assignment_id: assignment_id.to_string(),
        replica_id: stage_assignment.replica_id.clone(),
        from_stage_id: stage_assignment.stage_id.clone(),
        to_stage_id: downstream_stage_id.to_string(),
        handoff_kind: ClusterShardHandoffKind::Activation,
        transport: ClusterTransportClass::WiderNetworkStream,
        payload_path: payload_path.display().to_string(),
        payload_sha256: sha256_hex(payload_bytes.as_slice()),
        payload_bytes: u64::try_from(payload_bytes.len()).map_err(|error| {
            PsionicTrainGroupedReplicaTransportError::Write {
                path: payload_path.display().to_string(),
                detail: error.to_string(),
            }
        })?,
        source_stage_assignment_digest: stage_assignment.assignment_digest.clone(),
        detail: String::from(
            "Grouped stage transport envelope binds one retained payload to the next grouped-replica stage with explicit integrity fields.",
        ),
        transport_digest: String::new(),
    };
    envelope.transport_digest = envelope.stable_transport_digest();
    envelope.validate()?;
    write_json(transport_path.as_path(), &envelope)?;
    let loaded_transport = load_psionic_train_grouped_stage_transport(transport_path.as_path())?;

    Ok(Some(PsionicTrainGroupedReplicaStageTransportArtifacts {
        grouped_stage_output_transport_path: transport_path.display().to_string(),
        grouped_stage_output_transport_digest: loaded_transport.envelope.transport_digest,
        grouped_stage_output_payload_path: payload_path.display().to_string(),
        grouped_stage_output_payload_digest: loaded_transport.payload.payload_digest,
    }))
}

fn validate_transport_payload(
    transport_path: &Path,
    envelope: &PsionicTrainGroupedReplicaStageTransportEnvelope,
) -> Result<PsionicTrainGroupedReplicaStageTransportPayload, PsionicTrainGroupedReplicaTransportError>
{
    let payload_path = Path::new(envelope.payload_path.as_str());
    let bytes =
        fs::read(payload_path).map_err(|error| PsionicTrainGroupedReplicaTransportError::Read {
            path: payload_path.display().to_string(),
            detail: error.to_string(),
        })?;
    let actual_sha256 = sha256_hex(bytes.as_slice());
    if actual_sha256 != envelope.payload_sha256 {
        return Err(
            PsionicTrainGroupedReplicaTransportError::ArtifactDigestMismatch {
                detail: format!(
                    "grouped stage transport `{}` expected payload digest `{}` but the payload file hashed to `{}`",
                    transport_path.display(),
                    envelope.payload_sha256,
                    actual_sha256
                ),
            },
        );
    }
    let actual_bytes = u64::try_from(bytes.len()).map_err(|error| {
        PsionicTrainGroupedReplicaTransportError::Read {
            path: payload_path.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    if actual_bytes != envelope.payload_bytes {
        return Err(
            PsionicTrainGroupedReplicaTransportError::ArtifactDigestMismatch {
                detail: format!(
                    "grouped stage transport `{}` expected payload_bytes {} but found {}",
                    transport_path.display(),
                    envelope.payload_bytes,
                    actual_bytes
                ),
            },
        );
    }
    let payload: PsionicTrainGroupedReplicaStageTransportPayload = serde_json::from_slice(&bytes)
        .map_err(|error| {
        PsionicTrainGroupedReplicaTransportError::Parse {
            path: payload_path.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    payload.validate()?;
    if payload.lane_id != envelope.lane_id
        || payload.run_id != envelope.run_id
        || payload.window_id != envelope.window_id
        || payload.assignment_id != envelope.assignment_id
        || payload.replica_id != envelope.replica_id
        || payload.stage_id != envelope.from_stage_id
    {
        return Err(
            PsionicTrainGroupedReplicaTransportError::ArtifactDigestMismatch {
                detail: format!(
                    "grouped stage transport `{}` drifted from the retained payload identity",
                    transport_path.display()
                ),
            },
        );
    }
    Ok(payload)
}

fn require_nonempty(
    value: &str,
    label: &str,
) -> Result<(), PsionicTrainGroupedReplicaTransportError> {
    if value.trim().is_empty() {
        return Err(PsionicTrainGroupedReplicaTransportError::Invalid {
            detail: format!("{label} must not be empty"),
        });
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut digest = Sha256::new();
    digest.update(prefix);
    digest.update(
        serde_json::to_vec(value)
            .expect("grouped stage transport should serialize for stable digest"),
    );
    format!("{:x}", digest.finalize())
}

fn load_json<T: DeserializeOwned>(
    path: &Path,
) -> Result<T, PsionicTrainGroupedReplicaTransportError> {
    let bytes = fs::read(path).map_err(|error| PsionicTrainGroupedReplicaTransportError::Read {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionicTrainGroupedReplicaTransportError::Parse {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    })
}

fn write_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), PsionicTrainGroupedReplicaTransportError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionicTrainGroupedReplicaTransportError::Write {
                path: parent.display().to_string(),
                detail: error.to_string(),
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        PsionicTrainGroupedReplicaTransportError::Write {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    fs::write(path, bytes).map_err(|error| PsionicTrainGroupedReplicaTransportError::Write {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        persist_psionic_train_grouped_stage_output_transport,
        validate_psionic_train_grouped_stage_input_transport,
        PsionicTrainGroupedReplicaTransportError,
    };
    use crate::{
        PsionicTrainAdmissionIdentity, PsionicTrainCoordinationContext,
        PsionicTrainGroupedReplicaStageAssignment, PsionicTrainGroupedReplicaStageRole,
        PsionicTrainInvocationManifest, PsionicTrainOperation, PsionicTrainRole,
        PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF,
        PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID,
        PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION, PSIONIC_TRAIN_RUNTIME_SURFACE_ID,
    };

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let path = env::temp_dir().join(format!("psionic-grouped-transport-{label}-{unique}"));
        if path.exists() {
            fs::remove_dir_all(&path).expect("temp dir should clear");
        }
        fs::create_dir_all(&path).expect("temp dir should create");
        path
    }

    fn manifest_for_stage(
        stage_assignment: PsionicTrainGroupedReplicaStageAssignment,
    ) -> PsionicTrainInvocationManifest {
        PsionicTrainInvocationManifest {
            schema_version: String::from(PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION),
            runtime_surface_id: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
            lane_id: String::from(crate::PSION_ACTUAL_PRETRAINING_LANE_ID),
            role: PsionicTrainRole::Worker,
            operation: PsionicTrainOperation::Start,
            work_class: crate::PsionicTrainWorkClass::GroupedReplicaStageExecution,
            coordination: PsionicTrainCoordinationContext {
                network_id: Some(String::from("network.psionic.transport-test")),
                window_id: Some(String::from("window-0001")),
                assignment_id: Some(String::from("assignment-0001")),
                challenge_id: None,
                node_pubkey: Some(String::from("npub1-stage-node")),
                membership_revision: Some(3),
            },
            grouped_stage_assignment: Some(stage_assignment),
            admission_identity: PsionicTrainAdmissionIdentity {
                release_id: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID),
                build_digest: String::from("sha256:test-build"),
                environment_ref: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF),
            },
            run_id: Some(String::from("grouped-stage-run")),
            output_root: Some(String::from("/tmp/grouped-stage-run")),
            run_root: None,
            peer_node_pubkey: None,
            peer_checkpoint_handoff_receipt: None,
            validator_target_contribution_receipt: None,
            validator_target_contribution_artifact_manifest: None,
            validator_target_work_class: None,
            grouped_stage_input_transport: None,
            selected_git_ref: Some(String::from("HEAD")),
            hardware_observation_path: None,
            run_shape_observation_path: None,
            allow_dirty_tree: false,
            dry_run: true,
            checkpoint_label: None,
            optimizer_step: None,
            checkpoint_ref: None,
            checkpoint_object_digest: None,
            checkpoint_total_bytes: None,
            inject_failed_upload: false,
            inject_eval_worker_unavailable: false,
            manifest_digest: None,
        }
    }

    #[test]
    fn grouped_stage_input_transport_validates_for_downstream_stage() {
        let run_root = temp_root("valid");
        let contribution_root = run_root.join("windows/window-0001/contributions/contribution-1");
        fs::create_dir_all(&contribution_root).expect("contribution dir should create");

        let source_manifest = manifest_for_stage(
            PsionicTrainGroupedReplicaStageAssignment::new(
                "replica-01",
                "stage-01",
                0,
                2,
                PsionicTrainGroupedReplicaStageRole::Ingress,
                None,
                Some(String::from("stage-02")),
            )
            .expect("source stage should build"),
        );
        let artifacts = persist_psionic_train_grouped_stage_output_transport(
            &source_manifest,
            "grouped-stage-run",
            &contribution_root,
        )
        .expect("output transport should persist")
        .expect("ingress stage should emit transport");

        let mut destination_manifest = manifest_for_stage(
            PsionicTrainGroupedReplicaStageAssignment::new(
                "replica-01",
                "stage-02",
                1,
                2,
                PsionicTrainGroupedReplicaStageRole::Egress,
                Some(String::from("stage-01")),
                None,
            )
            .expect("destination stage should build"),
        );
        destination_manifest.grouped_stage_input_transport = Some(
            crate::build_psionic_train_artifact_binding_from_path(
                "grouped_stage_output_transport",
                std::path::Path::new(artifacts.grouped_stage_output_transport_path.as_str()),
            )
            .expect("artifact binding should build"),
        );

        let envelope = validate_psionic_train_grouped_stage_input_transport(&destination_manifest)
            .expect("destination stage input transport should validate")
            .expect("non-ingress stage should load transport");
        assert_eq!(envelope.from_stage_id, "stage-01");
        assert_eq!(envelope.to_stage_id, "stage-02");
    }

    #[test]
    fn grouped_stage_input_transport_rejects_payload_digest_drift() {
        let run_root = temp_root("digest-drift");
        let contribution_root = run_root.join("windows/window-0001/contributions/contribution-1");
        fs::create_dir_all(&contribution_root).expect("contribution dir should create");

        let source_manifest = manifest_for_stage(
            PsionicTrainGroupedReplicaStageAssignment::new(
                "replica-01",
                "stage-01",
                0,
                2,
                PsionicTrainGroupedReplicaStageRole::Ingress,
                None,
                Some(String::from("stage-02")),
            )
            .expect("source stage should build"),
        );
        let artifacts = persist_psionic_train_grouped_stage_output_transport(
            &source_manifest,
            "grouped-stage-run",
            &contribution_root,
        )
        .expect("output transport should persist")
        .expect("ingress stage should emit transport");
        fs::write(
            &artifacts.grouped_stage_output_payload_path,
            br#"{"schema_version":"drift"}"#,
        )
        .expect("payload drift should write");

        let mut destination_manifest = manifest_for_stage(
            PsionicTrainGroupedReplicaStageAssignment::new(
                "replica-01",
                "stage-02",
                1,
                2,
                PsionicTrainGroupedReplicaStageRole::Egress,
                Some(String::from("stage-01")),
                None,
            )
            .expect("destination stage should build"),
        );
        destination_manifest.grouped_stage_input_transport = Some(
            crate::build_psionic_train_artifact_binding_from_path(
                "grouped_stage_output_transport",
                std::path::Path::new(artifacts.grouped_stage_output_transport_path.as_str()),
            )
            .expect("artifact binding should build"),
        );

        let error = validate_psionic_train_grouped_stage_input_transport(&destination_manifest)
            .expect_err("drifted payload should fail validation");
        assert!(matches!(
            error,
            PsionicTrainGroupedReplicaTransportError::ArtifactDigestMismatch { .. }
                | PsionicTrainGroupedReplicaTransportError::Parse { .. }
        ));
    }

    #[test]
    fn grouped_stage_input_transport_rejects_wrong_destination_stage() {
        let run_root = temp_root("wrong-destination");
        let contribution_root = run_root.join("windows/window-0001/contributions/contribution-1");
        fs::create_dir_all(&contribution_root).expect("contribution dir should create");

        let source_manifest = manifest_for_stage(
            PsionicTrainGroupedReplicaStageAssignment::new(
                "replica-01",
                "stage-01",
                0,
                2,
                PsionicTrainGroupedReplicaStageRole::Ingress,
                None,
                Some(String::from("stage-02")),
            )
            .expect("source stage should build"),
        );
        let artifacts = persist_psionic_train_grouped_stage_output_transport(
            &source_manifest,
            "grouped-stage-run",
            &contribution_root,
        )
        .expect("output transport should persist")
        .expect("ingress stage should emit transport");

        let mut destination_manifest = manifest_for_stage(
            PsionicTrainGroupedReplicaStageAssignment::new(
                "replica-01",
                "stage-03",
                1,
                2,
                PsionicTrainGroupedReplicaStageRole::Egress,
                Some(String::from("stage-01")),
                None,
            )
            .expect("destination stage should build"),
        );
        destination_manifest.grouped_stage_input_transport = Some(
            crate::build_psionic_train_artifact_binding_from_path(
                "grouped_stage_output_transport",
                std::path::Path::new(artifacts.grouped_stage_output_transport_path.as_str()),
            )
            .expect("artifact binding should build"),
        );

        let error = validate_psionic_train_grouped_stage_input_transport(&destination_manifest)
            .expect_err("wrong destination stage should fail validation");
        assert!(matches!(
            error,
            PsionicTrainGroupedReplicaTransportError::StaleAssignment { .. }
        ));
    }
}
