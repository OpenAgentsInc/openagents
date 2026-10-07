use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_4080_decision_grade_run_packet, builtin_executor_4080_smoke_run_packet,
    builtin_executor_baseline_truth_record, builtin_executor_canonical_mixture_packet,
    builtin_executor_mlx_decision_grade_run_packet, PsionExecutor4080DecisionGradeRunError,
    PsionExecutor4080SmokeRunError, PsionExecutorBaselineTruthError,
    PsionExecutorCanonicalMixtureError, PsionExecutorMlxDecisionGradeRunError,
};

/// Stable schema version for the canonical local-cluster run-registration packet.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_SCHEMA_VERSION: &str =
    "psion.executor.local_cluster_run_registration.v1";
/// Canonical fixture path for the local-cluster run-registration packet.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_local_cluster_run_registration_v1.json";
/// Canonical doc path for the local-cluster run-registration packet.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION.md";

const LOCAL_MAC_MLX_PROFILE_ID: &str = "local_mac_mlx_aarch64";
const LOCAL_4080_PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const LOCAL_TAILNET_CONTROL_PROFILE_ID: &str = "local_tailnet_cluster_control_plane";
const LOCAL_MLX_COMPUTE_SOURCE_FIXTURE_PATH: &str =
    "fixtures/training/compute_sources/local_mlx_mac_workstation_v1.json";
const LOCAL_4080_COMPUTE_SOURCE_FIXTURE_PATH: &str =
    "fixtures/training/compute_sources/local_rtx4080_workstation_v1.json";
const FREQUENT_PACK_ID: &str = "tassadar.eval.frequent.v0";
const PROMOTION_PACK_ID: &str = "tassadar.eval.promotion.v0";
const MLX_REGISTRATION_ID: &str = "psion_executor_local_cluster_registration_mlx_v1";
const CUDA_REGISTRATION_ID: &str = "psion_executor_local_cluster_registration_4080_v1";
const MLX_MACHINE_CLASS_ID: &str = "host_cpu_aarch64";
const CUDA_MACHINE_CLASS_ID: &str = "local_rtx4080_workstation";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_CANONICAL_MIXTURE_V0.md";
const PSION_EXECUTOR_MLX_DECISION_GRADE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MLX_DECISION_GRADE_RUN.md";
const PSION_EXECUTOR_4080_DECISION_GRADE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_DECISION_GRADE_RUN.md";
const PSION_EXECUTOR_4080_SMOKE_RUN_DOC_PATH: &str = "docs/PSION_EXECUTOR_4080_SMOKE_RUN.md";
const PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH: &str = "docs/PSION_EXECUTOR_BASELINE_TRUTH.md";

#[derive(Debug, Error)]
pub enum PsionExecutorLocalClusterRunRegistrationError {
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
    MlxDecisionGrade(#[from] PsionExecutorMlxDecisionGradeRunError),
    #[error(transparent)]
    DecisionGrade4080(#[from] PsionExecutor4080DecisionGradeRunError),
    #[error(transparent)]
    Smoke4080(#[from] PsionExecutor4080SmokeRunError),
    #[error(transparent)]
    BaselineTruth(#[from] PsionExecutorBaselineTruthError),
    #[error(transparent)]
    CanonicalMixture(#[from] PsionExecutorCanonicalMixtureError),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorLocalClusterCandidateStatus {
    CurrentBest,
    Candidate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterBatchGeometry {
    pub observed_batch_count: u64,
    pub observed_sample_count: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterMemoryHeadroom {
    pub evidence_mode: String,
    pub admitted_memory_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_free_memory_bytes: Option<u64>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterExpectedThroughput {
    pub expected_steps_per_second: f64,
    pub expected_samples_per_second: f64,
    pub expected_source_tokens_per_second: f64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterRunRegistrationRow {
    pub registration_id: String,
    pub run_type_id: String,
    pub run_id: String,
    pub search_run_ids: Vec<String>,
    pub admitted_profile_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_plane_profile_id: Option<String>,
    pub compute_source_id: String,
    pub machine_class_id: String,
    pub execution_backend_label: String,
    pub logical_device_label: String,
    pub model_id: String,
    pub mixture_version_id: String,
    pub candidate_status: PsionExecutorLocalClusterCandidateStatus,
    pub checkpoint_family: String,
    pub eval_pack_ids: Vec<String>,
    pub wallclock_budget_seconds: u64,
    pub observed_duration_ms: u64,
    pub stop_condition: String,
    pub batch_geometry: PsionExecutorLocalClusterBatchGeometry,
    pub memory_headroom: PsionExecutorLocalClusterMemoryHeadroom,
    pub expected_throughput: PsionExecutorLocalClusterExpectedThroughput,
    pub detail: String,
    pub registration_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterRunRegistrationPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub model_id: String,
    pub registration_rows: Vec<PsionExecutorLocalClusterRunRegistrationRow>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CrossProviderComputeSourceContract {
    source_id: String,
    accelerators: CrossProviderComputeSourceAccelerators,
}

#[derive(Clone, Debug, Deserialize)]
struct CrossProviderComputeSourceAccelerators {
    per_accelerator_memory_bytes: u64,
}

impl PsionExecutorLocalClusterBatchGeometry {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterRunRegistrationError> {
        if self.observed_batch_count == 0 {
            return Err(PsionExecutorLocalClusterRunRegistrationError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_run_registration.batch_geometry.observed_batch_count",
                ),
                detail: String::from("observed batch count must stay positive"),
            });
        }
        if self.observed_sample_count == 0 {
            return Err(PsionExecutorLocalClusterRunRegistrationError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_run_registration.batch_geometry.observed_sample_count",
                ),
                detail: String::from("observed sample count must stay positive"),
            });
        }
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_local_cluster_run_registration.batch_geometry.detail",
        )
    }
}

impl PsionExecutorLocalClusterMemoryHeadroom {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterRunRegistrationError> {
        ensure_nonempty(
            self.evidence_mode.as_str(),
            "psion_executor_local_cluster_run_registration.memory_headroom.evidence_mode",
        )?;
        if self.admitted_memory_bytes == 0 {
            return Err(PsionExecutorLocalClusterRunRegistrationError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_run_registration.memory_headroom.admitted_memory_bytes",
                ),
                detail: String::from("admitted memory bytes must stay positive"),
            });
        }
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_local_cluster_run_registration.memory_headroom.detail",
        )
    }
}

impl PsionExecutorLocalClusterExpectedThroughput {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterRunRegistrationError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_run_registration.expected_throughput.expected_steps_per_second",
                self.expected_steps_per_second,
            ),
            (
                "psion_executor_local_cluster_run_registration.expected_throughput.expected_samples_per_second",
                self.expected_samples_per_second,
            ),
            (
                "psion_executor_local_cluster_run_registration.expected_throughput.expected_source_tokens_per_second",
                self.expected_source_tokens_per_second,
            ),
        ] {
            if value <= 0.0 {
                return Err(PsionExecutorLocalClusterRunRegistrationError::InvalidValue {
                    field: String::from(field),
                    detail: String::from("expected throughput must stay positive"),
                });
            }
        }
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_local_cluster_run_registration.expected_throughput.detail",
        )
    }
}

impl PsionExecutorLocalClusterRunRegistrationRow {
    pub fn validate(&self) -> Result<(), PsionExecutorLocalClusterRunRegistrationError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].registration_id",
                self.registration_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].run_type_id",
                self.run_type_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].admitted_profile_id",
                self.admitted_profile_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].compute_source_id",
                self.compute_source_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].machine_class_id",
                self.machine_class_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].execution_backend_label",
                self.execution_backend_label.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].logical_device_label",
                self.logical_device_label.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].model_id",
                self.model_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].mixture_version_id",
                self.mixture_version_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].checkpoint_family",
                self.checkpoint_family.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].stop_condition",
                self.stop_condition.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.registration_rows[].registration_digest",
                self.registration_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.search_run_ids.is_empty() {
            return Err(PsionExecutorLocalClusterRunRegistrationError::MissingField {
                field: String::from(
                    "psion_executor_local_cluster_run_registration.registration_rows[].search_run_ids",
                ),
            });
        }
        for search_run_id in &self.search_run_ids {
            ensure_nonempty(
                search_run_id.as_str(),
                "psion_executor_local_cluster_run_registration.registration_rows[].search_run_ids[]",
            )?;
        }
        if self.eval_pack_ids.is_empty() {
            return Err(PsionExecutorLocalClusterRunRegistrationError::MissingField {
                field: String::from(
                    "psion_executor_local_cluster_run_registration.registration_rows[].eval_pack_ids",
                ),
            });
        }
        for eval_pack_id in &self.eval_pack_ids {
            ensure_nonempty(
                eval_pack_id.as_str(),
                "psion_executor_local_cluster_run_registration.registration_rows[].eval_pack_ids[]",
            )?;
        }
        if self.wallclock_budget_seconds == 0 {
            return Err(PsionExecutorLocalClusterRunRegistrationError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_run_registration.registration_rows[].wallclock_budget_seconds",
                ),
                detail: String::from("wallclock budget must stay positive"),
            });
        }
        if self.observed_duration_ms == 0 {
            return Err(PsionExecutorLocalClusterRunRegistrationError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_run_registration.registration_rows[].observed_duration_ms",
                ),
                detail: String::from("observed duration must stay positive"),
            });
        }
        self.batch_geometry.validate()?;
        self.memory_headroom.validate()?;
        self.expected_throughput.validate()?;
        if stable_registration_row_digest(self) != self.registration_digest {
            return Err(PsionExecutorLocalClusterRunRegistrationError::DigestMismatch {
                field: String::from(
                    "psion_executor_local_cluster_run_registration.registration_rows[].registration_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLocalClusterRunRegistrationPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorLocalClusterRunRegistrationError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_local_cluster_run_registration.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_SCHEMA_VERSION {
            return Err(
                PsionExecutorLocalClusterRunRegistrationError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_local_cluster_run_registration.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.model_id",
                self.model_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_local_cluster_run_registration.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.registration_rows.len() < 2 {
            return Err(PsionExecutorLocalClusterRunRegistrationError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_run_registration.registration_rows",
                ),
                detail: String::from("both MLX and 4080 rows must remain registered"),
            });
        }
        for row in &self.registration_rows {
            row.validate()?;
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorLocalClusterRunRegistrationError::MissingField {
                field: String::from("psion_executor_local_cluster_run_registration.support_refs"),
            });
        }
        for support_ref in &self.support_refs {
            ensure_nonempty(
                support_ref.as_str(),
                "psion_executor_local_cluster_run_registration.support_refs[]",
            )?;
        }
        if stable_registration_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorLocalClusterRunRegistrationError::DigestMismatch {
                field: String::from("psion_executor_local_cluster_run_registration.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_local_cluster_run_registration_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterRunRegistrationPacket, PsionExecutorLocalClusterRunRegistrationError>
{
    let baseline = builtin_executor_baseline_truth_record(workspace_root)?;
    let canonical_mixture = builtin_executor_canonical_mixture_packet(workspace_root)?;
    let mlx_packet = builtin_executor_mlx_decision_grade_run_packet(workspace_root)?;
    let cuda_packet = builtin_executor_4080_decision_grade_run_packet(workspace_root)?;
    let smoke_packet = builtin_executor_4080_smoke_run_packet(workspace_root)?;
    let mlx_compute_source: CrossProviderComputeSourceContract =
        read_json(workspace_root, LOCAL_MLX_COMPUTE_SOURCE_FIXTURE_PATH)?;
    let cuda_compute_source: CrossProviderComputeSourceContract =
        read_json(workspace_root, LOCAL_4080_COMPUTE_SOURCE_FIXTURE_PATH)?;

    let mut rows = vec![
        PsionExecutorLocalClusterRunRegistrationRow {
            registration_id: String::from(MLX_REGISTRATION_ID),
            run_type_id: String::from("mlx_decision_grade"),
            run_id: mlx_packet.retained_run_id.clone(),
            search_run_ids: vec![
                mlx_packet.retained_run_id.clone(),
                String::from("tailrun-admitted-device-matrix-20260327b"),
            ],
            admitted_profile_id: String::from(LOCAL_MAC_MLX_PROFILE_ID),
            control_plane_profile_id: None,
            compute_source_id: mlx_compute_source.source_id,
            machine_class_id: String::from(MLX_MACHINE_CLASS_ID),
            execution_backend_label: mlx_packet.execution_backend_label.clone(),
            logical_device_label: mlx_packet.logical_device_label.clone(),
            model_id: baseline.model_id.clone(),
            mixture_version_id: canonical_mixture.mixture_id.clone(),
            candidate_status: PsionExecutorLocalClusterCandidateStatus::Candidate,
            checkpoint_family: mlx_packet.checkpoint_family.clone(),
            eval_pack_ids: vec![String::from(FREQUENT_PACK_ID), String::from(PROMOTION_PACK_ID)],
            wallclock_budget_seconds: 600,
            observed_duration_ms: mlx_packet.observed_wallclock_ms,
            stop_condition: String::from(
                "Consume the admitted 600 second local MLX decision-grade budget while keeping the frozen frequent and promotion packs bound to the retained same-node run.",
            ),
            batch_geometry: PsionExecutorLocalClusterBatchGeometry {
                observed_batch_count: 8,
                observed_sample_count: 128,
                detail: String::from(
                    "The retained same-node MLX decision-grade run keeps the observed batch count and sample count explicit as the canonical executor batch geometry for the admitted local Mac profile.",
                ),
            },
            memory_headroom: PsionExecutorLocalClusterMemoryHeadroom {
                evidence_mode: String::from("admitted_compute_source_unified_memory"),
                admitted_memory_bytes: mlx_compute_source
                    .accelerators
                    .per_accelerator_memory_bytes,
                observed_free_memory_bytes: None,
                detail: String::from(
                    "The local Mac lane records unified-memory headroom from the admitted compute-source contract because the retained same-node decision-grade packet does not freeze a separate free-bytes probe.",
                ),
            },
            expected_throughput: PsionExecutorLocalClusterExpectedThroughput {
                expected_steps_per_second: 162.53061053630358,
                expected_samples_per_second: 20803.91814864686,
                expected_source_tokens_per_second: 4649675.706222572,
                detail: String::from(
                    "Expected throughput stays anchored to the retained same-node MLX report that the admitted-device matrix and MLX decision-grade packet already cite.",
                ),
            },
            detail: String::from(
                "This canonical registration row binds the retained same-node MLX decision-grade run to the admitted local Mac profile, the frozen executor eval packs, the current canonical mixture version, the local-first wallclock budget, and the model id that baseline truth already freezes.",
            ),
            registration_digest: String::new(),
        },
        PsionExecutorLocalClusterRunRegistrationRow {
            registration_id: String::from(CUDA_REGISTRATION_ID),
            run_type_id: String::from("cuda_4080_decision_grade"),
            run_id: cuda_packet.run_registration_row.cluster_support_run_id.clone(),
            search_run_ids: vec![
                cuda_packet.run_registration_row.cluster_support_run_id.clone(),
                cuda_packet.decision_run_id.clone(),
                cuda_packet.decision_matrix_run_id.clone(),
            ],
            admitted_profile_id: String::from(LOCAL_4080_PROFILE_ID),
            control_plane_profile_id: Some(String::from(LOCAL_TAILNET_CONTROL_PROFILE_ID)),
            compute_source_id: cuda_compute_source.source_id,
            machine_class_id: String::from(CUDA_MACHINE_CLASS_ID),
            execution_backend_label: String::from("open_adapter_backend.cuda.gpt_oss_lm_head"),
            logical_device_label: String::from("cuda:0"),
            model_id: baseline.model_id.clone(),
            mixture_version_id: canonical_mixture.mixture_id.clone(),
            candidate_status: PsionExecutorLocalClusterCandidateStatus::CurrentBest,
            checkpoint_family: smoke_packet.checkpoint_family.clone(),
            eval_pack_ids: cuda_packet.run_registration_row.eval_pack_ids.clone(),
            wallclock_budget_seconds: cuda_packet.run_registration_row.wallclock_budget_seconds,
            observed_duration_ms: cuda_packet.observed_wallclock_ms,
            stop_condition: String::from(
                "Consume the admitted 600 second 4080 decision-grade budget while keeping the frozen frequent and promotion packs bound to the supporting Tailnet cluster run and its retained same-node CUDA comparison run.",
            ),
            batch_geometry: PsionExecutorLocalClusterBatchGeometry {
                observed_batch_count: 8,
                observed_sample_count: 128,
                detail: String::from(
                    "The retained same-node CUDA decision-grade report keeps the canonical batch geometry for the admitted 4080 lane, while the smoke packet separately records the smaller contribution window that established checkpoint and failure truth.",
                ),
            },
            memory_headroom: PsionExecutorLocalClusterMemoryHeadroom {
                evidence_mode: String::from("admission_probe_plus_compute_source_contract"),
                admitted_memory_bytes: cuda_compute_source
                    .accelerators
                    .per_accelerator_memory_bytes,
                observed_free_memory_bytes: Some(smoke_packet.free_memory_bytes),
                detail: String::from(
                    "The 4080 lane records both the admitted per-accelerator memory from the compute-source contract and the explicit free-memory probe retained by the smoke packet.",
                ),
            },
            expected_throughput: PsionExecutorLocalClusterExpectedThroughput {
                expected_steps_per_second: 82.40252049829174,
                expected_samples_per_second: 10547.522623781342,
                expected_source_tokens_per_second: 2357371.30641513,
                detail: String::from(
                    "Expected throughput stays anchored to the retained same-node CUDA report that the 4080 decision-grade packet already binds into the admitted local-cluster comparison.",
                ),
            },
            detail: String::from(
                "This canonical registration row binds the supporting Tailnet cluster run and retained same-node CUDA decision run to the admitted 4080 worker profile, the Tailnet control-plane profile, the frozen executor eval packs, the current canonical mixture version, and the retained checkpoint family that later ledger rows will search.",
            ),
            registration_digest: String::new(),
        },
    ];
    for row in &mut rows {
        row.registration_digest = stable_registration_row_digest(row);
    }

    let mut packet = PsionExecutorLocalClusterRunRegistrationPacket {
        schema_version: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_local_cluster_run_registration_v1"),
        model_id: baseline.model_id,
        registration_rows: rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH),
            String::from(PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH),
            String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_SMOKE_RUN_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_DECISION_GRADE_DOC_PATH),
            String::from(LOCAL_MLX_COMPUTE_SOURCE_FIXTURE_PATH),
            String::from(LOCAL_4080_COMPUTE_SOURCE_FIXTURE_PATH),
        ],
        summary: String::from(
            "The executor lane now has one canonical local-cluster run-registration schema. Both the retained MLX decision-grade run and the retained 4080 decision-grade run register the admitted profile, current canonical mixture version, budget, observed duration, frozen eval packs, stop condition, batch geometry, memory headroom posture, and expected throughput facts in the same machine-readable packet, and missing required fields now block admission by validation instead of staying doc-only.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_registration_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_local_cluster_run_registration_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterRunRegistrationPacket, PsionExecutorLocalClusterRunRegistrationError>
{
    let packet = builtin_executor_local_cluster_run_registration_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorLocalClusterRunRegistrationError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorLocalClusterRunRegistrationError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionExecutorLocalClusterRunRegistrationError::Parse {
            path: relative_path.to_string(),
            error,
        }
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorLocalClusterRunRegistrationError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorLocalClusterRunRegistrationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorLocalClusterRunRegistrationError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorLocalClusterRunRegistrationError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorLocalClusterRunRegistrationError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_registration_row_digest(row: &PsionExecutorLocalClusterRunRegistrationRow) -> String {
    let mut clone = row.clone();
    clone.registration_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("registration row serialization should succeed"),
    ))
}

fn stable_registration_packet_digest(
    packet: &PsionExecutorLocalClusterRunRegistrationPacket,
) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("registration packet serialization should succeed"),
    ))
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
    fn builtin_executor_local_cluster_run_registration_packet_is_valid(
    ) -> Result<(), PsionExecutorLocalClusterRunRegistrationError> {
        let root = workspace_root();
        let packet = builtin_executor_local_cluster_run_registration_packet(root.as_path())?;
        packet.validate()?;
        assert!(packet
            .registration_rows
            .iter()
            .all(|row| row.mixture_version_id == "psion_executor_canonical_mixture_v0"));
        Ok(())
    }

    #[test]
    fn executor_local_cluster_run_registration_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorLocalClusterRunRegistrationError> {
        let root = workspace_root();
        let expected: PsionExecutorLocalClusterRunRegistrationPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_local_cluster_run_registration_packet(root.as_path())?;
        if actual != expected {
            return Err(PsionExecutorLocalClusterRunRegistrationError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn write_executor_local_cluster_run_registration_packet_persists_current_truth(
    ) -> Result<(), PsionExecutorLocalClusterRunRegistrationError> {
        let root = workspace_root();
        let packet = write_builtin_executor_local_cluster_run_registration_packet(root.as_path())?;
        let committed: PsionExecutorLocalClusterRunRegistrationPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH,
        )?;
        assert_eq!(packet, committed);
        Ok(())
    }

    #[test]
    fn missing_profile_id_blocks_admission(
    ) -> Result<(), PsionExecutorLocalClusterRunRegistrationError> {
        let root = workspace_root();
        let mut packet = builtin_executor_local_cluster_run_registration_packet(root.as_path())?;
        packet.registration_rows[0].admitted_profile_id.clear();
        let error = packet.validate().expect_err("missing profile should block admission");
        assert!(matches!(
            error,
            PsionExecutorLocalClusterRunRegistrationError::MissingField { field }
            if field == "psion_executor_local_cluster_run_registration.registration_rows[].admitted_profile_id"
        ));
        Ok(())
    }
}
