use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionActualPretrainingArtifactRef, PsionActualPretrainingBaselineToolsBundle,
    PsionActualPretrainingDataBundle, PsionActualPretrainingEvidenceContract,
    PsionActualPretrainingSystemsBundle, PSION_ACTUAL_PRETRAINING_LANE_ID,
};

/// Stable schema version for the actual-lane run-shape observation input.
pub const PSION_ACTUAL_PRETRAINING_RUN_SHAPE_OBSERVATION_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_run_shape_observation.v1";

/// Stable schema version for the actual-lane run-shape qualification receipt.
pub const PSION_ACTUAL_PRETRAINING_RUN_SHAPE_QUALIFICATION_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_run_shape_qualification.v1";

/// Canonical fixture path for the admitted run-shape observation fixture.
pub const PSION_ACTUAL_PRETRAINING_RUN_SHAPE_OBSERVATION_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_run_shape_observation_admitted_v1.json";

/// Canonical fixture path for the canonical run-shape qualification fixture.
pub const PSION_ACTUAL_PRETRAINING_RUN_SHAPE_QUALIFICATION_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_run_shape_qualification_v1.json";

/// Canonical focused doc path for actual-lane run-shape qualification.
pub const PSION_ACTUAL_PRETRAINING_RUN_SHAPE_QUALIFICATION_DOC_PATH: &str =
    "docs/PSION_ACTUAL_PRETRAINING_RUN_SHAPE_QUALIFICATION.md";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingThroughputProbe {
    pub source_receipt_id: String,
    pub source_receipt_digest: String,
    pub observed_tokens_per_second: u64,
    pub observed_step_latency_ms: u64,
    pub observed_checkpoint_write_throughput_bytes_per_second: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingStorageProbe {
    pub storage_path: String,
    pub available_bytes: u64,
    pub observed_read_bytes_per_second: u64,
    pub observed_write_bytes_per_second: u64,
    pub writable: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDataloaderProbe {
    pub dataset_identity: String,
    pub max_sequence_tokens: u64,
    pub planned_optimizer_steps: u64,
    pub planned_tokens_per_step: u64,
    pub observed_horizon_steps: u64,
    pub observed_horizon_tokens: u64,
    pub observed_batches_per_second: u64,
    pub observed_stall_count: u64,
    pub deterministic_replay_observed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingRunShapeObservation {
    pub schema_version: String,
    pub observation_id: String,
    pub lane_id: String,
    pub observation_kind: String,
    pub observed_at_utc: String,
    pub observed_run_root: String,
    pub throughput_probe: PsionActualPretrainingThroughputProbe,
    pub storage_probe: PsionActualPretrainingStorageProbe,
    pub dataloader_probe: PsionActualPretrainingDataloaderProbe,
    pub summary: String,
    pub observation_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingRunShapeQualification {
    pub schema_version: String,
    pub qualification_id: String,
    pub lane_id: String,
    pub run_id: String,
    pub selected_git_ref: String,
    pub git_commit_sha: String,
    pub dirty_tree_admission: String,
    pub observation_kind: String,
    pub observation_artifact: Option<PsionActualPretrainingArtifactRef>,
    pub baseline_tools_bundle: PsionActualPretrainingArtifactRef,
    pub data_bundle: PsionActualPretrainingArtifactRef,
    pub systems_bundle: PsionActualPretrainingArtifactRef,
    pub evidence_contract: PsionActualPretrainingArtifactRef,
    pub required_dataset_identity: String,
    pub required_horizon_steps: u64,
    pub required_tokens_per_step: u64,
    pub required_max_sequence_tokens: u64,
    pub min_healthy_tokens_per_second: u64,
    pub max_healthy_step_latency_ms: u64,
    pub min_checkpoint_write_throughput_bytes_per_second: u64,
    pub min_storage_available_bytes: u64,
    pub max_dataloader_stall_count: u64,
    pub observed_at_utc: String,
    pub observed_run_root: String,
    pub throughput_probe: PsionActualPretrainingThroughputProbe,
    pub storage_probe: PsionActualPretrainingStorageProbe,
    pub dataloader_probe: PsionActualPretrainingDataloaderProbe,
    pub admission_state: String,
    pub refusal_reasons: Vec<String>,
    pub claim_boundary: String,
    pub detail: String,
    pub receipt_digest: String,
}

impl PsionActualPretrainingRunShapeObservation {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingRunShapeQualificationError> {
        ensure_exact(
            self.schema_version.as_str(),
            "run_shape_observation.schema_version",
            PSION_ACTUAL_PRETRAINING_RUN_SHAPE_OBSERVATION_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "run_shape_observation.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_nonempty(
            self.observation_id.as_str(),
            "run_shape_observation.observation_id",
        )?;
        ensure_nonempty(
            self.observation_kind.as_str(),
            "run_shape_observation.observation_kind",
        )?;
        ensure_nonempty(
            self.observed_at_utc.as_str(),
            "run_shape_observation.observed_at_utc",
        )?;
        ensure_nonempty(
            self.observed_run_root.as_str(),
            "run_shape_observation.observed_run_root",
        )?;
        self.throughput_probe
            .validate("run_shape_observation.throughput_probe")?;
        self.storage_probe
            .validate("run_shape_observation.storage_probe")?;
        self.dataloader_probe
            .validate("run_shape_observation.dataloader_probe")?;
        ensure_nonempty(self.summary.as_str(), "run_shape_observation.summary")?;
        if self.observation_digest != stable_run_shape_observation_digest(self)? {
            return Err(
                PsionActualPretrainingRunShapeQualificationError::DigestMismatch {
                    field: String::from("run_shape_observation.observation_digest"),
                },
            );
        }
        Ok(())
    }
}

impl PsionActualPretrainingRunShapeQualification {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingRunShapeQualificationError> {
        ensure_exact(
            self.schema_version.as_str(),
            "run_shape_qualification.schema_version",
            PSION_ACTUAL_PRETRAINING_RUN_SHAPE_QUALIFICATION_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "run_shape_qualification.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        for (field, value) in [
            (
                "run_shape_qualification.qualification_id",
                self.qualification_id.as_str(),
            ),
            ("run_shape_qualification.run_id", self.run_id.as_str()),
            (
                "run_shape_qualification.selected_git_ref",
                self.selected_git_ref.as_str(),
            ),
            (
                "run_shape_qualification.git_commit_sha",
                self.git_commit_sha.as_str(),
            ),
            (
                "run_shape_qualification.dirty_tree_admission",
                self.dirty_tree_admission.as_str(),
            ),
            (
                "run_shape_qualification.observation_kind",
                self.observation_kind.as_str(),
            ),
            (
                "run_shape_qualification.required_dataset_identity",
                self.required_dataset_identity.as_str(),
            ),
            (
                "run_shape_qualification.observed_at_utc",
                self.observed_at_utc.as_str(),
            ),
            (
                "run_shape_qualification.observed_run_root",
                self.observed_run_root.as_str(),
            ),
            (
                "run_shape_qualification.admission_state",
                self.admission_state.as_str(),
            ),
            (
                "run_shape_qualification.claim_boundary",
                self.claim_boundary.as_str(),
            ),
            ("run_shape_qualification.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        for (field, value) in [
            (
                "run_shape_qualification.required_horizon_steps",
                self.required_horizon_steps,
            ),
            (
                "run_shape_qualification.required_tokens_per_step",
                self.required_tokens_per_step,
            ),
            (
                "run_shape_qualification.required_max_sequence_tokens",
                self.required_max_sequence_tokens,
            ),
            (
                "run_shape_qualification.min_healthy_tokens_per_second",
                self.min_healthy_tokens_per_second,
            ),
            (
                "run_shape_qualification.max_healthy_step_latency_ms",
                self.max_healthy_step_latency_ms,
            ),
            (
                "run_shape_qualification.min_checkpoint_write_throughput_bytes_per_second",
                self.min_checkpoint_write_throughput_bytes_per_second,
            ),
            (
                "run_shape_qualification.min_storage_available_bytes",
                self.min_storage_available_bytes,
            ),
        ] {
            if value == 0 {
                return Err(
                    PsionActualPretrainingRunShapeQualificationError::UnsupportedValue {
                        field: String::from(field),
                        detail: String::from("run-shape qualification values must stay positive"),
                    },
                );
            }
        }
        if let Some(observation_artifact) = &self.observation_artifact {
            ensure_artifact_ref(
                observation_artifact,
                "run_shape_qualification.observation_artifact",
            )?;
        }
        ensure_artifact_ref(
            &self.baseline_tools_bundle,
            "run_shape_qualification.baseline_tools_bundle",
        )?;
        ensure_artifact_ref(&self.data_bundle, "run_shape_qualification.data_bundle")?;
        ensure_artifact_ref(
            &self.systems_bundle,
            "run_shape_qualification.systems_bundle",
        )?;
        ensure_artifact_ref(
            &self.evidence_contract,
            "run_shape_qualification.evidence_contract",
        )?;
        self.throughput_probe
            .validate("run_shape_qualification.throughput_probe")?;
        self.storage_probe
            .validate("run_shape_qualification.storage_probe")?;
        self.dataloader_probe
            .validate("run_shape_qualification.dataloader_probe")?;
        match self.admission_state.as_str() {
            "admitted" => {
                if !self.refusal_reasons.is_empty() {
                    return Err(
                        PsionActualPretrainingRunShapeQualificationError::UnsupportedValue {
                            field: String::from("run_shape_qualification.refusal_reasons"),
                            detail: String::from(
                                "admitted run-shape qualification must not retain refusal reasons",
                            ),
                        },
                    );
                }
            }
            "refused" => {
                if self.refusal_reasons.is_empty() {
                    return Err(
                        PsionActualPretrainingRunShapeQualificationError::MissingField {
                            field: String::from("run_shape_qualification.refusal_reasons"),
                        },
                    );
                }
            }
            _ => {
                return Err(
                    PsionActualPretrainingRunShapeQualificationError::UnsupportedValue {
                        field: String::from("run_shape_qualification.admission_state"),
                        detail: String::from(
                            "run-shape qualification admission state must be admitted or refused",
                        ),
                    },
                );
            }
        }
        ensure_unique_nonempty_strings(
            self.refusal_reasons.as_slice(),
            "run_shape_qualification.refusal_reasons[]",
        )?;
        if self.receipt_digest != stable_run_shape_qualification_digest(self)? {
            return Err(
                PsionActualPretrainingRunShapeQualificationError::DigestMismatch {
                    field: String::from("run_shape_qualification.receipt_digest"),
                },
            );
        }
        Ok(())
    }
}

impl PsionActualPretrainingThroughputProbe {
    fn validate(
        &self,
        field_prefix: &str,
    ) -> Result<(), PsionActualPretrainingRunShapeQualificationError> {
        for (field_suffix, value) in [
            ("source_receipt_id", self.source_receipt_id.as_str()),
            ("source_receipt_digest", self.source_receipt_digest.as_str()),
            ("detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, &format!("{field_prefix}.{field_suffix}"))?;
        }
        Ok(())
    }
}

impl PsionActualPretrainingStorageProbe {
    fn validate(
        &self,
        field_prefix: &str,
    ) -> Result<(), PsionActualPretrainingRunShapeQualificationError> {
        ensure_nonempty(
            self.storage_path.as_str(),
            &format!("{field_prefix}.storage_path"),
        )?;
        ensure_nonempty(self.detail.as_str(), &format!("{field_prefix}.detail"))?;
        Ok(())
    }
}

impl PsionActualPretrainingDataloaderProbe {
    fn validate(
        &self,
        field_prefix: &str,
    ) -> Result<(), PsionActualPretrainingRunShapeQualificationError> {
        for (field_suffix, value) in [
            ("dataset_identity", self.dataset_identity.as_str()),
            ("detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, &format!("{field_prefix}.{field_suffix}"))?;
        }
        for (field_suffix, value) in [
            ("max_sequence_tokens", self.max_sequence_tokens),
            ("planned_optimizer_steps", self.planned_optimizer_steps),
            ("planned_tokens_per_step", self.planned_tokens_per_step),
        ] {
            if value == 0 {
                return Err(
                    PsionActualPretrainingRunShapeQualificationError::UnsupportedValue {
                        field: format!("{field_prefix}.{field_suffix}"),
                        detail: String::from(
                            "dataloader plan values must stay positive in run-shape observation",
                        ),
                    },
                );
            }
        }
        Ok(())
    }
}

pub fn stable_run_shape_observation_digest(
    observation: &PsionActualPretrainingRunShapeObservation,
) -> Result<String, PsionActualPretrainingRunShapeQualificationError> {
    let mut copy = observation.clone();
    copy.observation_digest.clear();
    stable_digest(b"psion_actual_pretraining_run_shape_observation|", &copy)
}

pub fn stable_run_shape_qualification_digest(
    qualification: &PsionActualPretrainingRunShapeQualification,
) -> Result<String, PsionActualPretrainingRunShapeQualificationError> {
    let mut copy = qualification.clone();
    copy.receipt_digest.clear();
    stable_digest(b"psion_actual_pretraining_run_shape_qualification|", &copy)
}

#[allow(clippy::too_many_arguments)]
pub fn derive_psion_actual_pretraining_run_shape_qualification(
    run_id: &str,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    observation: &PsionActualPretrainingRunShapeObservation,
    observation_artifact: Option<PsionActualPretrainingArtifactRef>,
    baseline_tools_bundle: PsionActualPretrainingArtifactRef,
    data_bundle: PsionActualPretrainingArtifactRef,
    systems_bundle: PsionActualPretrainingArtifactRef,
    evidence_contract: PsionActualPretrainingArtifactRef,
    baseline_tools_contract: &PsionActualPretrainingBaselineToolsBundle,
    data_bundle_contract: &PsionActualPretrainingDataBundle,
    systems_bundle_contract: &PsionActualPretrainingSystemsBundle,
    evidence_contract_bundle: &PsionActualPretrainingEvidenceContract,
) -> Result<
    PsionActualPretrainingRunShapeQualification,
    PsionActualPretrainingRunShapeQualificationError,
> {
    observation.validate()?;
    let actual_lane_accounting = baseline_tools_contract
        .resource_accounting_rows
        .iter()
        .find(|row| row.scope_kind == "actual_lane")
        .ok_or_else(
            || PsionActualPretrainingRunShapeQualificationError::MissingField {
                field: String::from("baseline_tools_bundle.resource_accounting_rows[actual_lane]"),
            },
        )?;
    let throughput_anchor = systems_bundle_contract
        .throughput_baselines
        .iter()
        .find(|baseline| baseline.baseline_kind == "trusted_cluster_anchor")
        .ok_or_else(
            || PsionActualPretrainingRunShapeQualificationError::MissingField {
                field: String::from("systems_bundle.throughput_baselines[trusted_cluster_anchor]"),
            },
        )?;

    let required_dataset_identity = data_bundle_contract
        .replay_authority
        .dataset_identity
        .clone();
    let required_horizon_steps = actual_lane_accounting.optimizer_steps;
    let required_tokens_per_step = actual_lane_accounting.tokens_per_step;
    let required_max_sequence_tokens = data_bundle_contract.replay_authority.max_sequence_tokens;
    let min_healthy_tokens_per_second =
        throughput_anchor.mean_tokens_per_second.saturating_mul(85) / 100;
    let max_healthy_step_latency_ms =
        throughput_anchor.mean_step_latency_ms.saturating_mul(125) / 100;
    let min_checkpoint_write_throughput_bytes_per_second = throughput_anchor
        .checkpoint_write_throughput_bytes_per_second
        .saturating_mul(80)
        / 100;
    let min_storage_available_bytes = systems_bundle_contract
        .memory_qualification
        .checkpoint_total_bytes
        .saturating_mul(8);
    let max_dataloader_stall_count = 3;
    let required_horizon_tokens = required_horizon_steps.saturating_mul(required_tokens_per_step);

    let mut refusal_reasons = Vec::new();
    if observation.throughput_probe.observed_tokens_per_second < min_healthy_tokens_per_second {
        refusal_reasons.push(format!(
            "observed tokens/sec {} is below required floor {}",
            observation.throughput_probe.observed_tokens_per_second, min_healthy_tokens_per_second
        ));
    }
    if observation.throughput_probe.observed_step_latency_ms > max_healthy_step_latency_ms {
        refusal_reasons.push(format!(
            "observed step latency {}ms exceeds required ceiling {}ms",
            observation.throughput_probe.observed_step_latency_ms, max_healthy_step_latency_ms
        ));
    }
    if observation
        .throughput_probe
        .observed_checkpoint_write_throughput_bytes_per_second
        < min_checkpoint_write_throughput_bytes_per_second
    {
        refusal_reasons.push(format!(
            "observed checkpoint write throughput {} is below required floor {}",
            observation
                .throughput_probe
                .observed_checkpoint_write_throughput_bytes_per_second,
            min_checkpoint_write_throughput_bytes_per_second
        ));
    }
    if !observation.storage_probe.writable {
        refusal_reasons.push(String::from(
            "storage probe reports the run root is not writable",
        ));
    }
    if observation.storage_probe.available_bytes < min_storage_available_bytes {
        refusal_reasons.push(format!(
            "available storage {} is below required floor {}",
            observation.storage_probe.available_bytes, min_storage_available_bytes
        ));
    }
    if observation.dataloader_probe.dataset_identity != required_dataset_identity {
        refusal_reasons.push(format!(
            "dataloader dataset identity `{}` does not match required `{}`",
            observation.dataloader_probe.dataset_identity, required_dataset_identity
        ));
    }
    if observation.dataloader_probe.max_sequence_tokens != required_max_sequence_tokens {
        refusal_reasons.push(format!(
            "dataloader max sequence tokens {} does not match required {}",
            observation.dataloader_probe.max_sequence_tokens, required_max_sequence_tokens
        ));
    }
    if !observation.dataloader_probe.deterministic_replay_observed {
        refusal_reasons.push(String::from(
            "dataloader probe did not retain deterministic replay truth",
        ));
    }
    if observation.dataloader_probe.observed_horizon_steps < required_horizon_steps {
        refusal_reasons.push(format!(
            "dataloader observed horizon steps {} is below required {}",
            observation.dataloader_probe.observed_horizon_steps, required_horizon_steps
        ));
    }
    if observation.dataloader_probe.observed_horizon_tokens < required_horizon_tokens {
        refusal_reasons.push(format!(
            "dataloader observed horizon tokens {} is below required {}",
            observation.dataloader_probe.observed_horizon_tokens, required_horizon_tokens
        ));
    }
    if observation.dataloader_probe.observed_stall_count > max_dataloader_stall_count {
        refusal_reasons.push(format!(
            "dataloader observed stall count {} exceeds admitted ceiling {}",
            observation.dataloader_probe.observed_stall_count, max_dataloader_stall_count
        ));
    }

    let mut qualification = PsionActualPretrainingRunShapeQualification {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_RUN_SHAPE_QUALIFICATION_SCHEMA_VERSION,
        ),
        qualification_id: format!("psion_actual_pretraining_run_shape_qualification::{run_id}"),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        selected_git_ref: String::from(selected_git_ref),
        git_commit_sha: String::from(git_commit_sha),
        dirty_tree_admission: String::from(dirty_tree_admission),
        observation_kind: observation.observation_kind.clone(),
        observation_artifact,
        baseline_tools_bundle,
        data_bundle,
        systems_bundle,
        evidence_contract,
        required_dataset_identity,
        required_horizon_steps,
        required_tokens_per_step,
        required_max_sequence_tokens,
        min_healthy_tokens_per_second,
        max_healthy_step_latency_ms,
        min_checkpoint_write_throughput_bytes_per_second,
        min_storage_available_bytes,
        max_dataloader_stall_count,
        observed_at_utc: observation.observed_at_utc.clone(),
        observed_run_root: observation.observed_run_root.clone(),
        throughput_probe: observation.throughput_probe.clone(),
        storage_probe: observation.storage_probe.clone(),
        dataloader_probe: observation.dataloader_probe.clone(),
        admission_state: String::from(if refusal_reasons.is_empty() {
            "admitted"
        } else {
            "refused"
        }),
        refusal_reasons,
        claim_boundary: String::from(
            "This retained run-shape qualification ties throughput, storage, and dataloader admission to the frozen actual-lane systems, data, and baseline-tools bundles. It does not claim that later backup, eval, or alert automation is already finished.",
        ),
        detail: format!(
            "Run-shape qualification consumes the frozen actual-lane throughput anchor `{}`, replay authority `{}`, and evidence family `{}` before admitting a long run.",
            throughput_anchor.baseline_id,
            data_bundle_contract.replay_authority.dataset_identity,
            evidence_contract_bundle.evidence_family
        ),
        receipt_digest: String::new(),
    };
    qualification.receipt_digest = stable_run_shape_qualification_digest(&qualification)?;
    qualification.validate()?;
    Ok(qualification)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingRunShapeQualificationError {
    #[error("psion actual-pretraining run-shape field `{field}` must not be empty")]
    MissingField { field: String },
    #[error(
        "psion actual-pretraining run-shape field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("psion actual-pretraining run-shape field `{field}` is unsupported: {detail}")]
    UnsupportedValue { field: String, detail: String },
    #[error("psion actual-pretraining run-shape digest drifted for `{field}`")]
    DigestMismatch { field: String },
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingRunShapeQualificationError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field_prefix}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field_prefix}.sha256"))?;
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingRunShapeQualificationError> {
    if value.trim().is_empty() {
        return Err(
            PsionActualPretrainingRunShapeQualificationError::MissingField {
                field: String::from(field),
            },
        );
    }
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingRunShapeQualificationError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(
            PsionActualPretrainingRunShapeQualificationError::FieldMismatch {
                field: String::from(field),
                expected: String::from(expected),
                actual: String::from(actual),
            },
        );
    }
    Ok(())
}

fn ensure_unique_nonempty_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionActualPretrainingRunShapeQualificationError> {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        ensure_nonempty(value.as_str(), field)?;
        if !seen.insert(value) {
            return Err(
                PsionActualPretrainingRunShapeQualificationError::UnsupportedValue {
                    field: String::from(field),
                    detail: format!("duplicate value `{value}`"),
                },
            );
        }
    }
    Ok(())
}

fn stable_digest<T: Serialize>(
    domain: &[u8],
    value: &T,
) -> Result<String, PsionActualPretrainingRunShapeQualificationError> {
    let canonical = serde_json::to_vec(value).map_err(|error| {
        PsionActualPretrainingRunShapeQualificationError::UnsupportedValue {
            field: String::from("serialization"),
            detail: error.to_string(),
        }
    })?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(canonical);
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::{
        PsionActualPretrainingRunShapeObservation, PsionActualPretrainingRunShapeQualification,
    };

    fn observation() -> PsionActualPretrainingRunShapeObservation {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_run_shape_observation_admitted_v1.json"
        ))
        .expect("actual pretraining run-shape observation fixture should parse")
    }

    fn qualification() -> PsionActualPretrainingRunShapeQualification {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_run_shape_qualification_v1.json"
        ))
        .expect("actual pretraining run-shape qualification fixture should parse")
    }

    #[test]
    fn actual_pretraining_run_shape_observation_fixture_validates() {
        observation()
            .validate()
            .expect("actual pretraining run-shape observation fixture should validate");
    }

    #[test]
    fn actual_pretraining_run_shape_qualification_fixture_validates() {
        qualification()
            .validate()
            .expect("actual pretraining run-shape qualification fixture should validate");
    }

    #[test]
    fn run_shape_qualification_rejects_admitted_receipt_with_refusal_reason() {
        let mut qualification = qualification();
        qualification.admission_state = String::from("admitted");
        qualification.refusal_reasons = vec![String::from("unexpected refusal reason")];
        let error = qualification
            .validate()
            .expect_err("admitted run-shape receipt should reject refusal reasons");
        assert_eq!(
            error,
            super::PsionActualPretrainingRunShapeQualificationError::UnsupportedValue {
                field: String::from("run_shape_qualification.refusal_reasons"),
                detail: String::from(
                    "admitted run-shape qualification must not retain refusal reasons",
                ),
            }
        );
    }
}
