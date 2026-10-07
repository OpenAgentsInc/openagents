use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionActualPretrainingArtifactRef, PSION_ACTUAL_PRETRAINING_LANE_ID,
    PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID,
};

/// Stable schema version for the canonical actual-lane systems bundle.
pub const PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_systems_bundle.v1";

/// Stable systems bundle identifier for the actual pretraining lane.
pub const PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_ID: &str =
    "psion_actual_pretraining_systems_bundle_v1";

/// Canonical fixture path for the actual-lane systems bundle.
pub const PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json";

/// Canonical focused doc path for the actual-lane systems bundle.
pub const PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_DOC_PATH: &str =
    "docs/PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE.md";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingSystemsThroughputBaseline {
    pub baseline_id: String,
    pub baseline_kind: String,
    pub source_profile: String,
    pub worker_count: u64,
    pub mean_tokens_per_second: u64,
    pub peak_tokens_per_second: u64,
    pub mean_step_latency_ms: u64,
    pub checkpoint_write_throughput_bytes_per_second: u64,
    pub source_receipt_id: String,
    pub source_receipt_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingMemoryQualification {
    pub qualification_id: String,
    pub required_backend: String,
    pub worker_count: u64,
    pub per_worker_total_memory_bytes: u64,
    pub min_per_worker_free_memory_bytes: u64,
    pub checkpoint_total_bytes: u64,
    pub optimizer_state_bytes: u64,
    pub shard_count: u64,
    pub activation_posture: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDistributedQualification {
    pub qualification_id: String,
    pub topology_storage_bundle_id: String,
    pub supported_topology_label: String,
    pub placement_shape: String,
    pub runtime_backend: String,
    pub transport: String,
    pub collective_kind: String,
    pub collective_benchmark_digest: String,
    pub replay_receipt_id: String,
    pub replay_receipt_digest: String,
    pub exact_replay_observed: bool,
    pub data_feed_contract: String,
    pub distributed_step_receipt_id: String,
    pub distributed_step_contract_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingHardwarePreflightItem {
    pub item_id: String,
    pub category: String,
    pub required_evidence_ref: String,
    pub blocking_reason: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingSystemsBenchmarkBinding {
    pub benchmark_id: String,
    pub benchmark_family: String,
    pub source_artifact: PsionActualPretrainingArtifactRef,
    pub source_receipt_id: String,
    pub source_receipt_digest: String,
    pub required_for: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingResumeRehearsalSupport {
    pub recovery_bundle: PsionActualPretrainingArtifactRef,
    pub recovery_bundle_digest: String,
    pub required_recovery_event_ids: Vec<String>,
    pub accepted_pointer_path: String,
    pub continuation_handoff_path: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingSystemsBundle {
    pub schema_version: String,
    pub systems_bundle_id: String,
    pub lane_id: String,
    pub lane_spec: PsionActualPretrainingArtifactRef,
    pub recipe_bundle: PsionActualPretrainingArtifactRef,
    pub topology_storage_bundle: PsionActualPretrainingArtifactRef,
    pub anchor_run_bundle: PsionActualPretrainingArtifactRef,
    pub throughput_baselines: Vec<PsionActualPretrainingSystemsThroughputBaseline>,
    pub memory_qualification: PsionActualPretrainingMemoryQualification,
    pub distributed_qualification: PsionActualPretrainingDistributedQualification,
    pub hardware_preflight_items: Vec<PsionActualPretrainingHardwarePreflightItem>,
    pub benchmark_bindings: Vec<PsionActualPretrainingSystemsBenchmarkBinding>,
    pub resume_rehearsal_support: PsionActualPretrainingResumeRehearsalSupport,
    pub support_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

impl PsionActualPretrainingSystemsBundle {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingSystemsBundleError> {
        ensure_exact(
            self.schema_version.as_str(),
            "systems_bundle.schema_version",
            PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.systems_bundle_id.as_str(),
            "systems_bundle.systems_bundle_id",
            PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_ID,
        )?;
        ensure_exact(self.lane_id.as_str(), "systems_bundle.lane_id", PSION_ACTUAL_PRETRAINING_LANE_ID)?;
        ensure_artifact_ref(&self.lane_spec, "systems_bundle.lane_spec")?;
        ensure_artifact_ref(&self.recipe_bundle, "systems_bundle.recipe_bundle")?;
        ensure_artifact_ref(
            &self.topology_storage_bundle,
            "systems_bundle.topology_storage_bundle",
        )?;
        ensure_artifact_ref(&self.anchor_run_bundle, "systems_bundle.anchor_run_bundle")?;

        if self.throughput_baselines.is_empty() {
            return Err(PsionActualPretrainingSystemsBundleError::MissingField {
                field: String::from("systems_bundle.throughput_baselines"),
            });
        }
        for baseline in &self.throughput_baselines {
            baseline.validate()?;
        }
        let baseline_kinds: std::collections::BTreeSet<_> = self
            .throughput_baselines
            .iter()
            .map(|baseline| baseline.baseline_kind.as_str())
            .collect();
        if !baseline_kinds.contains("trusted_cluster_anchor") {
            return Err(PsionActualPretrainingSystemsBundleError::MissingField {
                field: String::from("systems_bundle.throughput_baselines.trusted_cluster_anchor"),
            });
        }

        self.memory_qualification.validate()?;
        self.distributed_qualification.validate()?;

        if self.hardware_preflight_items.is_empty() {
            return Err(PsionActualPretrainingSystemsBundleError::MissingField {
                field: String::from("systems_bundle.hardware_preflight_items"),
            });
        }
        for item in &self.hardware_preflight_items {
            item.validate()?;
        }
        let preflight_categories: std::collections::BTreeSet<_> = self
            .hardware_preflight_items
            .iter()
            .map(|item| item.category.as_str())
            .collect();
        for required_category in [
            "backend_family",
            "worker_inventory",
            "storage_credentials",
            "checkpoint_restore",
        ] {
            if !preflight_categories.contains(required_category) {
                return Err(PsionActualPretrainingSystemsBundleError::MissingField {
                    field: format!(
                        "systems_bundle.hardware_preflight_items[{required_category}]"
                    ),
                });
            }
        }

        if self.benchmark_bindings.is_empty() {
            return Err(PsionActualPretrainingSystemsBundleError::MissingField {
                field: String::from("systems_bundle.benchmark_bindings"),
            });
        }
        for binding in &self.benchmark_bindings {
            binding.validate()?;
        }
        let benchmark_families: std::collections::BTreeSet<_> = self
            .benchmark_bindings
            .iter()
            .map(|binding| binding.benchmark_family.as_str())
            .collect();
        for required_family in [
            "throughput_anchor",
            "collective_sync",
            "replay_exactness",
            "resume_recovery",
        ] {
            if !benchmark_families.contains(required_family) {
                return Err(PsionActualPretrainingSystemsBundleError::MissingField {
                    field: format!("systems_bundle.benchmark_bindings[{required_family}]"),
                });
            }
        }

        self.resume_rehearsal_support.validate()?;

        if self.support_refs.is_empty() {
            return Err(PsionActualPretrainingSystemsBundleError::MissingField {
                field: String::from("systems_bundle.support_refs"),
            });
        }
        for support_ref in &self.support_refs {
            ensure_nonempty(support_ref.as_str(), "systems_bundle.support_refs[]")?;
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "systems_bundle.claim_boundary",
        )?;
        ensure_nonempty(self.summary.as_str(), "systems_bundle.summary")?;
        if self.bundle_digest != stable_systems_bundle_digest(self)? {
            return Err(PsionActualPretrainingSystemsBundleError::DigestMismatch {
                field: String::from("systems_bundle.bundle_digest"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingSystemsThroughputBaseline {
    fn validate(&self) -> Result<(), PsionActualPretrainingSystemsBundleError> {
        for (field, value) in [
            ("throughput_baseline.baseline_id", self.baseline_id.as_str()),
            ("throughput_baseline.baseline_kind", self.baseline_kind.as_str()),
            ("throughput_baseline.source_profile", self.source_profile.as_str()),
            ("throughput_baseline.source_receipt_id", self.source_receipt_id.as_str()),
            (
                "throughput_baseline.source_receipt_digest",
                self.source_receipt_digest.as_str(),
            ),
            ("throughput_baseline.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        for (field, value) in [
            ("throughput_baseline.worker_count", self.worker_count),
            (
                "throughput_baseline.mean_tokens_per_second",
                self.mean_tokens_per_second,
            ),
            (
                "throughput_baseline.peak_tokens_per_second",
                self.peak_tokens_per_second,
            ),
            (
                "throughput_baseline.mean_step_latency_ms",
                self.mean_step_latency_ms,
            ),
            (
                "throughput_baseline.checkpoint_write_throughput_bytes_per_second",
                self.checkpoint_write_throughput_bytes_per_second,
            ),
        ] {
            if value == 0 {
                return Err(PsionActualPretrainingSystemsBundleError::UnsupportedValue {
                    field: String::from(field),
                    detail: String::from("systems throughput values must stay positive"),
                });
            }
        }
        Ok(())
    }
}

impl PsionActualPretrainingMemoryQualification {
    fn validate(&self) -> Result<(), PsionActualPretrainingSystemsBundleError> {
        for (field, value) in [
            (
                "memory_qualification.qualification_id",
                self.qualification_id.as_str(),
            ),
            (
                "memory_qualification.required_backend",
                self.required_backend.as_str(),
            ),
            (
                "memory_qualification.activation_posture",
                self.activation_posture.as_str(),
            ),
            ("memory_qualification.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        ensure_exact(
            self.required_backend.as_str(),
            "memory_qualification.required_backend",
            "cuda",
        )?;
        for (field, value) in [
            (
                "memory_qualification.worker_count",
                self.worker_count,
            ),
            (
                "memory_qualification.per_worker_total_memory_bytes",
                self.per_worker_total_memory_bytes,
            ),
            (
                "memory_qualification.min_per_worker_free_memory_bytes",
                self.min_per_worker_free_memory_bytes,
            ),
            (
                "memory_qualification.checkpoint_total_bytes",
                self.checkpoint_total_bytes,
            ),
            (
                "memory_qualification.optimizer_state_bytes",
                self.optimizer_state_bytes,
            ),
            ("memory_qualification.shard_count", self.shard_count),
        ] {
            if value == 0 {
                return Err(PsionActualPretrainingSystemsBundleError::UnsupportedValue {
                    field: String::from(field),
                    detail: String::from("memory-qualification values must stay positive"),
                });
            }
        }
        Ok(())
    }
}

impl PsionActualPretrainingDistributedQualification {
    fn validate(&self) -> Result<(), PsionActualPretrainingSystemsBundleError> {
        for (field, value) in [
            (
                "distributed_qualification.qualification_id",
                self.qualification_id.as_str(),
            ),
            (
                "distributed_qualification.topology_storage_bundle_id",
                self.topology_storage_bundle_id.as_str(),
            ),
            (
                "distributed_qualification.supported_topology_label",
                self.supported_topology_label.as_str(),
            ),
            (
                "distributed_qualification.placement_shape",
                self.placement_shape.as_str(),
            ),
            (
                "distributed_qualification.runtime_backend",
                self.runtime_backend.as_str(),
            ),
            (
                "distributed_qualification.transport",
                self.transport.as_str(),
            ),
            (
                "distributed_qualification.collective_kind",
                self.collective_kind.as_str(),
            ),
            (
                "distributed_qualification.collective_benchmark_digest",
                self.collective_benchmark_digest.as_str(),
            ),
            (
                "distributed_qualification.replay_receipt_id",
                self.replay_receipt_id.as_str(),
            ),
            (
                "distributed_qualification.replay_receipt_digest",
                self.replay_receipt_digest.as_str(),
            ),
            (
                "distributed_qualification.data_feed_contract",
                self.data_feed_contract.as_str(),
            ),
            (
                "distributed_qualification.distributed_step_receipt_id",
                self.distributed_step_receipt_id.as_str(),
            ),
            (
                "distributed_qualification.distributed_step_contract_digest",
                self.distributed_step_contract_digest.as_str(),
            ),
            ("distributed_qualification.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        ensure_exact(
            self.topology_storage_bundle_id.as_str(),
            "distributed_qualification.topology_storage_bundle_id",
            PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID,
        )?;
        ensure_exact(
            self.runtime_backend.as_str(),
            "distributed_qualification.runtime_backend",
            "cuda",
        )?;
        if !self.exact_replay_observed {
            return Err(PsionActualPretrainingSystemsBundleError::UnsupportedValue {
                field: String::from("distributed_qualification.exact_replay_observed"),
                detail: String::from(
                    "actual-lane distributed qualification must retain exact replay",
                ),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingHardwarePreflightItem {
    fn validate(&self) -> Result<(), PsionActualPretrainingSystemsBundleError> {
        for (field, value) in [
            ("hardware_preflight.item_id", self.item_id.as_str()),
            ("hardware_preflight.category", self.category.as_str()),
            (
                "hardware_preflight.required_evidence_ref",
                self.required_evidence_ref.as_str(),
            ),
            (
                "hardware_preflight.blocking_reason",
                self.blocking_reason.as_str(),
            ),
            ("hardware_preflight.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        Ok(())
    }
}

impl PsionActualPretrainingSystemsBenchmarkBinding {
    fn validate(&self) -> Result<(), PsionActualPretrainingSystemsBundleError> {
        for (field, value) in [
            ("benchmark_binding.benchmark_id", self.benchmark_id.as_str()),
            (
                "benchmark_binding.benchmark_family",
                self.benchmark_family.as_str(),
            ),
            (
                "benchmark_binding.source_receipt_id",
                self.source_receipt_id.as_str(),
            ),
            (
                "benchmark_binding.source_receipt_digest",
                self.source_receipt_digest.as_str(),
            ),
            (
                "benchmark_binding.required_for",
                self.required_for.as_str(),
            ),
            ("benchmark_binding.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        ensure_artifact_ref(&self.source_artifact, "benchmark_binding.source_artifact")?;
        Ok(())
    }
}

impl PsionActualPretrainingResumeRehearsalSupport {
    fn validate(&self) -> Result<(), PsionActualPretrainingSystemsBundleError> {
        ensure_artifact_ref(
            &self.recovery_bundle,
            "resume_rehearsal_support.recovery_bundle",
        )?;
        ensure_nonempty(
            self.recovery_bundle_digest.as_str(),
            "resume_rehearsal_support.recovery_bundle_digest",
        )?;
        if self.required_recovery_event_ids.is_empty() {
            return Err(PsionActualPretrainingSystemsBundleError::MissingField {
                field: String::from("resume_rehearsal_support.required_recovery_event_ids"),
            });
        }
        for event_id in &self.required_recovery_event_ids {
            ensure_nonempty(
                event_id.as_str(),
                "resume_rehearsal_support.required_recovery_event_ids[]",
            )?;
        }
        let event_ids: std::collections::BTreeSet<_> = self
            .required_recovery_event_ids
            .iter()
            .map(String::as_str)
            .collect();
        for required_event in [
            "psion-trusted-cluster-distributed-restart-v1",
            "psion-trusted-cluster-corruption-rollback-v1",
            "psion-trusted-cluster-corruption-invalidation-v1",
        ] {
            if !event_ids.contains(required_event) {
                return Err(PsionActualPretrainingSystemsBundleError::MissingField {
                    field: format!(
                        "resume_rehearsal_support.required_recovery_event_ids[{required_event}]"
                    ),
                });
            }
        }
        ensure_exact(
            self.accepted_pointer_path.as_str(),
            "resume_rehearsal_support.accepted_pointer_path",
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        )?;
        ensure_exact(
            self.continuation_handoff_path.as_str(),
            "resume_rehearsal_support.continuation_handoff_path",
            "continuation/accepted_checkpoint_handoff.json",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "resume_rehearsal_support.detail",
        )?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionActualPretrainingSystemsBundleError {
    #[error("psion actual-pretraining systems bundle field `{field}` must not be empty")]
    MissingField { field: String },
    #[error("psion actual-pretraining systems bundle field `{field}` expected `{expected}` but found `{actual}`")]
    Mismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("psion actual-pretraining systems bundle field `{field}` is unsupported: {detail}")]
    UnsupportedValue { field: String, detail: String },
    #[error("psion actual-pretraining systems bundle digest drifted for `{field}`")]
    DigestMismatch { field: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingSystemsBundleError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingSystemsBundleError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingSystemsBundleError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionActualPretrainingSystemsBundleError::Mismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field: &str,
) -> Result<(), PsionActualPretrainingSystemsBundleError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field}.sha256"))?;
    Ok(())
}

fn stable_systems_bundle_digest(
    bundle: &PsionActualPretrainingSystemsBundle,
) -> Result<String, PsionActualPretrainingSystemsBundleError> {
    let mut copy = bundle.clone();
    copy.bundle_digest.clear();
    let canonical = serde_json::to_vec(&copy)?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
}

#[cfg(test)]
mod tests {
    use super::PsionActualPretrainingSystemsBundle;

    fn fixture() -> PsionActualPretrainingSystemsBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json"
        ))
        .expect("actual pretraining systems bundle fixture should parse")
    }

    #[test]
    fn actual_pretraining_systems_bundle_fixture_validates() {
        fixture()
            .validate()
            .expect("actual pretraining systems bundle fixture should validate");
    }

    #[test]
    fn actual_pretraining_systems_bundle_requires_resume_recovery_binding() {
        let mut bundle = fixture();
        bundle
            .resume_rehearsal_support
            .required_recovery_event_ids
            .retain(|event_id| event_id != "psion-trusted-cluster-distributed-restart-v1");
        let error = bundle
            .validate()
            .expect_err("missing resume recovery binding should be rejected");
        assert!(
            matches!(
                error,
                super::PsionActualPretrainingSystemsBundleError::MissingField { ref field }
                if field
                    == "resume_rehearsal_support.required_recovery_event_ids[psion-trusted-cluster-distributed-restart-v1]"
            ),
            "unexpected error: {error}"
        );
    }
}
