use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionActualPretrainingArtifactRef, PsionActualPretrainingEvidenceContract,
    PsionActualPretrainingSystemsBundle, PsionActualPretrainingTopologyStorageBundle,
    PSION_ACTUAL_PRETRAINING_LANE_ID,
};

/// Stable schema version for the actual-lane hardware observation input.
pub const PSION_ACTUAL_PRETRAINING_HARDWARE_OBSERVATION_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_hardware_observation.v1";

/// Stable schema version for the actual-lane hardware qualification receipt.
pub const PSION_ACTUAL_PRETRAINING_HARDWARE_QUALIFICATION_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_hardware_qualification.v1";

/// Canonical fixture path for the admitted hardware observation fixture.
pub const PSION_ACTUAL_PRETRAINING_HARDWARE_OBSERVATION_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_hardware_observation_admitted_v1.json";

/// Canonical fixture path for the canonical hardware qualification fixture.
pub const PSION_ACTUAL_PRETRAINING_HARDWARE_QUALIFICATION_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_hardware_qualification_v1.json";

/// Canonical focused doc path for actual-lane hardware qualification.
pub const PSION_ACTUAL_PRETRAINING_HARDWARE_QUALIFICATION_DOC_PATH: &str =
    "docs/PSION_ACTUAL_PRETRAINING_HARDWARE_QUALIFICATION.md";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingObservedWorker {
    pub worker_label: String,
    pub backend: String,
    pub device_name: String,
    pub total_memory_bytes: u64,
    pub free_memory_bytes: u64,
    pub temperature_celsius: Option<u64>,
    pub ecc_uncorrected_error_count: Option<u64>,
    pub throttling_observed: Option<bool>,
    pub resident_compute_process_count: Option<u64>,
    pub mig_partitioned: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingObservedCredentialSource {
    pub source_name: String,
    pub kind: String,
    pub present: bool,
    pub redacted_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingHardwareObservation {
    pub schema_version: String,
    pub observation_id: String,
    pub lane_id: String,
    pub observation_kind: String,
    pub observed_at_utc: String,
    pub backend: String,
    pub workers: Vec<PsionActualPretrainingObservedWorker>,
    pub credential_sources: Vec<PsionActualPretrainingObservedCredentialSource>,
    pub checkpoint_restore_ready: bool,
    pub summary: String,
    pub observation_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingHardwareQualification {
    pub schema_version: String,
    pub qualification_id: String,
    pub lane_id: String,
    pub run_id: String,
    pub selected_git_ref: String,
    pub git_commit_sha: String,
    pub dirty_tree_admission: String,
    pub observation_kind: String,
    pub observation_artifact: Option<PsionActualPretrainingArtifactRef>,
    pub topology_storage_bundle: PsionActualPretrainingArtifactRef,
    pub systems_bundle: PsionActualPretrainingArtifactRef,
    pub evidence_contract: PsionActualPretrainingArtifactRef,
    pub required_backend: String,
    pub required_worker_count: u64,
    pub required_device_name_substring: String,
    pub required_min_free_memory_bytes_per_worker: u64,
    pub required_credential_sources: Vec<String>,
    pub required_recovery_event_ids: Vec<String>,
    pub observed_at_utc: String,
    pub observed_workers: Vec<PsionActualPretrainingObservedWorker>,
    pub credential_checks: Vec<PsionActualPretrainingObservedCredentialSource>,
    pub checkpoint_restore_ready: bool,
    pub admission_state: String,
    pub refusal_reasons: Vec<String>,
    pub claim_boundary: String,
    pub detail: String,
    pub receipt_digest: String,
}

impl PsionActualPretrainingHardwareObservation {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingHardwareQualificationError> {
        ensure_exact(
            self.schema_version.as_str(),
            "hardware_observation.schema_version",
            PSION_ACTUAL_PRETRAINING_HARDWARE_OBSERVATION_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "hardware_observation.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_nonempty(
            self.observation_id.as_str(),
            "hardware_observation.observation_id",
        )?;
        ensure_nonempty(
            self.observation_kind.as_str(),
            "hardware_observation.observation_kind",
        )?;
        ensure_nonempty(
            self.observed_at_utc.as_str(),
            "hardware_observation.observed_at_utc",
        )?;
        ensure_nonempty(self.backend.as_str(), "hardware_observation.backend")?;
        ensure_nonempty(self.summary.as_str(), "hardware_observation.summary")?;
        validate_workers(self.workers.as_slice(), "hardware_observation.workers")?;
        validate_credentials(
            self.credential_sources.as_slice(),
            "hardware_observation.credential_sources",
        )?;
        if self.observation_digest != stable_hardware_observation_digest(self)? {
            return Err(PsionActualPretrainingHardwareQualificationError::DigestMismatch {
                field: String::from("hardware_observation.observation_digest"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingHardwareQualification {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingHardwareQualificationError> {
        ensure_exact(
            self.schema_version.as_str(),
            "hardware_qualification.schema_version",
            PSION_ACTUAL_PRETRAINING_HARDWARE_QUALIFICATION_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "hardware_qualification.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        for (field, value) in [
            (
                "hardware_qualification.qualification_id",
                self.qualification_id.as_str(),
            ),
            ("hardware_qualification.run_id", self.run_id.as_str()),
            (
                "hardware_qualification.selected_git_ref",
                self.selected_git_ref.as_str(),
            ),
            (
                "hardware_qualification.git_commit_sha",
                self.git_commit_sha.as_str(),
            ),
            (
                "hardware_qualification.dirty_tree_admission",
                self.dirty_tree_admission.as_str(),
            ),
            (
                "hardware_qualification.observation_kind",
                self.observation_kind.as_str(),
            ),
            (
                "hardware_qualification.required_backend",
                self.required_backend.as_str(),
            ),
            (
                "hardware_qualification.required_device_name_substring",
                self.required_device_name_substring.as_str(),
            ),
            (
                "hardware_qualification.observed_at_utc",
                self.observed_at_utc.as_str(),
            ),
            (
                "hardware_qualification.admission_state",
                self.admission_state.as_str(),
            ),
            (
                "hardware_qualification.claim_boundary",
                self.claim_boundary.as_str(),
            ),
            ("hardware_qualification.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.required_worker_count == 0 {
            return Err(PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                field: String::from("hardware_qualification.required_worker_count"),
                detail: String::from("required worker count must stay positive"),
            });
        }
        if self.required_min_free_memory_bytes_per_worker == 0 {
            return Err(PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                field: String::from(
                    "hardware_qualification.required_min_free_memory_bytes_per_worker",
                ),
                detail: String::from("required free-memory floor must stay positive"),
            });
        }
        ensure_artifact_ref(
            &self.topology_storage_bundle,
            "hardware_qualification.topology_storage_bundle",
        )?;
        ensure_artifact_ref(
            &self.systems_bundle,
            "hardware_qualification.systems_bundle",
        )?;
        ensure_artifact_ref(
            &self.evidence_contract,
            "hardware_qualification.evidence_contract",
        )?;
        if let Some(observation_artifact) = &self.observation_artifact {
            ensure_artifact_ref(
                observation_artifact,
                "hardware_qualification.observation_artifact",
            )?;
        }
        ensure_unique_nonempty_strings(
            self.required_credential_sources.as_slice(),
            "hardware_qualification.required_credential_sources[]",
        )?;
        ensure_unique_nonempty_strings(
            self.required_recovery_event_ids.as_slice(),
            "hardware_qualification.required_recovery_event_ids[]",
        )?;
        validate_workers(
            self.observed_workers.as_slice(),
            "hardware_qualification.observed_workers",
        )?;
        validate_credentials(
            self.credential_checks.as_slice(),
            "hardware_qualification.credential_checks",
        )?;
        match self.admission_state.as_str() {
            "admitted" => {
                if !self.refusal_reasons.is_empty() {
                    return Err(
                        PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                            field: String::from("hardware_qualification.refusal_reasons"),
                            detail: String::from(
                                "admitted hardware qualification must not retain refusal reasons",
                            ),
                        },
                    );
                }
                if self.observed_workers.len() as u64 != self.required_worker_count {
                    return Err(
                        PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                            field: String::from("hardware_qualification.observed_workers"),
                            detail: String::from(
                                "admitted hardware qualification must retain the required worker count",
                            ),
                        },
                    );
                }
                if !self.checkpoint_restore_ready {
                    return Err(
                        PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                            field: String::from("hardware_qualification.checkpoint_restore_ready"),
                            detail: String::from(
                                "admitted hardware qualification must retain checkpoint_restore_ready = true",
                            ),
                        },
                    );
                }
            }
            "refused" => {
                ensure_unique_nonempty_strings(
                    self.refusal_reasons.as_slice(),
                    "hardware_qualification.refusal_reasons[]",
                )?;
            }
            _ => {
                return Err(
                    PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                        field: String::from("hardware_qualification.admission_state"),
                        detail: String::from("admission state must be admitted or refused"),
                    },
                );
            }
        }
        if self.receipt_digest != stable_hardware_qualification_digest(self)? {
            return Err(PsionActualPretrainingHardwareQualificationError::DigestMismatch {
                field: String::from("hardware_qualification.receipt_digest"),
            });
        }
        Ok(())
    }
}

pub fn derive_psion_actual_pretraining_hardware_qualification(
    run_id: &str,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    observation: &PsionActualPretrainingHardwareObservation,
    observation_artifact: Option<PsionActualPretrainingArtifactRef>,
    topology_storage_bundle_ref: PsionActualPretrainingArtifactRef,
    systems_bundle_ref: PsionActualPretrainingArtifactRef,
    evidence_contract_ref: PsionActualPretrainingArtifactRef,
    topology: &PsionActualPretrainingTopologyStorageBundle,
    systems_bundle: &PsionActualPretrainingSystemsBundle,
    evidence_contract: &PsionActualPretrainingEvidenceContract,
) -> Result<PsionActualPretrainingHardwareQualification, PsionActualPretrainingHardwareQualificationError>
{
    observation.validate()?;
    systems_bundle.validate().map_err(|error| {
        PsionActualPretrainingHardwareQualificationError::UpstreamValidation {
            surface: String::from("systems_bundle"),
            detail: error.to_string(),
        }
    })?;
    topology.validate().map_err(|error| {
        PsionActualPretrainingHardwareQualificationError::UpstreamValidation {
            surface: String::from("topology_storage_bundle"),
            detail: error.to_string(),
        }
    })?;
    evidence_contract.validate().map_err(|error| {
        PsionActualPretrainingHardwareQualificationError::UpstreamValidation {
            surface: String::from("evidence_contract"),
            detail: error.to_string(),
        }
    })?;

    let required_device_name_substring = if topology
        .supported_topology_label
        .to_ascii_lowercase()
        .contains("h100")
    {
        String::from("H100")
    } else {
        topology.supported_topology_label.clone()
    };
    let mut refusal_reasons = Vec::new();
    if observation.backend != topology.required_backend {
        refusal_reasons.push(format!(
            "required backend `{}` but observed `{}`",
            topology.required_backend, observation.backend
        ));
    }
    if observation.workers.len() as u64 != topology.required_worker_count {
        refusal_reasons.push(format!(
            "required {} worker(s) but observed {}",
            topology.required_worker_count,
            observation.workers.len()
        ));
    }
    for worker in &observation.workers {
        if worker.backend != topology.required_backend {
            refusal_reasons.push(format!(
                "worker `{}` reported backend `{}` instead of `{}`",
                worker.worker_label, worker.backend, topology.required_backend
            ));
        }
        if !worker
            .device_name
            .to_ascii_uppercase()
            .contains(&required_device_name_substring.to_ascii_uppercase())
        {
            refusal_reasons.push(format!(
                "worker `{}` device `{}` does not satisfy required device substring `{}`",
                worker.worker_label, worker.device_name, required_device_name_substring
            ));
        }
        if worker.total_memory_bytes < systems_bundle.memory_qualification.per_worker_total_memory_bytes
        {
            refusal_reasons.push(format!(
                "worker `{}` total memory {} is below required {}",
                worker.worker_label,
                worker.total_memory_bytes,
                systems_bundle.memory_qualification.per_worker_total_memory_bytes
            ));
        }
        if worker.free_memory_bytes
            < systems_bundle.memory_qualification.min_per_worker_free_memory_bytes
        {
            refusal_reasons.push(format!(
                "worker `{}` free memory {} is below required {}",
                worker.worker_label,
                worker.free_memory_bytes,
                systems_bundle.memory_qualification.min_per_worker_free_memory_bytes
            ));
        }
        match worker.temperature_celsius {
            Some(temperature_celsius) if temperature_celsius < 80 => {}
            Some(temperature_celsius) => refusal_reasons.push(format!(
                "worker `{}` temperature {}C exceeds the launch ceiling",
                worker.worker_label, temperature_celsius
            )),
            None => refusal_reasons.push(format!(
                "worker `{}` did not report temperature telemetry",
                worker.worker_label
            )),
        }
        match worker.ecc_uncorrected_error_count {
            Some(0) => {}
            Some(error_count) => refusal_reasons.push(format!(
                "worker `{}` reported {} ECC uncorrected error(s)",
                worker.worker_label, error_count
            )),
            None => refusal_reasons.push(format!(
                "worker `{}` did not report ECC telemetry",
                worker.worker_label
            )),
        }
        match worker.throttling_observed {
            Some(false) => {}
            Some(true) => refusal_reasons.push(format!(
                "worker `{}` reported active throttling",
                worker.worker_label
            )),
            None => refusal_reasons.push(format!(
                "worker `{}` did not report throttling telemetry",
                worker.worker_label
            )),
        }
        match worker.resident_compute_process_count {
            Some(0) => {}
            Some(process_count) => refusal_reasons.push(format!(
                "worker `{}` has {} resident compute process(es)",
                worker.worker_label, process_count
            )),
            None => refusal_reasons.push(format!(
                "worker `{}` did not report resident compute process telemetry",
                worker.worker_label
            )),
        }
        if worker.mig_partitioned {
            refusal_reasons.push(format!(
                "worker `{}` is MIG partitioned and therefore not admitted",
                worker.worker_label
            ));
        }
    }
    let observed_credential_names: BTreeSet<_> = observation
        .credential_sources
        .iter()
        .map(|credential| credential.source_name.as_str())
        .collect();
    for source in &topology.credential_sources {
        if !observed_credential_names.contains(source.source_name.as_str()) {
            refusal_reasons.push(format!(
                "required credential source `{}` is missing from the observation",
                source.source_name
            ));
        }
    }
    for credential in &observation.credential_sources {
        if !credential.present {
            refusal_reasons.push(format!(
                "required credential source `{}` was not present",
                credential.source_name
            ));
        }
        if credential.redacted_digest.is_none() {
            refusal_reasons.push(format!(
                "required credential source `{}` is missing a redacted digest",
                credential.source_name
            ));
        }
    }
    if !observation.checkpoint_restore_ready {
        refusal_reasons.push(String::from(
            "checkpoint restore rehearsal support is not green for this launch",
        ));
    }

    let mut qualification = PsionActualPretrainingHardwareQualification {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_HARDWARE_QUALIFICATION_SCHEMA_VERSION,
        ),
        qualification_id: format!("psion_actual_pretraining_hardware_qualification::{run_id}"),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        selected_git_ref: String::from(selected_git_ref),
        git_commit_sha: String::from(git_commit_sha),
        dirty_tree_admission: String::from(dirty_tree_admission),
        observation_kind: observation.observation_kind.clone(),
        observation_artifact,
        topology_storage_bundle: topology_storage_bundle_ref,
        systems_bundle: systems_bundle_ref,
        evidence_contract: evidence_contract_ref,
        required_backend: topology.required_backend.clone(),
        required_worker_count: topology.required_worker_count,
        required_device_name_substring,
        required_min_free_memory_bytes_per_worker: systems_bundle
            .memory_qualification
            .min_per_worker_free_memory_bytes,
        required_credential_sources: topology
            .credential_sources
            .iter()
            .map(|source| source.source_name.clone())
            .collect(),
        required_recovery_event_ids: systems_bundle
            .resume_rehearsal_support
            .required_recovery_event_ids
            .clone(),
        observed_at_utc: observation.observed_at_utc.clone(),
        observed_workers: observation.workers.clone(),
        credential_checks: observation.credential_sources.clone(),
        checkpoint_restore_ready: observation.checkpoint_restore_ready,
        admission_state: String::from(if refusal_reasons.is_empty() {
            "admitted"
        } else {
            "refused"
        }),
        refusal_reasons,
        claim_boundary: format!(
            "This hardware qualification binds actual-lane launch admission to one retained preflight receipt under `{}`. It proves backend, worker inventory, free-memory, temperature, ECC, throttling, credential, and checkpoint-restore posture only; it does not claim durable backup, automatic eval triggering, or live run health.",
            evidence_contract.evidence_family
        ),
        detail: String::from(
            "Actual-lane launch now consumes a retained hardware qualification receipt before non-dry-run start or resume can stage manifests.",
        ),
        receipt_digest: String::new(),
    };
    qualification.receipt_digest = stable_hardware_qualification_digest(&qualification)?;
    qualification.validate()?;
    Ok(qualification)
}

fn validate_workers(
    workers: &[PsionActualPretrainingObservedWorker],
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingHardwareQualificationError> {
    let mut labels = BTreeSet::new();
    for worker in workers {
        worker.validate(field_prefix)?;
        if !labels.insert(worker.worker_label.as_str()) {
            return Err(PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                field: format!("{field_prefix}[].worker_label"),
                detail: format!("duplicate worker label `{}`", worker.worker_label),
            });
        }
    }
    Ok(())
}

fn validate_credentials(
    credentials: &[PsionActualPretrainingObservedCredentialSource],
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingHardwareQualificationError> {
    let mut names = BTreeSet::new();
    for credential in credentials {
        credential.validate(field_prefix)?;
        if !names.insert(credential.source_name.as_str()) {
            return Err(PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                field: format!("{field_prefix}[].source_name"),
                detail: format!(
                    "duplicate credential source `{}`",
                    credential.source_name
                ),
            });
        }
    }
    Ok(())
}

impl PsionActualPretrainingObservedWorker {
    fn validate(
        &self,
        field_prefix: &str,
    ) -> Result<(), PsionActualPretrainingHardwareQualificationError> {
        for (field, value) in [
            (format!("{field_prefix}[].worker_label"), self.worker_label.as_str()),
            (format!("{field_prefix}[].backend"), self.backend.as_str()),
            (format!("{field_prefix}[].device_name"), self.device_name.as_str()),
            (format!("{field_prefix}[].detail"), self.detail.as_str()),
        ] {
            ensure_nonempty(value, field.as_str())?;
        }
        if self.total_memory_bytes == 0 || self.free_memory_bytes == 0 {
            return Err(PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                field: format!("{field_prefix}[].memory_bytes"),
                detail: String::from("observed worker memory values must stay positive"),
            });
        }
        if self.free_memory_bytes > self.total_memory_bytes {
            return Err(PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                field: format!("{field_prefix}[].free_memory_bytes"),
                detail: String::from("free memory must not exceed total memory"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingObservedCredentialSource {
    fn validate(
        &self,
        field_prefix: &str,
    ) -> Result<(), PsionActualPretrainingHardwareQualificationError> {
        ensure_nonempty(
            self.source_name.as_str(),
            &format!("{field_prefix}[].source_name"),
        )?;
        ensure_nonempty(self.kind.as_str(), &format!("{field_prefix}[].kind"))?;
        ensure_nonempty(self.detail.as_str(), &format!("{field_prefix}[].detail"))?;
        Ok(())
    }
}

pub fn stable_hardware_observation_digest(
    observation: &PsionActualPretrainingHardwareObservation,
) -> Result<String, PsionActualPretrainingHardwareQualificationError> {
    let mut copy = observation.clone();
    copy.observation_digest.clear();
    Ok(stable_digest(&copy))
}

pub fn stable_hardware_qualification_digest(
    qualification: &PsionActualPretrainingHardwareQualification,
) -> Result<String, PsionActualPretrainingHardwareQualificationError> {
    let mut copy = qualification.clone();
    copy.receipt_digest.clear();
    Ok(stable_digest(&copy))
}

fn stable_digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(value)
                .expect("hardware qualification values must serialize deterministically"),
        )
    )
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingHardwareQualificationError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field_prefix}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field_prefix}.sha256"))?;
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingHardwareQualificationError> {
    if actual != expected {
        return Err(PsionActualPretrainingHardwareQualificationError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingHardwareQualificationError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingHardwareQualificationError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_unique_nonempty_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionActualPretrainingHardwareQualificationError> {
    let mut seen = BTreeSet::new();
    for value in values {
        ensure_nonempty(value.as_str(), field)?;
        if !seen.insert(value.as_str()) {
            return Err(PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                field: String::from(field),
                detail: format!("duplicate value `{value}`"),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingHardwareQualificationError {
    #[error("psion actual-pretraining hardware field `{field}` must not be empty")]
    MissingField { field: String },
    #[error(
        "psion actual-pretraining hardware field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("psion actual-pretraining hardware field `{field}` has unsupported value: {detail}")]
    UnsupportedValue { field: String, detail: String },
    #[error("psion actual-pretraining hardware digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error(
        "psion actual-pretraining hardware qualification depends on invalid `{surface}`: {detail}"
    )]
    UpstreamValidation { surface: String, detail: String },
}

#[cfg(test)]
mod tests {
    use super::{
        PsionActualPretrainingHardwareObservation, PsionActualPretrainingHardwareQualification,
    };

    fn observation() -> PsionActualPretrainingHardwareObservation {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_hardware_observation_admitted_v1.json"
        ))
        .expect("hardware observation fixture should parse")
    }

    fn qualification() -> PsionActualPretrainingHardwareQualification {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_hardware_qualification_v1.json"
        ))
        .expect("hardware qualification fixture should parse")
    }

    #[test]
    fn actual_pretraining_hardware_observation_fixture_validates() {
        observation()
            .validate()
            .expect("hardware observation fixture should validate");
    }

    #[test]
    fn actual_pretraining_hardware_qualification_fixture_validates() {
        qualification()
            .validate()
            .expect("hardware qualification fixture should validate");
    }

    #[test]
    fn hardware_qualification_rejects_admitted_receipt_with_refusal_reason() {
        let mut qualification = qualification();
        qualification
            .refusal_reasons
            .push(String::from("unexpected refusal"));
        let error = qualification
            .validate()
            .expect_err("admitted receipt with refusal reasons must fail");
        assert_eq!(
            error,
            super::PsionActualPretrainingHardwareQualificationError::UnsupportedValue {
                field: String::from("hardware_qualification.refusal_reasons"),
                detail: String::from(
                    "admitted hardware qualification must not retain refusal reasons"
                ),
            }
        );
    }
}
