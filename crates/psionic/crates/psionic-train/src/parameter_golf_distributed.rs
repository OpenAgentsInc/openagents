use psionic_collectives::{
    CollectiveMeshMember, CollectiveSyncCadencePolicy, ElasticCollectivePlanner,
};
use psionic_core::{DType, Device, DeviceKind, Shape, TensorSpec};
use psionic_eval::{
    ParameterGolfDistributedChallengeThresholds, ParameterGolfDistributedCommunicationReceipt,
    ParameterGolfDistributedCommunicationStageReceipt, ParameterGolfDistributedLaneDisposition,
    ParameterGolfDistributedLaneRefusal, ParameterGolfDistributedLaneRefusalKind,
    ParameterGolfDistributedMemoryReceipt, ParameterGolfDistributedThroughputReceipt,
    ParameterGolfDistributedTimingReceipt, ParameterGolfDistributedTopologyReceipt,
    ParameterGolfDistributedValidationAggregationReceipt,
    ParameterGolfDistributedValidationShardReceipt, PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF,
    PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF,
    PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY,
};
use psionic_ir::GraphError;
use psionic_models::ParameterGolfModelDescriptor;
use psionic_runtime::{
    BackendSelection, ClusterCommunicationClass, ClusterExecutionCapabilityProfile,
    ClusterTransportClass, DeviceDescriptor, ExecutionTopologyPlan, ServedProductBackendPolicy,
    ServedProductFallbackTrigger, TrainingCollectiveKind, TrainingCollectiveQuantization,
    TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind, TrainingElasticMembershipContext,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use thiserror::Error;

use crate::{
    builtin_parameter_golf_cuda_training_capability_report, parameter_golf_optimizer_plan,
    DistributedOptimizerContract, DistributedOptimizerError, DistributedOptimizerGroupContract,
    DistributedOptimizerRun, DistributedTrainingMemoryBudget, OptimizerStateResidency,
    ParameterGolfBatchGeometry, ParameterGolfOptimizerExecution, ParameterGolfTrainError,
    ParameterGolfTrainingHyperparameters, ParameterGolfValidationEvalMode,
    TrainingActivationCheckpointPolicy, TrainingDistributedOptimizerKind,
    TrainingGradientAccumulationPolicy, TrainingGradientAccumulationReduction, TrainingLoopBudget,
    TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy, TrainingOptimizerShardResidency,
    TrainingOptimizerStateShardKind, TrainingOptimizerStateShardLayout,
    TrainingParameterGroupState, TrainingParameterShardKind, TrainingParameterShardLayout,
    TrainingPrecisionPolicy, TrainingShardPlacement, TrainingShardRange, TrainingTensorBuffer,
};

/// Stable version identifier for the distributed `8xH100` receipt lane.
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_VERSION: &str = "2026.03.18.distributed_8xh100.v1";
/// Stable version identifier for the RunPod operator-measurement bridge that
/// feeds the distributed `8xH100` receipt lane.
pub const PARAMETER_GOLF_RUNPOD_8XH100_MEASUREMENTS_VERSION: &str =
    "2026.03.24.runpod_8xh100_measurements.v1";

const PARAMETER_GOLF_DISTRIBUTED_CHECKPOINT_FAMILY: &str =
    "train.parameter_golf.distributed_8xh100";

/// One measured optimizer-step observation used to build the distributed receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedStepObservation {
    /// One-based global step number.
    pub global_step: u64,
    /// Observed step start.
    pub started_at_ms: u64,
    /// Observed step finish.
    pub finished_at_ms: u64,
    /// Global train tokens processed by the step.
    pub train_tokens: u64,
}

impl ParameterGolfDistributedStepObservation {
    /// Creates one measured step observation.
    #[must_use]
    pub const fn new(
        global_step: u64,
        started_at_ms: u64,
        finished_at_ms: u64,
        train_tokens: u64,
    ) -> Self {
        Self {
            global_step,
            started_at_ms,
            finished_at_ms,
            train_tokens,
        }
    }

    const fn duration_ms(&self) -> u64 {
        self.finished_at_ms.saturating_sub(self.started_at_ms)
    }
}

/// One observed runtime memory posture used to widen the distributed receipt
/// beyond the earlier analytic-only peak estimate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedMemoryObservation {
    /// Observed peak device bytes per worker.
    pub peak_device_bytes_per_worker: u64,
    /// Observed peak host bytes per worker.
    pub peak_host_bytes_per_worker: u64,
    /// Plain-language source for the observation.
    pub source: String,
}

/// One observed rank-local validation shard for the distributed receipt lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedValidationShardObservation {
    /// Zero-based rank identifier.
    pub rank: usize,
    /// Zero-based global validation-sequence offset covered by this rank's scored tokens.
    pub sequence_start: u64,
    /// Number of validation sequences covered by this rank's scored tokens.
    pub sequence_count: u64,
    /// Zero-based global validation evaluation-unit offset owned by this rank.
    #[serde(default)]
    pub evaluation_unit_start: u64,
    /// Number of validation evaluation units owned by this rank.
    #[serde(default)]
    pub evaluation_unit_count: u64,
    /// Zero-based scored-token offset owned by this rank.
    #[serde(default)]
    pub scored_token_start: u64,
    /// Number of scored tokens owned by this rank.
    #[serde(default)]
    pub scored_token_count: u64,
    /// Rank-local summed loss over the shard.
    pub loss_sum: f64,
    /// Rank-local evaluated token count.
    pub token_count: u64,
    /// Rank-local evaluated byte count.
    pub byte_count: u64,
    /// Rank-local observed validation wallclock.
    pub observed_ms: u64,
}

/// Config for the distributed `8xH100` Parameter Golf receipt lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributed8xH100Config {
    /// Stable run identifier.
    pub run_id: String,
    /// Stable mesh identifier.
    pub mesh_id: String,
    /// Public challenge batch geometry.
    pub geometry: ParameterGolfBatchGeometry,
    /// Declared challenge thresholds.
    pub thresholds: ParameterGolfDistributedChallengeThresholds,
    /// Ordered measured step observations.
    pub step_observations: Vec<ParameterGolfDistributedStepObservation>,
    /// Validation evaluation mode used by the lane.
    #[serde(default)]
    pub validation_eval_mode: ParameterGolfValidationEvalMode,
    /// Observed runtime memory posture when the lane has real execution
    /// telemetry instead of only the analytic optimizer-contract estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_observation: Option<ParameterGolfDistributedMemoryObservation>,
    /// Expected local validation batch size in sequences or windows.
    #[serde(default = "default_validation_batch_sequences")]
    pub validation_batch_sequences: u64,
    /// Observed validation duration.
    pub validation_observed_ms: u64,
    /// Total validation sequence count across all ranks when distributed
    /// validation sharding facts are available.
    #[serde(default)]
    pub validation_total_sequence_count: u64,
    /// Ordered rank-local validation shard observations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_shard_observations: Vec<ParameterGolfDistributedValidationShardObservation>,
    /// Observed export or roundtrip duration.
    pub export_observed_ms: u64,
}

impl ParameterGolfDistributed8xH100Config {
    /// Returns the canonical public `8xH100` config skeleton.
    #[must_use]
    pub fn challenge_defaults() -> Self {
        Self {
            run_id: String::from("parameter-golf-distributed-8xh100"),
            mesh_id: String::from("mesh.parameter_golf.8xh100"),
            geometry: ParameterGolfBatchGeometry::challenge_distributed_8xh100_defaults(),
            thresholds: ParameterGolfDistributedChallengeThresholds::challenge_8xh100(),
            step_observations: Vec::new(),
            validation_eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
            memory_observation: None,
            validation_batch_sequences: default_validation_batch_sequences(),
            validation_observed_ms: 0,
            validation_total_sequence_count: 0,
            validation_shard_observations: Vec::new(),
            export_observed_ms: 0,
        }
    }

    fn validate(&self) -> Result<(), ParameterGolfDistributedLaneError> {
        if self.run_id.trim().is_empty() {
            return Err(ParameterGolfDistributedLaneError::MissingRunId);
        }
        validate_distributed_geometry(&self.geometry, &self.thresholds)?;
        if self.validation_batch_sequences == 0 {
            return Err(ParameterGolfDistributedLaneError::InvalidConfig {
                message: String::from(
                    "validation_batch_sequences must be positive for the distributed validation lane",
                ),
            });
        }
        if let ParameterGolfValidationEvalMode::SlidingWindow { stride } = self.validation_eval_mode
        {
            if stride == 0 || stride > self.geometry.train_sequence_length {
                return Err(ParameterGolfDistributedLaneError::InvalidConfig {
                    message: format!(
                        "sliding-window distributed validation stride must be in 1..={}, observed {}",
                        self.geometry.train_sequence_length, stride
                    ),
                });
            }
        }
        for observation in &self.step_observations {
            if observation.global_step == 0 {
                return Err(ParameterGolfDistributedLaneError::InvalidStepObservation {
                    message: String::from("global_step must be positive"),
                });
            }
            if observation.finished_at_ms < observation.started_at_ms {
                return Err(ParameterGolfDistributedLaneError::InvalidStepObservation {
                    message: format!(
                        "step {} finished_at_ms={} is earlier than started_at_ms={}",
                        observation.global_step,
                        observation.finished_at_ms,
                        observation.started_at_ms
                    ),
                });
            }
        }
        if let Some(memory_observation) = &self.memory_observation {
            if memory_observation.source.trim().is_empty() {
                return Err(ParameterGolfDistributedLaneError::InvalidConfig {
                    message: String::from(
                        "memory_observation.source must be non-empty when observed memory is supplied",
                    ),
                });
            }
        }
        if !self.validation_shard_observations.is_empty() {
            if self.validation_total_sequence_count == 0 {
                return Err(ParameterGolfDistributedLaneError::InvalidConfig {
                    message: String::from(
                        "validation_total_sequence_count must be positive when distributed validation shard observations are supplied",
                    ),
                });
            }
            let shard_plan = build_validation_observation_plan(
                self.validation_total_sequence_count,
                self.geometry.train_sequence_length,
                &self.validation_eval_mode,
                self.geometry.world_size,
            )?;
            if self.validation_shard_observations.len() != shard_plan.len() {
                return Err(ParameterGolfDistributedLaneError::InvalidConfig {
                    message: format!(
                        "distributed validation shard observations must cover exactly {} ranks, found {}",
                        shard_plan.len(),
                        self.validation_shard_observations.len()
                    ),
                });
            }
            for (expected, observed) in shard_plan
                .iter()
                .zip(self.validation_shard_observations.iter())
            {
                if observed.rank != expected.rank
                    || observed.sequence_start != expected.sequence_start
                    || observed.sequence_count != expected.sequence_count
                    || observed.evaluation_unit_start != expected.evaluation_unit_start
                    || observed.evaluation_unit_count != expected.evaluation_unit_count
                    || observed.scored_token_start != expected.scored_token_start
                    || observed.scored_token_count != expected.scored_token_count
                {
                    return Err(ParameterGolfDistributedLaneError::InvalidConfig {
                        message: format!(
                            "validation shard observation for rank {} does not match the expected {:?} shard layout",
                            observed.rank, self.validation_eval_mode
                        ),
                    });
                }
            }
        } else if self.validation_total_sequence_count != 0 {
            return Err(ParameterGolfDistributedLaneError::InvalidConfig {
                message: String::from(
                    "validation_total_sequence_count cannot be set without distributed validation shard observations",
                ),
            });
        }
        Ok(())
    }
}

/// Minimal operator-collected measurements that can be lifted from one RunPod
/// `8xH100` run root into the typed distributed receipt lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfRunPod8xH100Measurements {
    /// Stable schema version.
    pub schema_version: String,
    /// Optional stable run identifier override. When omitted, the caller-owned
    /// run-root identifier is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Optional mesh identifier override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_id: Option<String>,
    /// Ordered measured step observations.
    #[serde(default)]
    pub step_observations: Vec<ParameterGolfDistributedStepObservation>,
    /// Validation evaluation mode used by the lane.
    #[serde(default)]
    pub validation_eval_mode: ParameterGolfValidationEvalMode,
    /// Expected local validation batch size in sequences or windows.
    #[serde(default = "default_validation_batch_sequences")]
    pub validation_batch_sequences: u64,
    /// Observed distributed validation duration.
    pub validation_observed_ms: u64,
    /// Total validation sequence count across all ranks when distributed
    /// validation sharding facts are available.
    #[serde(default)]
    pub validation_total_sequence_count: u64,
    /// Ordered rank-local validation shard observations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_shard_observations: Vec<ParameterGolfDistributedValidationShardObservation>,
    /// Observed distributed export or roundtrip duration.
    pub export_observed_ms: u64,
    /// Observed runtime memory posture when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_observation: Option<ParameterGolfDistributedMemoryObservation>,
}

impl ParameterGolfRunPod8xH100Measurements {
    /// Returns the canonical RunPod `8xH100` measurement skeleton.
    #[must_use]
    pub fn challenge_defaults() -> Self {
        Self {
            schema_version: String::from(PARAMETER_GOLF_RUNPOD_8XH100_MEASUREMENTS_VERSION),
            run_id: None,
            mesh_id: None,
            step_observations: Vec::new(),
            validation_eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
            validation_batch_sequences: default_validation_batch_sequences(),
            validation_observed_ms: 0,
            validation_total_sequence_count: 0,
            validation_shard_observations: Vec::new(),
            export_observed_ms: 0,
            memory_observation: None,
        }
    }

    pub(crate) fn into_config(self, fallback_run_id: &str) -> ParameterGolfDistributed8xH100Config {
        let mut config = ParameterGolfDistributed8xH100Config::challenge_defaults();
        config.run_id = self.run_id.unwrap_or_else(|| String::from(fallback_run_id));
        config.mesh_id = self
            .mesh_id
            .unwrap_or_else(|| String::from("mesh.parameter_golf.runpod_8xh100"));
        config.step_observations = self.step_observations;
        config.validation_eval_mode = self.validation_eval_mode;
        config.validation_batch_sequences = self.validation_batch_sequences;
        config.validation_observed_ms = self.validation_observed_ms;
        config.validation_total_sequence_count = self.validation_total_sequence_count;
        config.validation_shard_observations = self.validation_shard_observations;
        config.export_observed_ms = self.export_observed_ms;
        config.memory_observation = self.memory_observation;
        config
    }
}

const fn default_validation_batch_sequences() -> u64 {
    64
}

/// Failure while building the distributed `8xH100` Parameter Golf lane.
#[derive(Debug, Error)]
pub enum ParameterGolfDistributedLaneError {
    #[error("distributed parameter golf config is missing `run_id`")]
    MissingRunId,
    #[error("distributed parameter golf config is invalid: {message}")]
    InvalidConfig { message: String },
    #[error("distributed parameter golf measurements are invalid: {message}")]
    InvalidMeasurements { message: String },
    #[error("distributed parameter golf inventory is invalid: {message}")]
    InvalidInventory { message: String },
    #[error("distributed parameter golf step observation is invalid: {message}")]
    InvalidStepObservation { message: String },
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error(transparent)]
    CollectivePlanning(#[from] psionic_collectives::CollectivePlanningError),
    #[error(transparent)]
    DistributedOptimizer(#[from] DistributedOptimizerError),
    #[error(transparent)]
    TrainingCore(#[from] crate::TrainingCoreError),
    #[error(transparent)]
    Train(#[from] ParameterGolfTrainError),
}

/// Returns the canonical RunPod `8xH100` cluster capability profile used by
/// the Parameter Golf distributed receipt lane.
#[must_use]
pub fn parameter_golf_runpod_8xh100_capability_profile() -> ClusterExecutionCapabilityProfile {
    ClusterExecutionCapabilityProfile::new("cuda")
        .with_supported_communication_classes(vec![ClusterCommunicationClass::TensorCollectiveMesh])
        .with_detail(
            "single-pod RunPod H100 mesh advertises the same tensor_collective_mesh vocabulary as the intended WORLD_SIZE=8 challenge lane",
        )
}

/// Parses the exact `nvidia-smi --query-gpu=index,name,memory.total,memory.used,utilization.gpu --format=csv,noheader`
/// surface emitted by the RunPod finalizer into typed CUDA device descriptors.
pub fn parse_parameter_golf_runpod_8xh100_inventory(
    inventory_csv: &str,
) -> Result<Vec<DeviceDescriptor>, ParameterGolfDistributedLaneError> {
    let mut devices = Vec::new();
    for raw_line in inventory_csv.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let fields = line.split(',').map(str::trim).collect::<Vec<_>>();
        if fields.len() != 5 {
            return Err(ParameterGolfDistributedLaneError::InvalidInventory {
                message: format!(
                    "expected 5 csv columns in nvidia-smi inventory line, found {} in `{line}`",
                    fields.len()
                ),
            });
        }
        let ordinal = fields[0].parse::<u16>().map_err(|error| {
            ParameterGolfDistributedLaneError::InvalidInventory {
                message: format!("failed to parse GPU index `{}`: {error}", fields[0]),
            }
        })?;
        let device_name = fields[1];
        let total_mib = parse_inventory_quantity(fields[2], "MiB")?;
        let used_mib = parse_inventory_quantity(fields[3], "MiB")?;
        let utilization_percent = parse_inventory_quantity(fields[4], "%")?;
        let reported_memory_capacity_bytes = total_mib.saturating_mul(1024 * 1024);
        let mut warnings = Vec::new();
        warnings.push(format!(
            "inventory_snapshot.memory_used_mib={used_mib}; inventory_snapshot.utilization_percent={utilization_percent}"
        ));
        let is_h100 = device_name.contains("H100");
        let canonical_h100_memory_bytes = 80_u64 * 1024 * 1024 * 1024;
        let memory_capacity_bytes = if is_h100 {
            reported_memory_capacity_bytes.max(canonical_h100_memory_bytes)
        } else {
            reported_memory_capacity_bytes
        };
        devices.push(DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(DeviceKind::Cuda, ordinal, Some(format!("cuda:{ordinal}"))),
            device_name: Some(String::from(device_name)),
            supported_dtypes: vec![DType::F32, DType::BF16],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(memory_capacity_bytes),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: Some(psionic_runtime::NvidiaDeviceMetadata {
                topology: psionic_runtime::NvidiaTopologyInfo {
                    architecture: is_h100.then(|| String::from("hopper")),
                    compute_capability: is_h100.then(|| String::from("9.0")),
                    pci_bdf: None,
                    sm_count: is_h100.then_some(132),
                    vram_bytes: Some(memory_capacity_bytes),
                    mig_profile: None,
                },
                risk: psionic_runtime::NvidiaRiskProfile {
                    level: psionic_runtime::NvidiaRiskLevel::Standard,
                    display_attached: Some(false),
                    mig_partitioned: false,
                    warnings,
                },
                recovery: psionic_runtime::NvidiaRecoveryProfile {
                    supports_gpu_reset: Some(true),
                    expected_actions: vec![
                        psionic_runtime::NvidiaRecoveryAction::ProcessRestart,
                        psionic_runtime::NvidiaRecoveryAction::GpuReset,
                        psionic_runtime::NvidiaRecoveryAction::RebootHost,
                    ],
                },
            }),
        });
    }
    if devices.is_empty() {
        return Err(ParameterGolfDistributedLaneError::InvalidInventory {
            message: String::from("the RunPod nvidia-smi inventory was empty"),
        });
    }
    Ok(devices)
}

/// Builds the typed distributed receipt directly from a RunPod `8xH100`
/// inventory snapshot plus the minimal operator-collected timing or memory
/// measurements.
pub fn benchmark_parameter_golf_runpod_8xh100_from_measurements(
    descriptor: &ParameterGolfModelDescriptor,
    hyperparameters: &ParameterGolfTrainingHyperparameters,
    run_root_id: &str,
    inventory_csv: &str,
    measurements: ParameterGolfRunPod8xH100Measurements,
) -> Result<ParameterGolfDistributedThroughputReceipt, ParameterGolfDistributedLaneError> {
    let devices = parse_parameter_golf_runpod_8xh100_inventory(inventory_csv)?;
    benchmark_parameter_golf_distributed_8xh100(
        descriptor,
        hyperparameters,
        devices.as_slice(),
        &parameter_golf_runpod_8xh100_capability_profile(),
        &measurements.into_config(run_root_id),
    )
}

/// Builds the distributed `8xH100` Parameter Golf throughput receipt.
pub fn benchmark_parameter_golf_distributed_8xh100(
    descriptor: &ParameterGolfModelDescriptor,
    hyperparameters: &ParameterGolfTrainingHyperparameters,
    devices: &[DeviceDescriptor],
    capability_profile: &ClusterExecutionCapabilityProfile,
    config: &ParameterGolfDistributed8xH100Config,
) -> Result<ParameterGolfDistributedThroughputReceipt, ParameterGolfDistributedLaneError> {
    config.validate()?;
    let optimizer_plan = parameter_golf_optimizer_plan(descriptor, hyperparameters)?;
    let optimizer_plan_digest =
        stable_digest(b"psionic_parameter_golf_optimizer_plan|", &optimizer_plan);
    let cuda_coverage_report = builtin_parameter_golf_cuda_training_capability_report()?;
    let (backend_selection, all_devices_match_required_model, inventory_refusal) =
        distributed_backend_selection(devices, capability_profile, &config.thresholds);
    let topology_digest = backend_selection
        .execution_topology
        .as_ref()
        .map_or_else(String::new, ExecutionTopologyPlan::stable_digest);
    let topology = ParameterGolfDistributedTopologyReceipt {
        selected_device_names: devices
            .iter()
            .map(|device| {
                device
                    .device_name
                    .clone()
                    .unwrap_or_else(|| device.device.label().unwrap_or("unknown").to_string())
            })
            .collect(),
        backend_selection,
        topology_digest,
        all_devices_match_required_model,
    };
    let axes = vec![TrainingDeviceMeshAxis::new(
        "dp",
        TrainingDeviceMeshAxisKind::DataParallel,
        config.geometry.world_size,
    )
    .with_collective_group_size(config.geometry.world_size)
    .with_detail("matches WORLD_SIZE=8 DDP replica axis from train_gpt.py")];
    let matrix_parameter_count = optimizer_plan
        .groups
        .iter()
        .find(|group| {
            matches!(
                group.execution,
                ParameterGolfOptimizerExecution::Muon { .. }
            )
        })
        .map_or(0_u64, |group| group.parameter_count as u64);
    let total_parameter_count = optimizer_plan.total_parameter_count as u64;
    let ddp_gradient_payload_bytes = total_parameter_count.saturating_mul(2);
    let muon_update_payload_bytes = matrix_parameter_count.saturating_mul(2);
    let validation_payload_bytes = 3 * 8;
    let communication = build_communication_receipt(
        &config.mesh_id,
        &axes,
        devices,
        ddp_gradient_payload_bytes,
        muon_update_payload_bytes,
        validation_payload_bytes,
    )?;
    let validation_aggregation = build_validation_aggregation_receipt(config)?;
    let memory = build_memory_receipt(
        descriptor,
        hyperparameters,
        &config.geometry,
        &config.thresholds,
        total_parameter_count,
        config.memory_observation.as_ref(),
    )?;
    let timing = build_timing_receipt(config);
    let mut boundary_notes = vec![String::from(
        "This receipt mirrors the public train_gpt.py 8xH100 posture: replicated DDP, WORLD_SIZE=8, grad_accum_steps=1, NCCL-style all-reduce for training and validation.",
    )];
    boundary_notes.extend(cuda_coverage_report.boundary_notes());
    if let Some(memory_observation) = config.memory_observation.as_ref() {
        boundary_notes.push(format!(
            "The memory receipt binds observed runtime peak bytes from `{}` while still using the analytic optimizer contract for the logical parameter, gradient, optimizer-state, master-weight, and activation breakdown.",
            memory_observation.source
        ));
    } else {
        boundary_notes.push(String::from(
            "The memory receipt is an analytic upper bound over the distributed optimizer contract; it is not a direct CUDA allocator trace.",
        ));
    }
    if let Some(validation_aggregation) = validation_aggregation.as_ref() {
        boundary_notes.push(format!(
            "Distributed validation now preserves {} rank-local shard receipts over {} total sequences, aggregates loss_sum/token_count/byte_count across the full mesh, and reports one honest validation wallclock as the slowest participating rank.",
            validation_aggregation.shards.len(),
            validation_aggregation.total_sequence_count,
        ));
    } else {
        boundary_notes.push(String::from(
            "The distributed receipt still lacks typed rank-local validation shard observations, so validation sharding and aggregation remain narrative rather than execution-backed.",
        ));
    }

    let refusal = inventory_refusal
        .or_else(|| {
            if !memory.within_device_budget || !memory.within_host_budget {
                Some(ParameterGolfDistributedLaneRefusal {
                    refusal_kind: ParameterGolfDistributedLaneRefusalKind::MemoryBudgetExceeded,
                    reason: format!(
                        "planned memory exceeds the declared challenge bar: peak_device_bytes_per_worker={} peak_host_bytes_per_worker={}",
                        memory.peak_device_bytes_per_worker, memory.peak_host_bytes_per_worker
                    ),
                    fallback_benchmark_ref: String::from(
                        PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF,
                    ),
                })
            } else {
                None
            }
        })
        .or_else(|| {
            if config.step_observations.is_empty() {
                Some(ParameterGolfDistributedLaneRefusal {
                    refusal_kind: ParameterGolfDistributedLaneRefusalKind::MeasurementsMissing,
                    reason: String::from(
                        "no distributed optimizer-step timing observations were supplied for the 8xH100 lane",
                    ),
                    fallback_benchmark_ref: String::from(
                        PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF,
                    ),
                })
            } else {
                None
            }
        })
        .or_else(|| {
            timing.as_ref().and_then(|timing| {
                (!timing.within_wallclock_cap).then(|| ParameterGolfDistributedLaneRefusal {
                    refusal_kind: ParameterGolfDistributedLaneRefusalKind::WallclockExceeded,
                    reason: format!(
                        "observed distributed wallclock {}ms exceeds the declared cap {}ms",
                        timing.total_observed_ms, timing.wallclock_cap_ms
                    ),
                    fallback_benchmark_ref: String::from(
                        PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF,
                    ),
                })
            })
        });

    Ok(ParameterGolfDistributedThroughputReceipt {
        benchmark_ref: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF),
        run_id: config.run_id.clone(),
        model_descriptor_digest: descriptor.stable_digest(),
        optimizer_plan_digest,
        thresholds: config.thresholds.clone(),
        topology,
        communication,
        training_capability_report_digest: cuda_coverage_report.report_digest.clone(),
        challenge_kernel_blockers: cuda_coverage_report.challenge_kernel_blockers().to_vec(),
        disposition: if refusal.is_some() {
            ParameterGolfDistributedLaneDisposition::Refused
        } else {
            ParameterGolfDistributedLaneDisposition::Measured
        },
        timing,
        validation_aggregation,
        memory: Some(memory),
        refusal,
        boundary_notes,
        claim_boundary: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY),
        receipt_digest: String::new(),
    }
    .with_stable_digest())
}

/// Builds one RunPod `8xH100` measurements JSON payload directly from a
/// retained execution log.
///
/// The builder currently accepts the upstream `train_gpt.py` train/validation
/// line shape plus the final roundtrip and peak-memory lines that the later
/// RunPod evidence lane already preserves in `execution.log`.
pub fn build_parameter_golf_runpod_8xh100_measurements_from_train_log(
    log_text: &str,
    run_id: Option<&str>,
    mesh_id: Option<&str>,
    memory_source: Option<&str>,
) -> Result<ParameterGolfRunPod8xH100Measurements, ParameterGolfDistributedLaneError> {
    let geometry = ParameterGolfBatchGeometry::challenge_distributed_8xh100_defaults();
    let mut checkpoints = Vec::<(u64, u64)>::new();
    let mut final_validation_observed_ms = None;
    let mut final_roundtrip_eval_ms = None;
    let mut peak_allocated_mib = None;
    let mut peak_reserved_mib = None;
    let mut validation_eval_mode = ParameterGolfValidationEvalMode::NonOverlapping;
    let mut validation_batch_sequences = default_validation_batch_sequences();
    let mut validation_shard_observations = Vec::new();

    for line in log_text.lines().map(str::trim) {
        if line.starts_with("step:") && line.contains(" train_loss:") {
            let global_step = extract_u64_after(line, "step:", "/").ok_or_else(|| {
                ParameterGolfDistributedLaneError::InvalidMeasurements {
                    message: format!("failed to parse train-step line `{line}`"),
                }
            })?;
            let train_time_ms = extract_u64_after(line, "train_time:", "ms").ok_or_else(|| {
                ParameterGolfDistributedLaneError::InvalidMeasurements {
                    message: format!("failed to parse train_time from `{line}`"),
                }
            })?;
            checkpoints.push((global_step, train_time_ms));
        } else if line.starts_with("step:") && line.contains(" val_loss:") {
            final_validation_observed_ms = extract_u64_after(line, "train_time:", "ms");
        } else if line.starts_with("peak memory allocated:") {
            peak_allocated_mib = extract_u64_after(line, "peak memory allocated:", "MiB");
            peak_reserved_mib = extract_u64_after(line, "reserved:", "MiB");
        } else if line.starts_with("distributed_validation_rank_complete ")
            && line.contains(" rank=")
            && line.contains(" sequence_start=")
            && line.contains(" sequence_count=")
            && line.contains(" loss_sum=")
            && line.contains(" token_count=")
            && line.contains(" byte_count=")
            && line.contains(" elapsed_ms=")
        {
            let rank = extract_u64_after(line, "rank=", " ").ok_or_else(|| {
                ParameterGolfDistributedLaneError::InvalidMeasurements {
                    message: format!("failed to parse distributed validation rank from `{line}`"),
                }
            })? as usize;
            let sequence_start =
                extract_u64_after(line, "sequence_start=", " ").ok_or_else(|| {
                    ParameterGolfDistributedLaneError::InvalidMeasurements {
                        message: format!(
                            "failed to parse distributed validation sequence_start from `{line}`"
                        ),
                    }
                })?;
            let sequence_count =
                extract_u64_after(line, "sequence_count=", " ").ok_or_else(|| {
                    ParameterGolfDistributedLaneError::InvalidMeasurements {
                        message: format!(
                            "failed to parse distributed validation sequence_count from `{line}`"
                        ),
                    }
                })?;
            let loss_sum = extract_f64_after(line, "loss_sum=", " ").ok_or_else(|| {
                ParameterGolfDistributedLaneError::InvalidMeasurements {
                    message: format!(
                        "failed to parse distributed validation loss_sum from `{line}`"
                    ),
                }
            })?;
            let token_count = extract_u64_after(line, "token_count=", " ").ok_or_else(|| {
                ParameterGolfDistributedLaneError::InvalidMeasurements {
                    message: format!(
                        "failed to parse distributed validation token_count from `{line}`"
                    ),
                }
            })?;
            let byte_count = extract_u64_after(line, "byte_count=", " ").ok_or_else(|| {
                ParameterGolfDistributedLaneError::InvalidMeasurements {
                    message: format!(
                        "failed to parse distributed validation byte_count from `{line}`"
                    ),
                }
            })?;
            let observed_ms = extract_u64_after(line, "elapsed_ms=", "").ok_or_else(|| {
                ParameterGolfDistributedLaneError::InvalidMeasurements {
                    message: format!(
                        "failed to parse distributed validation elapsed_ms from `{line}`"
                    ),
                }
            })?;
            if let Some(raw_mode) = extract_string_after(line, "eval_mode=", " ") {
                validation_eval_mode = ParameterGolfValidationEvalMode::parse(raw_mode.as_str())
                    .map_err(|error| ParameterGolfDistributedLaneError::InvalidMeasurements {
                        message: format!(
                            "failed to parse distributed validation eval_mode from `{line}`: {error}"
                        ),
                    })?;
            }
            if let Some(batch_sequences) = extract_u64_after(line, "batch_sequences=", " ") {
                validation_batch_sequences = batch_sequences;
            }
            validation_shard_observations.push(
                ParameterGolfDistributedValidationShardObservation {
                    rank,
                    sequence_start,
                    sequence_count,
                    evaluation_unit_start: extract_u64_after(line, "evaluation_unit_start=", " ")
                        .unwrap_or(sequence_start),
                    evaluation_unit_count: extract_u64_after(line, "evaluation_unit_count=", " ")
                        .unwrap_or(sequence_count),
                    scored_token_start: extract_u64_after(line, "scored_token_start=", " ")
                        .unwrap_or(
                            sequence_start.saturating_mul(geometry.train_sequence_length as u64),
                        ),
                    scored_token_count: extract_u64_after(line, "scored_token_count=", " ")
                        .unwrap_or(
                            sequence_count.saturating_mul(geometry.train_sequence_length as u64),
                        ),
                    loss_sum,
                    token_count,
                    byte_count,
                    observed_ms,
                },
            );
        } else if line.starts_with("final_")
            && line.contains("_roundtrip")
            && line.contains("eval_time:")
        {
            final_roundtrip_eval_ms = extract_u64_after(line, "eval_time:", "ms");
        }
    }

    if checkpoints.is_empty() {
        return Err(ParameterGolfDistributedLaneError::InvalidMeasurements {
            message: String::from("missing any `step:... train_loss:... train_time:...` lines"),
        });
    }

    let mut step_observations = Vec::new();
    let mut previous_step = 0_u64;
    let mut previous_finish_ms = 0_u64;
    for (global_step, finished_at_ms) in checkpoints {
        if global_step <= previous_step {
            return Err(ParameterGolfDistributedLaneError::InvalidMeasurements {
                message: format!(
                    "train-step checkpoints must be strictly increasing, found step {} after {}",
                    global_step, previous_step
                ),
            });
        }
        if finished_at_ms < previous_finish_ms {
            return Err(ParameterGolfDistributedLaneError::InvalidMeasurements {
                message: format!(
                    "train-step cumulative time regressed from {}ms to {}ms at step {}",
                    previous_finish_ms, finished_at_ms, global_step
                ),
            });
        }

        let delta_steps = global_step - previous_step;
        let delta_ms = finished_at_ms - previous_finish_ms;
        let base_step_ms = delta_ms / delta_steps;
        let remainder_ms = delta_ms % delta_steps;
        let mut cursor_ms = previous_finish_ms;
        for offset in 0..delta_steps {
            let step_duration_ms = base_step_ms + u64::from(offset < remainder_ms);
            let step_start_ms = cursor_ms;
            let step_finish_ms = step_start_ms.saturating_add(step_duration_ms);
            cursor_ms = step_finish_ms;
            step_observations.push(ParameterGolfDistributedStepObservation::new(
                previous_step + offset + 1,
                step_start_ms,
                step_finish_ms,
                geometry.train_batch_tokens as u64,
            ));
        }
        previous_step = global_step;
        previous_finish_ms = finished_at_ms;
    }

    let mut measurements = ParameterGolfRunPod8xH100Measurements::challenge_defaults();
    measurements.run_id = run_id.map(String::from);
    measurements.mesh_id = mesh_id.map(String::from);
    measurements.step_observations = step_observations;
    measurements.validation_eval_mode = validation_eval_mode.clone();
    measurements.validation_batch_sequences = validation_batch_sequences;
    measurements.validation_observed_ms =
        final_validation_observed_ms.map_or(0, |value| value.saturating_sub(previous_finish_ms));
    if !validation_shard_observations.is_empty() {
        validation_shard_observations.sort_by_key(|observation| observation.rank);
        for pair in validation_shard_observations.windows(2) {
            if pair[0].rank == pair[1].rank {
                return Err(ParameterGolfDistributedLaneError::InvalidMeasurements {
                    message: format!(
                        "distributed validation observations contain duplicate rank {}",
                        pair[0].rank
                    ),
                });
            }
        }
        if validation_shard_observations
            .iter()
            .any(|observation| observation.rank >= geometry.world_size)
        {
            return Err(ParameterGolfDistributedLaneError::InvalidMeasurements {
                message: format!(
                    "distributed validation observations must use ranks inside the world_size={} posture",
                    geometry.world_size
                ),
            });
        }
        measurements.validation_total_sequence_count = match validation_eval_mode {
            ParameterGolfValidationEvalMode::NonOverlapping => validation_shard_observations
                .iter()
                .map(|observation| observation.sequence_count)
                .sum(),
            ParameterGolfValidationEvalMode::SlidingWindow { .. } => validation_shard_observations
                .iter()
                .map(|observation| {
                    observation
                        .scored_token_start
                        .saturating_add(observation.scored_token_count)
                })
                .max()
                .unwrap_or(0)
                .div_ceil(geometry.train_sequence_length as u64),
        };
        measurements.validation_observed_ms = measurements.validation_observed_ms.max(
            validation_shard_observations
                .iter()
                .map(|observation| observation.observed_ms)
                .max()
                .unwrap_or(0),
        );
        measurements.validation_shard_observations = validation_shard_observations;
    }
    measurements.export_observed_ms = final_roundtrip_eval_ms.unwrap_or(0);
    if let Some(peak_device_mib) = peak_reserved_mib.or(peak_allocated_mib) {
        measurements.memory_observation = Some(ParameterGolfDistributedMemoryObservation {
            peak_device_bytes_per_worker: peak_device_mib.saturating_mul(1024 * 1024),
            peak_host_bytes_per_worker: 0,
            source: memory_source
                .unwrap_or("execution log peak memory")
                .to_string(),
        });
    }
    Ok(measurements)
}

fn validate_distributed_geometry(
    geometry: &ParameterGolfBatchGeometry,
    thresholds: &ParameterGolfDistributedChallengeThresholds,
) -> Result<(), ParameterGolfDistributedLaneError> {
    if geometry.world_size != thresholds.required_world_size {
        return Err(ParameterGolfDistributedLaneError::InvalidConfig {
            message: format!(
                "geometry.world_size={} must match the required challenge world_size={}",
                geometry.world_size, thresholds.required_world_size
            ),
        });
    }
    if geometry.grad_accum_steps != 1 {
        return Err(ParameterGolfDistributedLaneError::InvalidConfig {
            message: format!(
                "the exact 8xH100 lane requires grad_accum_steps=1, found {}",
                geometry.grad_accum_steps
            ),
        });
    }
    if geometry.train_sequence_length == 0 {
        return Err(ParameterGolfDistributedLaneError::InvalidConfig {
            message: String::from("train_sequence_length must be positive"),
        });
    }
    let denom = geometry
        .world_size
        .saturating_mul(geometry.grad_accum_steps);
    if geometry.train_batch_tokens == 0
        || geometry.train_batch_tokens % denom != 0
        || geometry.local_train_batch_tokens() < geometry.train_sequence_length
        || geometry.local_train_batch_tokens() % geometry.train_sequence_length != 0
    {
        return Err(ParameterGolfDistributedLaneError::InvalidConfig {
            message: format!(
                "train_batch_tokens={} must divide cleanly across world_size={} and admit full sequences of length {}",
                geometry.train_batch_tokens, geometry.world_size, geometry.train_sequence_length
            ),
        });
    }
    if geometry.validation_batch_tokens == 0
        || geometry.validation_batch_tokens % denom != 0
        || geometry.local_validation_batch_tokens() < geometry.train_sequence_length
        || geometry.local_validation_batch_tokens() % geometry.train_sequence_length != 0
    {
        return Err(ParameterGolfDistributedLaneError::InvalidConfig {
            message: format!(
                "validation_batch_tokens={} must divide cleanly across world_size={} and admit full sequences of length {}",
                geometry.validation_batch_tokens,
                geometry.world_size,
                geometry.train_sequence_length
            ),
        });
    }
    Ok(())
}

fn distributed_backend_selection(
    devices: &[DeviceDescriptor],
    capability_profile: &ClusterExecutionCapabilityProfile,
    thresholds: &ParameterGolfDistributedChallengeThresholds,
) -> (
    BackendSelection,
    bool,
    Option<ParameterGolfDistributedLaneRefusal>,
) {
    let supported_ops = vec![String::from("parameter_golf_distributed_train")];
    let topology = (!devices.is_empty()).then(|| {
        ExecutionTopologyPlan::replicated(
            thresholds.required_backend.clone(),
            devices
                .iter()
                .map(DeviceDescriptor::inventory_qualifiers)
                .collect(),
        )
    });
    let all_devices_match_required_model = devices
        .iter()
        .all(|device| device_matches_h100(device, thresholds));
    let rejection_reason = if capability_profile.runtime_backend != thresholds.required_backend {
        Some(format!(
            "cluster capability profile targets backend `{}` instead of required `{}`",
            capability_profile.runtime_backend, thresholds.required_backend
        ))
    } else if !capability_profile
        .supports_communication_class(ClusterCommunicationClass::TensorCollectiveMesh)
    {
        Some(String::from(
            "cluster capability profile does not advertise tensor_collective_mesh support required for NCCL-style all-reduce",
        ))
    } else if devices.len() != thresholds.required_world_size {
        Some(format!(
            "expected exactly {} devices for the 8xH100 lane, found {}",
            thresholds.required_world_size,
            devices.len()
        ))
    } else if !all_devices_match_required_model {
        Some(format!(
            "selected inventory is not an exact `{}` CUDA posture with non-MIG devices",
            thresholds.required_device_name
        ))
    } else {
        None
    };
    let selection = match rejection_reason.as_ref() {
        Some(reason) => BackendSelection::refused(
            thresholds.required_backend.clone(),
            devices.first().cloned(),
            supported_ops,
            ServedProductBackendPolicy::same_backend_only(),
            ServedProductFallbackTrigger::RequestedBackendUnavailable,
            reason.clone(),
        ),
        None => BackendSelection::direct(
            thresholds.required_backend.clone(),
            devices.first().cloned(),
            supported_ops,
        ),
    }
    .with_selected_devices(devices.to_vec())
    .with_execution_topology(topology)
    .with_cluster_execution_capability_profile(capability_profile.clone());
    let refusal = rejection_reason.map(|reason| ParameterGolfDistributedLaneRefusal {
        refusal_kind: if capability_profile.runtime_backend != thresholds.required_backend
            || !capability_profile
                .supports_communication_class(ClusterCommunicationClass::TensorCollectiveMesh)
        {
            ParameterGolfDistributedLaneRefusalKind::CapabilityMismatch
        } else {
            ParameterGolfDistributedLaneRefusalKind::DeviceInventoryMismatch
        },
        reason,
        fallback_benchmark_ref: String::from(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF),
    });
    (selection, all_devices_match_required_model, refusal)
}

fn parse_inventory_quantity(
    raw_value: &str,
    expected_suffix: &str,
) -> Result<u64, ParameterGolfDistributedLaneError> {
    let trimmed = raw_value.trim();
    let value = trimmed
        .strip_suffix(expected_suffix)
        .ok_or_else(|| ParameterGolfDistributedLaneError::InvalidInventory {
            message: format!("expected `{trimmed}` to end with the suffix `{expected_suffix}`"),
        })?
        .trim();
    value.parse::<u64>().map_err(
        |error| ParameterGolfDistributedLaneError::InvalidInventory {
            message: format!("failed to parse inventory quantity `{trimmed}`: {error}"),
        },
    )
}

pub(crate) const PARAMETER_GOLF_H100_DEVICE_CAPACITY_TOLERANCE_BYTES: u64 = 512 * 1024 * 1024;

pub(crate) fn device_capacity_matches_h100_threshold(
    device: &DeviceDescriptor,
    thresholds: &ParameterGolfDistributedChallengeThresholds,
) -> bool {
    device.memory_capacity_bytes.is_some_and(|bytes| {
        bytes.saturating_add(PARAMETER_GOLF_H100_DEVICE_CAPACITY_TOLERANCE_BYTES)
            >= thresholds.max_peak_device_bytes_per_worker
    })
}

fn device_matches_h100(
    device: &DeviceDescriptor,
    thresholds: &ParameterGolfDistributedChallengeThresholds,
) -> bool {
    if device.backend != thresholds.required_backend || device.device.kind() != DeviceKind::Cuda {
        return false;
    }
    let name_matches = device
        .device_name
        .as_deref()
        .is_some_and(|name| name.contains(thresholds.required_device_name.as_str()));
    let metadata = match &device.nvidia_metadata {
        Some(metadata) => metadata,
        None => return false,
    };
    name_matches
        && metadata.topology.mig_profile.is_none()
        && !metadata.risk.mig_partitioned
        && device_capacity_matches_h100_threshold(device, thresholds)
}

fn build_communication_receipt(
    mesh_id: &str,
    axes: &[TrainingDeviceMeshAxis],
    devices: &[DeviceDescriptor],
    ddp_gradient_payload_bytes: u64,
    muon_update_payload_bytes: u64,
    validation_payload_bytes: u64,
) -> Result<ParameterGolfDistributedCommunicationReceipt, ParameterGolfDistributedLaneError> {
    let mut planner = ElasticCollectivePlanner::new(
        mesh_id,
        "cuda",
        ClusterCommunicationClass::TensorCollectiveMesh,
        axes.to_vec(),
    )
    .with_transport(ClusterTransportClass::Loopback);
    let members = devices
        .iter()
        .enumerate()
        .map(|(rank, device)| {
            let node_id = format!("rank-{rank}");
            let device_label = device.device.label().unwrap_or("cuda").to_string();
            CollectiveMeshMember::new(node_id, rank, rank, device_label)
        })
        .collect::<Vec<_>>();
    let elastic_membership = TrainingElasticMembershipContext::new(
        1,
        format!("{mesh_id}.cluster_state"),
        format!("{mesh_id}.topology"),
        (0..devices.len())
            .map(|rank| format!("rank-{rank}"))
            .collect(),
    );
    planner.observe_mesh(elastic_membership, members)?;
    let ddp_collective = planner.plan_collective(
        TrainingCollectiveKind::AllReduce,
        ddp_gradient_payload_bytes,
        TrainingCollectiveQuantization::None,
    )?;
    let muon_collective = planner.plan_collective(
        TrainingCollectiveKind::AllReduce,
        muon_update_payload_bytes,
        TrainingCollectiveQuantization::None,
    )?;
    let validation_collective = planner.plan_collective(
        TrainingCollectiveKind::AllReduce,
        validation_payload_bytes,
        TrainingCollectiveQuantization::None,
    )?;
    Ok(ParameterGolfDistributedCommunicationReceipt {
        communication_class: ClusterCommunicationClass::TensorCollectiveMesh,
        transport: ClusterTransportClass::Loopback,
        mesh_id: String::from(mesh_id),
        axes: axes.to_vec(),
        stages: vec![
            ParameterGolfDistributedCommunicationStageReceipt {
                stage_id: String::from("ddp_gradient_all_reduce"),
                collective_kind: ddp_collective.collective.kind,
                quantization: ddp_collective.collective.quantization,
                payload_bytes: ddp_collective.collective.payload_bytes,
                estimated_wire_bytes: ddp_collective.collective.estimated_wire_bytes,
                worker_count: ddp_collective.collective.worker_count,
                detail: String::from(
                    "PyTorch DDP-style gradient synchronization over the full BF16 model gradient surface once per optimizer step",
                ),
            },
            ParameterGolfDistributedCommunicationStageReceipt {
                stage_id: String::from("muon_matrix_update_all_reduce"),
                collective_kind: muon_collective.collective.kind,
                quantization: muon_collective.collective.quantization,
                payload_bytes: muon_collective.collective.payload_bytes,
                estimated_wire_bytes: muon_collective.collective.estimated_wire_bytes,
                worker_count: muon_collective.collective.worker_count,
                detail: String::from(
                    "Muon rank-local matrix updates are flattened into one BF16 buffer and all-reduced across the full 8-way mesh",
                ),
            },
            ParameterGolfDistributedCommunicationStageReceipt {
                stage_id: String::from("validation_metric_all_reduce"),
                collective_kind: validation_collective.collective.kind,
                quantization: validation_collective.collective.quantization,
                payload_bytes: validation_collective.collective.payload_bytes,
                estimated_wire_bytes: validation_collective.collective.estimated_wire_bytes,
                worker_count: validation_collective.collective.worker_count,
                detail: String::from(
                    "Validation loss, token count, and byte count are reduced as three f64 scalars across the full mesh",
                ),
            },
        ],
    })
}

fn build_memory_receipt(
    descriptor: &ParameterGolfModelDescriptor,
    hyperparameters: &ParameterGolfTrainingHyperparameters,
    geometry: &ParameterGolfBatchGeometry,
    thresholds: &ParameterGolfDistributedChallengeThresholds,
    total_parameter_count: u64,
    memory_observation: Option<&ParameterGolfDistributedMemoryObservation>,
) -> Result<ParameterGolfDistributedMemoryReceipt, ParameterGolfDistributedLaneError> {
    let optimizer_plan = parameter_golf_optimizer_plan(descriptor, hyperparameters)?;
    let parameter_groups = optimizer_plan
        .groups
        .iter()
        .map(|group| {
            let optimizer = match &group.execution {
                ParameterGolfOptimizerExecution::Adam { optimizer } => optimizer.clone(),
                ParameterGolfOptimizerExecution::Muon { optimizer } => {
                    TrainingOptimizerConfig::sgd(optimizer.learning_rate)
                        .with_momentum(optimizer.momentum)
                }
            };
            TrainingParameterGroupState::new(
                group.group_id.clone(),
                group.parameter_class,
                TrainingTensorBuffer::from_f32(
                    group.group_id.clone(),
                    TensorSpec::new(
                        Shape::new(vec![group.parameter_count]),
                        DType::F32,
                        Device::cpu(),
                    ),
                    vec![0.0; group.parameter_count],
                )?,
                optimizer,
                TrainingOptimizerResidencyPolicy::new(
                    OptimizerStateResidency::DeviceResident,
                    OptimizerStateResidency::DeviceResident,
                ),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let collective_sync_plan =
        build_collective_sync_plan(total_parameter_count, geometry.world_size)?;
    let contract = DistributedOptimizerContract::new(
        "optimizer://openagents/parameter_golf/distributed_8xh100",
        TrainingDistributedOptimizerKind::DataParallel,
        TrainingPrecisionPolicy::bf16_master_fp32(TrainingCollectiveQuantization::None),
        TrainingGradientAccumulationPolicy::new(
            geometry.grad_accum_steps as u32,
            TrainingGradientAccumulationReduction::Mean,
            TrainingCollectiveKind::AllReduce,
        ),
        TrainingActivationCheckpointPolicy::Disabled {
            activation_peak_bytes: estimated_activation_peak_bytes(descriptor, geometry),
        },
        DistributedTrainingMemoryBudget::new(u64::MAX / 4, u64::MAX / 4, 0),
        collective_sync_plan,
        optimizer_plan
            .groups
            .iter()
            .map(|group| {
                let placements = (0..geometry.world_size)
                    .map(|rank| {
                        TrainingShardPlacement::new(
                            rank,
                            "dp",
                            format!("rank-{rank}"),
                            format!("cuda:{rank}"),
                            rank,
                            TrainingShardRange::new(0, group.parameter_count),
                        )
                    })
                    .collect::<Vec<_>>();
                DistributedOptimizerGroupContract::new(
                    group.group_id.clone(),
                    group.parameter_class,
                    TrainingParameterShardLayout::new(
                        TrainingParameterShardKind::Replicated,
                        placements.clone(),
                    )
                    .with_axis_id("dp"),
                    TrainingParameterShardLayout::new(
                        TrainingParameterShardKind::Replicated,
                        placements.clone(),
                    )
                    .with_axis_id("dp"),
                    TrainingOptimizerStateShardLayout::new(
                        TrainingOptimizerStateShardKind::Replicated,
                        TrainingOptimizerShardResidency::DeviceResident,
                        placements,
                    )
                    .with_axis_id("dp"),
                    OptimizerStateResidency::DeviceResident,
                )
            })
            .collect(),
    )?;
    let run = DistributedOptimizerRun::new(
        "parameter-golf-distributed-memory-plan",
        PARAMETER_GOLF_DISTRIBUTED_CHECKPOINT_FAMILY,
        TrainingLoopBudget::new(1, 1, 1)?,
        parameter_groups,
        contract,
    )?;
    let memory_plan = run.memory_plan;
    let (measurement_posture, peak_device_bytes_per_worker, peak_host_bytes_per_worker) =
        match memory_observation {
            Some(observation) => (
                String::from("observed_runtime_peak_plus_analytic_state_breakdown"),
                observation.peak_device_bytes_per_worker,
                observation.peak_host_bytes_per_worker,
            ),
            None => (
                String::from("analytic_optimizer_contract_upper_bound"),
                memory_plan.peak_device_bytes_per_worker,
                memory_plan.peak_host_bytes_per_worker,
            ),
        };
    Ok(ParameterGolfDistributedMemoryReceipt {
        measurement_posture,
        declared_device_budget_bytes_per_worker: thresholds.max_peak_device_bytes_per_worker,
        declared_host_budget_bytes_per_worker: thresholds.max_peak_host_bytes_per_worker,
        parameter_logical_bytes: memory_plan.parameter_logical_bytes,
        gradient_logical_bytes: memory_plan.gradient_logical_bytes,
        optimizer_state_logical_bytes: memory_plan.optimizer_state_logical_bytes,
        master_weight_logical_bytes: memory_plan.master_weight_logical_bytes,
        activation_peak_bytes: memory_plan.activation_peak_bytes,
        activation_bytes_saved: memory_plan.activation_bytes_saved,
        peak_device_bytes_per_worker,
        peak_host_bytes_per_worker,
        remote_offloaded_optimizer_bytes: memory_plan.remote_offloaded_optimizer_bytes,
        within_device_budget: peak_device_bytes_per_worker
            <= thresholds.max_peak_device_bytes_per_worker,
        within_host_budget: peak_host_bytes_per_worker <= thresholds.max_peak_host_bytes_per_worker,
        planner_receipt_digest: memory_plan.receipt_digest,
    })
}

fn build_collective_sync_plan(
    total_parameter_count: u64,
    world_size: usize,
) -> Result<psionic_collectives::CollectiveSyncExecutionPlan, ParameterGolfDistributedLaneError> {
    let mesh_id = "mesh.parameter_golf.8xh100.contract";
    let axes = vec![TrainingDeviceMeshAxis::new(
        "dp",
        TrainingDeviceMeshAxisKind::DataParallel,
        world_size,
    )
    .with_collective_group_size(world_size)
    .with_detail("single data-parallel axis for the public 8xH100 lane")];
    let mut planner = ElasticCollectivePlanner::new(
        mesh_id,
        "cuda",
        ClusterCommunicationClass::TensorCollectiveMesh,
        axes,
    )
    .with_transport(ClusterTransportClass::Loopback);
    let members = (0..world_size)
        .map(|rank| {
            CollectiveMeshMember::new(format!("rank-{rank}"), rank, rank, format!("cuda:{rank}"))
        })
        .collect::<Vec<_>>();
    planner.observe_mesh(
        TrainingElasticMembershipContext::new(
            1,
            "parameter-golf-contract-cluster-state",
            "parameter-golf-contract-topology",
            (0..world_size).map(|rank| format!("rank-{rank}")).collect(),
        ),
        members,
    )?;
    Ok(planner.plan_sync(
        1,
        TrainingCollectiveKind::AllReduce,
        total_parameter_count.saturating_mul(2),
        TrainingCollectiveQuantization::None,
        &CollectiveSyncCadencePolicy::new(),
    )?)
}

fn estimated_activation_peak_bytes(
    descriptor: &ParameterGolfModelDescriptor,
    geometry: &ParameterGolfBatchGeometry,
) -> u64 {
    let tokens_per_worker = geometry.local_train_batch_tokens() as u64;
    let model_dim = descriptor.config.model_dim as u64;
    let layer_factor = (descriptor.config.num_layers as u64)
        .saturating_mul((4 + descriptor.config.mlp_mult) as u64);
    tokens_per_worker
        .saturating_mul(model_dim)
        .saturating_mul(2)
        .saturating_mul(layer_factor.max(1))
}

fn build_timing_receipt(
    config: &ParameterGolfDistributed8xH100Config,
) -> Option<ParameterGolfDistributedTimingReceipt> {
    if config.step_observations.is_empty() {
        return None;
    }
    let step_count = config.step_observations.len() as u64;
    let total_train_tokens = config
        .step_observations
        .iter()
        .map(|observation| observation.train_tokens)
        .sum::<u64>();
    let training_step_observed_ms = config
        .step_observations
        .iter()
        .map(ParameterGolfDistributedStepObservation::duration_ms)
        .sum::<u64>();
    let total_observed_ms = training_step_observed_ms
        .saturating_add(config.validation_observed_ms)
        .saturating_add(config.export_observed_ms);
    let tail_step_duration_ms = config
        .step_observations
        .iter()
        .map(ParameterGolfDistributedStepObservation::duration_ms)
        .max()
        .unwrap_or_default();
    let mean_step_duration_ms = if step_count == 0 {
        0
    } else {
        training_step_observed_ms / step_count
    };
    let train_tokens_per_second = if training_step_observed_ms == 0 {
        0
    } else {
        total_train_tokens.saturating_mul(1_000) / training_step_observed_ms
    };
    Some(ParameterGolfDistributedTimingReceipt {
        measurement_posture: String::from("observed_step_wallclock_plus_validation_and_export"),
        step_count,
        total_train_tokens,
        training_step_observed_ms,
        validation_observed_ms: config.validation_observed_ms,
        export_observed_ms: config.export_observed_ms,
        total_observed_ms,
        mean_step_duration_ms,
        tail_step_duration_ms,
        train_tokens_per_second,
        wallclock_cap_ms: config.thresholds.max_total_wallclock_ms,
        within_wallclock_cap: total_observed_ms <= config.thresholds.max_total_wallclock_ms,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ValidationObservationPlan {
    pub(crate) rank: usize,
    pub(crate) sequence_start: u64,
    pub(crate) sequence_count: u64,
    pub(crate) evaluation_unit_start: u64,
    pub(crate) evaluation_unit_count: u64,
    pub(crate) scored_token_start: u64,
    pub(crate) scored_token_count: u64,
}

pub(crate) fn build_validation_observation_plan(
    total_sequence_count: u64,
    sequence_length: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
    world_size: usize,
) -> Result<Vec<ValidationObservationPlan>, ParameterGolfDistributedLaneError> {
    if world_size == 0 {
        return Err(ParameterGolfDistributedLaneError::InvalidConfig {
            message: String::from("distributed validation requires world_size > 0"),
        });
    }
    let sequence_length = sequence_length as u64;
    let world_size_u64 = world_size as u64;
    match eval_mode {
        ParameterGolfValidationEvalMode::NonOverlapping => {
            let base = total_sequence_count / world_size_u64;
            let remainder = total_sequence_count % world_size_u64;
            let mut sequence_start = 0_u64;
            let mut plan = Vec::with_capacity(world_size);
            for rank in 0..world_size {
                let sequence_count = base + u64::from((rank as u64) < remainder);
                let scored_token_start = sequence_start.saturating_mul(sequence_length);
                let scored_token_count = sequence_count.saturating_mul(sequence_length);
                plan.push(ValidationObservationPlan {
                    rank,
                    sequence_start,
                    sequence_count,
                    evaluation_unit_start: sequence_start,
                    evaluation_unit_count: sequence_count,
                    scored_token_start,
                    scored_token_count,
                });
                sequence_start = sequence_start.saturating_add(sequence_count);
            }
            Ok(plan)
        }
        ParameterGolfValidationEvalMode::SlidingWindow { stride } => {
            let total_tokens = total_sequence_count.saturating_mul(sequence_length);
            let window_starts = crate::build_parameter_golf_validation_window_starts(
                total_tokens as usize,
                sequence_length as usize,
                *stride,
            );
            let total_units = window_starts.len() as u64;
            let base = total_units / world_size_u64;
            let remainder = total_units % world_size_u64;
            let mut evaluation_unit_start = 0_u64;
            let mut plan = Vec::with_capacity(world_size);
            for rank in 0..world_size {
                let evaluation_unit_count = base + u64::from((rank as u64) < remainder);
                let (scored_token_start, scored_token_count) = if evaluation_unit_count == 0 {
                    (total_tokens, 0)
                } else {
                    let first_window_start = window_starts[evaluation_unit_start as usize] as u64;
                    let last_window_start = window_starts
                        [(evaluation_unit_start + evaluation_unit_count - 1) as usize]
                        as u64;
                    let first_valid_length = (first_window_start + sequence_length)
                        .min(total_tokens)
                        - first_window_start;
                    let last_valid_length =
                        (last_window_start + sequence_length).min(total_tokens) - last_window_start;
                    let first_score_start = if first_window_start == 0 {
                        0
                    } else {
                        first_valid_length.saturating_sub(*stride as u64)
                    };
                    let scored_token_start = first_window_start + first_score_start;
                    let scored_token_end = last_window_start + last_valid_length;
                    (
                        scored_token_start,
                        scored_token_end.saturating_sub(scored_token_start),
                    )
                };
                let sequence_start = scored_token_start / sequence_length;
                let sequence_count = if scored_token_count == 0 {
                    0
                } else {
                    scored_token_start
                        .saturating_add(scored_token_count)
                        .div_ceil(sequence_length)
                        .saturating_sub(sequence_start)
                };
                plan.push(ValidationObservationPlan {
                    rank,
                    sequence_start,
                    sequence_count,
                    evaluation_unit_start,
                    evaluation_unit_count,
                    scored_token_start,
                    scored_token_count,
                });
                evaluation_unit_start = evaluation_unit_start.saturating_add(evaluation_unit_count);
            }
            Ok(plan)
        }
    }
}

fn build_validation_aggregation_receipt(
    config: &ParameterGolfDistributed8xH100Config,
) -> Result<
    Option<ParameterGolfDistributedValidationAggregationReceipt>,
    ParameterGolfDistributedLaneError,
> {
    if config.validation_shard_observations.is_empty() {
        return Ok(None);
    }
    let shard_plan = build_validation_observation_plan(
        config.validation_total_sequence_count,
        config.geometry.train_sequence_length,
        &config.validation_eval_mode,
        config.geometry.world_size,
    )?;
    let local_batch_sequences = config.validation_batch_sequences;
    let mut aggregated_loss_sum = 0.0_f64;
    let mut aggregated_token_count = 0_u64;
    let mut aggregated_byte_count = 0_u64;
    let mut observed_ms = 0_u64;
    let mut shards = Vec::with_capacity(config.validation_shard_observations.len());

    for (expected, observed) in shard_plan
        .iter()
        .zip(config.validation_shard_observations.iter())
    {
        if observed.rank != expected.rank
            || observed.sequence_start != expected.sequence_start
            || observed.sequence_count != expected.sequence_count
            || observed.evaluation_unit_start != expected.evaluation_unit_start
            || observed.evaluation_unit_count != expected.evaluation_unit_count
            || observed.scored_token_start != expected.scored_token_start
            || observed.scored_token_count != expected.scored_token_count
        {
            return Err(ParameterGolfDistributedLaneError::InvalidConfig {
                message: format!(
                    "validation shard observation for rank {} does not match the expected {:?} shard layout",
                    observed.rank, config.validation_eval_mode
                ),
            });
        }
        aggregated_loss_sum += observed.loss_sum;
        aggregated_token_count = aggregated_token_count.saturating_add(observed.token_count);
        aggregated_byte_count = aggregated_byte_count.saturating_add(observed.byte_count);
        observed_ms = observed_ms.max(observed.observed_ms);
        shards.push(ParameterGolfDistributedValidationShardReceipt {
            rank: observed.rank,
            sequence_start: observed.sequence_start,
            sequence_count: observed.sequence_count,
            evaluation_unit_start: observed.evaluation_unit_start,
            evaluation_unit_count: observed.evaluation_unit_count,
            scored_token_start: observed.scored_token_start,
            scored_token_count: observed.scored_token_count,
            local_batch_sequences,
            loss_sum: observed.loss_sum,
            token_count: observed.token_count,
            byte_count: observed.byte_count,
            observed_ms: observed.observed_ms,
        });
    }

    let mean_loss = aggregated_loss_sum / aggregated_token_count.max(1) as f64;
    let bits_per_byte = (mean_loss / std::f64::consts::LN_2)
        * (aggregated_token_count as f64 / aggregated_byte_count.max(1) as f64);

    Ok(Some(ParameterGolfDistributedValidationAggregationReceipt {
        measurement_posture: String::from("rank_local_validation_shards_plus_metric_all_reduce"),
        eval_mode: config.validation_eval_mode.as_str().to_string(),
        world_size: config.geometry.world_size,
        total_sequence_count: config.validation_total_sequence_count,
        total_evaluation_unit_count: shard_plan
            .iter()
            .map(|observation| observation.evaluation_unit_count)
            .sum(),
        local_batch_sequences,
        aggregated_loss_sum,
        aggregated_token_count,
        aggregated_byte_count,
        mean_loss,
        bits_per_byte,
        observed_ms,
        shards,
    }))
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = sha2::Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn extract_u64_after(line: &str, prefix: &str, suffix: &str) -> Option<u64> {
    extract_token_after(line, prefix, suffix)?.parse().ok()
}

fn extract_f64_after(line: &str, prefix: &str, suffix: &str) -> Option<f64> {
    extract_token_after(line, prefix, suffix)?.parse().ok()
}

fn extract_string_after(line: &str, prefix: &str, suffix: &str) -> Option<String> {
    Some(String::from(extract_token_after(line, prefix, suffix)?))
}

fn extract_token_after<'a>(line: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    let start = line.find(prefix)? + prefix.len();
    let tail = line[start..].trim_start();
    if suffix.is_empty() {
        return Some(tail.trim());
    }
    let end = tail.find(suffix)?;
    Some(tail[..end].trim())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use psionic_core::{DType, Device, DeviceKind};
    use psionic_models::{
        ModelDescriptor, ParameterGolfConfig, ParameterGolfDeterministicInitializer,
        ParameterGolfReferenceModel, ParameterGolfWeights,
    };
    use psionic_runtime::{
        BackendSelectionState, ClusterExecutionCapabilityProfile, DeviceDescriptor,
        NvidiaDeviceMetadata, NvidiaRecoveryAction, NvidiaRecoveryProfile, NvidiaRiskLevel,
        NvidiaRiskProfile, NvidiaTopologyInfo,
    };

    use crate::{
        benchmark_parameter_golf_distributed_8xh100,
        benchmark_parameter_golf_runpod_8xh100_from_measurements,
        build_parameter_golf_runpod_8xh100_measurements_from_train_log,
        device_capacity_matches_h100_threshold, parameter_golf_runpod_8xh100_capability_profile,
        parse_parameter_golf_runpod_8xh100_inventory, ParameterGolfBatchGeometry,
        ParameterGolfDistributed8xH100Config, ParameterGolfDistributedLaneError,
        ParameterGolfDistributedMemoryObservation, ParameterGolfDistributedStepObservation,
        ParameterGolfDistributedValidationShardObservation, ParameterGolfRunPod8xH100Measurements,
        ParameterGolfTrainingHyperparameters, PARAMETER_GOLF_DISTRIBUTED_8XH100_VERSION,
        PARAMETER_GOLF_RUNPOD_8XH100_MEASUREMENTS_VERSION,
    };
    use psionic_eval::{
        ParameterGolfDistributedLaneDisposition, ParameterGolfDistributedLaneRefusalKind,
    };

    fn sample_model() -> Result<ParameterGolfReferenceModel, Box<dyn Error>> {
        let config = ParameterGolfConfig {
            vocab_size: 16,
            num_layers: 2,
            model_dim: 8,
            num_heads: 2,
            num_kv_heads: 1,
            mlp_mult: 2,
            bigram_vocab_size: 0,
            bigram_dim: 128,
            mlp_activation: Default::default(),
            max_context: 16,
            tie_embeddings: true,
            tied_embed_init_std: 0.005,
            logit_softcap: 30.0,
            rope_base: 10_000.0,
            rope_rotary_dim: None,
            layer_norm_scale: Default::default(),
            xsa_last_n: 0,
            ve_dim: 128,
            ve_layer_indices: Vec::new(),
            qk_gain_init: 1.5,
        };
        let weights = ParameterGolfWeights::from_initializer(
            &config,
            ParameterGolfDeterministicInitializer::default(),
        )?;
        Ok(ParameterGolfReferenceModel::new(
            ModelDescriptor::new(
                "parameter-golf-distributed-test",
                "parameter_golf_decoder",
                "test",
            ),
            config,
            weights,
        )?)
    }

    fn sample_h100_device(ordinal: usize) -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(
                DeviceKind::Cuda,
                ordinal as u16,
                Some(format!("cuda:{ordinal}")),
            ),
            device_name: Some(String::from("NVIDIA H100 80GB HBM3")),
            supported_dtypes: vec![DType::F32, DType::BF16],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(80 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: Some(NvidiaDeviceMetadata {
                topology: NvidiaTopologyInfo {
                    architecture: Some(String::from("hopper")),
                    compute_capability: Some(String::from("9.0")),
                    pci_bdf: Some(format!("00000000:{:02x}:00.0", ordinal + 1)),
                    sm_count: Some(132),
                    vram_bytes: Some(80 * 1024 * 1024 * 1024),
                    mig_profile: None,
                },
                risk: NvidiaRiskProfile {
                    level: NvidiaRiskLevel::Standard,
                    display_attached: Some(false),
                    mig_partitioned: false,
                    warnings: Vec::new(),
                },
                recovery: NvidiaRecoveryProfile {
                    supports_gpu_reset: Some(true),
                    expected_actions: vec![
                        NvidiaRecoveryAction::ProcessRestart,
                        NvidiaRecoveryAction::GpuReset,
                        NvidiaRecoveryAction::RebootHost,
                    ],
                },
            }),
        }
    }

    fn sample_non_h100_device(ordinal: usize) -> DeviceDescriptor {
        DeviceDescriptor {
            device_name: Some(String::from("NVIDIA A100 80GB")),
            ..sample_h100_device(ordinal)
        }
    }

    fn capability_profile() -> ClusterExecutionCapabilityProfile {
        parameter_golf_runpod_8xh100_capability_profile()
    }

    fn measured_config() -> ParameterGolfDistributed8xH100Config {
        let mut config = ParameterGolfDistributed8xH100Config::challenge_defaults();
        config.run_id = format!(
            "parameter-golf-distributed-test-{}",
            PARAMETER_GOLF_DISTRIBUTED_8XH100_VERSION
        );
        config.mesh_id = String::from("mesh.parameter_golf.test");
        config.geometry = ParameterGolfBatchGeometry {
            world_size: 8,
            train_batch_tokens: 128,
            validation_batch_tokens: 128,
            train_sequence_length: 4,
            grad_accum_steps: 1,
        };
        config.thresholds.max_total_wallclock_ms = 250;
        config.step_observations = vec![
            ParameterGolfDistributedStepObservation::new(1, 0, 40, 128),
            ParameterGolfDistributedStepObservation::new(2, 40, 85, 128),
        ];
        config.memory_observation = None;
        config.validation_observed_ms = 10;
        // Commit a0688399 made the validation batch width configurable with a
        // challenge default of 64; this test's bespoke 4-sequence shards pin
        // the matching local batch width explicitly.
        config.validation_batch_sequences = 4;
        config.validation_total_sequence_count = 32;
        config.validation_shard_observations = (0..8)
            .map(|rank| ParameterGolfDistributedValidationShardObservation {
                rank,
                sequence_start: (rank * 4) as u64,
                sequence_count: 4,
                evaluation_unit_start: (rank * 4) as u64,
                evaluation_unit_count: 4,
                scored_token_start: (rank * 16) as u64,
                scored_token_count: 16,
                loss_sum: 8.0 + rank as f64,
                token_count: 16,
                byte_count: 12,
                observed_ms: 9 + rank as u64,
            })
            .collect();
        config.export_observed_ms = 5;
        config
    }

    #[test]
    fn parameter_golf_distributed_8xh100_lane_measures_ddp_posture() -> Result<(), Box<dyn Error>> {
        let model = sample_model()?;
        let devices = (0..8).map(sample_h100_device).collect::<Vec<_>>();
        let receipt = benchmark_parameter_golf_distributed_8xh100(
            model.descriptor(),
            &ParameterGolfTrainingHyperparameters::baseline_defaults(),
            devices.as_slice(),
            &capability_profile(),
            &measured_config(),
        )?;
        assert_eq!(
            receipt.disposition,
            ParameterGolfDistributedLaneDisposition::Measured
        );
        assert_eq!(
            receipt.topology.backend_selection.selection_state,
            BackendSelectionState::Direct
        );
        assert!(receipt.topology.all_devices_match_required_model);
        assert_eq!(receipt.communication.axes.len(), 1);
        assert_eq!(receipt.communication.axes[0].axis_id, "dp");
        assert_eq!(receipt.communication.axes[0].extent, 8);
        assert_eq!(receipt.communication.stages.len(), 3);
        assert_eq!(
            receipt.communication.stages[0].stage_id,
            "ddp_gradient_all_reduce"
        );
        assert_eq!(
            receipt.communication.stages[1].stage_id,
            "muon_matrix_update_all_reduce"
        );
        assert!(!receipt.training_capability_report_digest.is_empty());
        assert!(receipt.challenge_kernel_blockers.is_empty());
        assert!(!receipt.boundary_notes.is_empty());
        assert!(!receipt
            .boundary_notes
            .iter()
            .any(|note| note.contains("cuda_bf16_train_graph_and_optimizer_surface")));
        assert!(receipt
            .timing
            .as_ref()
            .is_some_and(|timing| timing.within_wallclock_cap));
        let validation = receipt
            .validation_aggregation
            .as_ref()
            .expect("validation aggregation should be present");
        assert_eq!(validation.world_size, 8);
        assert_eq!(validation.total_sequence_count, 32);
        assert_eq!(validation.local_batch_sequences, 4);
        assert_eq!(validation.shards.len(), 8);
        assert_eq!(validation.shards[0].sequence_start, 0);
        assert_eq!(validation.shards[7].sequence_start, 28);
        assert!(receipt
            .memory
            .as_ref()
            .is_some_and(|memory| memory.within_device_budget));
        assert!(receipt.refusal.is_none());
        Ok(())
    }

    #[test]
    fn parameter_golf_distributed_8xh100_lane_refuses_non_h100_inventory(
    ) -> Result<(), Box<dyn Error>> {
        let model = sample_model()?;
        let devices = (0..8).map(sample_non_h100_device).collect::<Vec<_>>();
        let receipt = benchmark_parameter_golf_distributed_8xh100(
            model.descriptor(),
            &ParameterGolfTrainingHyperparameters::baseline_defaults(),
            devices.as_slice(),
            &capability_profile(),
            &measured_config(),
        )?;
        assert_eq!(
            receipt.disposition,
            ParameterGolfDistributedLaneDisposition::Refused
        );
        let refusal = receipt.refusal.expect("refusal should be present");
        assert_eq!(
            refusal.refusal_kind,
            ParameterGolfDistributedLaneRefusalKind::DeviceInventoryMismatch
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_distributed_8xh100_lane_refuses_when_wallclock_exceeds_cap(
    ) -> Result<(), Box<dyn Error>> {
        let model = sample_model()?;
        let devices = (0..8).map(sample_h100_device).collect::<Vec<_>>();
        let mut config = measured_config();
        config.thresholds.max_total_wallclock_ms = 80;
        let receipt = benchmark_parameter_golf_distributed_8xh100(
            model.descriptor(),
            &ParameterGolfTrainingHyperparameters::baseline_defaults(),
            devices.as_slice(),
            &capability_profile(),
            &config,
        )?;
        assert_eq!(
            receipt.disposition,
            ParameterGolfDistributedLaneDisposition::Refused
        );
        assert!(receipt.timing.is_some());
        let refusal = receipt.refusal.expect("refusal should be present");
        assert_eq!(
            refusal.refusal_kind,
            ParameterGolfDistributedLaneRefusalKind::WallclockExceeded
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_distributed_8xh100_lane_uses_observed_runtime_memory_when_supplied(
    ) -> Result<(), Box<dyn Error>> {
        let model = sample_model()?;
        let devices = (0..8).map(sample_h100_device).collect::<Vec<_>>();
        let mut config = measured_config();
        config.memory_observation = Some(ParameterGolfDistributedMemoryObservation {
            peak_device_bytes_per_worker: 70 * 1024 * 1024 * 1024,
            peak_host_bytes_per_worker: 8 * 1024 * 1024 * 1024,
            source: String::from("runpod cuda allocator telemetry"),
        });
        let receipt = benchmark_parameter_golf_distributed_8xh100(
            model.descriptor(),
            &ParameterGolfTrainingHyperparameters::baseline_defaults(),
            devices.as_slice(),
            &capability_profile(),
            &config,
        )?;
        let memory = receipt.memory.expect("memory receipt should be present");
        assert_eq!(
            memory.measurement_posture,
            "observed_runtime_peak_plus_analytic_state_breakdown"
        );
        assert_eq!(memory.peak_device_bytes_per_worker, 70 * 1024 * 1024 * 1024);
        assert_eq!(memory.peak_host_bytes_per_worker, 8 * 1024 * 1024 * 1024);
        assert!(memory.within_device_budget);
        assert!(memory.within_host_budget);
        assert!(receipt
            .boundary_notes
            .iter()
            .any(|note| note.contains("runpod cuda allocator telemetry")));
        Ok(())
    }

    #[test]
    fn parameter_golf_distributed_8xh100_lane_refuses_observed_runtime_memory_overflow(
    ) -> Result<(), Box<dyn Error>> {
        let model = sample_model()?;
        let devices = (0..8).map(sample_h100_device).collect::<Vec<_>>();
        let mut config = measured_config();
        config.memory_observation = Some(ParameterGolfDistributedMemoryObservation {
            peak_device_bytes_per_worker: 96 * 1024 * 1024 * 1024,
            peak_host_bytes_per_worker: 8 * 1024 * 1024 * 1024,
            source: String::from("synthetic over-budget runtime telemetry"),
        });
        let receipt = benchmark_parameter_golf_distributed_8xh100(
            model.descriptor(),
            &ParameterGolfTrainingHyperparameters::baseline_defaults(),
            devices.as_slice(),
            &capability_profile(),
            &config,
        )?;
        assert_eq!(
            receipt.disposition,
            ParameterGolfDistributedLaneDisposition::Refused
        );
        let refusal = receipt.refusal.expect("refusal should be present");
        assert_eq!(
            refusal.refusal_kind,
            ParameterGolfDistributedLaneRefusalKind::MemoryBudgetExceeded
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_distributed_8xh100_lane_rejects_invalid_geometry(
    ) -> Result<(), Box<dyn Error>> {
        let model = sample_model()?;
        let devices = (0..8).map(sample_h100_device).collect::<Vec<_>>();
        let mut config = measured_config();
        config.geometry.grad_accum_steps = 2;
        let error = benchmark_parameter_golf_distributed_8xh100(
            model.descriptor(),
            &ParameterGolfTrainingHyperparameters::baseline_defaults(),
            devices.as_slice(),
            &capability_profile(),
            &config,
        )
        .expect_err("invalid geometry should be rejected");
        assert!(matches!(
            error,
            ParameterGolfDistributedLaneError::InvalidConfig { .. }
        ));
        Ok(())
    }

    #[test]
    fn parameter_golf_distributed_8xh100_lane_rejects_invalid_validation_shard_layout(
    ) -> Result<(), Box<dyn Error>> {
        let model = sample_model()?;
        let devices = (0..8).map(sample_h100_device).collect::<Vec<_>>();
        let mut config = measured_config();
        config.validation_shard_observations[0].sequence_count = 5;
        let error = benchmark_parameter_golf_distributed_8xh100(
            model.descriptor(),
            &ParameterGolfTrainingHyperparameters::baseline_defaults(),
            devices.as_slice(),
            &capability_profile(),
            &config,
        )
        .expect_err("invalid validation shard layout should be rejected");
        assert!(matches!(
            error,
            ParameterGolfDistributedLaneError::InvalidConfig { .. }
        ));
        Ok(())
    }

    #[test]
    fn runpod_8xh100_inventory_parser_builds_cuda_h100_devices() -> Result<(), Box<dyn Error>> {
        let inventory = "\
0, NVIDIA H100 80GB HBM3, 81443 MiB, 1024 MiB, 99 %\n\
1, NVIDIA H100 80GB HBM3, 81443 MiB, 2048 MiB, 31 %\n";
        let devices = parse_parameter_golf_runpod_8xh100_inventory(inventory)?;
        assert_eq!(devices.len(), 2);
        assert_eq!(
            devices[0].device_name.as_deref(),
            Some("NVIDIA H100 80GB HBM3")
        );
        assert_eq!(
            devices[0].memory_capacity_bytes,
            Some(80 * 1024 * 1024 * 1024)
        );
        assert_eq!(devices[0].device.label(), Some("cuda:0"));
        assert_eq!(
            devices[0]
                .nvidia_metadata
                .as_ref()
                .expect("nvidia metadata")
                .topology
                .compute_capability
                .as_deref(),
            Some("9.0")
        );
        Ok(())
    }

    #[test]
    fn distributed_h100_capacity_matcher_accepts_real_runpod_inventory_delta() {
        let mut device = sample_h100_device(0);
        device.memory_capacity_bytes = Some(81_559 * 1024 * 1024);
        assert!(device_capacity_matches_h100_threshold(
            &device,
            &psionic_eval::ParameterGolfDistributedChallengeThresholds::challenge_8xh100()
        ));
    }

    #[test]
    fn runpod_8xh100_measurement_bridge_builds_measured_receipt() -> Result<(), Box<dyn Error>> {
        let model = sample_model()?;
        let inventory = (0..8)
            .map(|ordinal| format!("{ordinal}, NVIDIA H100 80GB HBM3, 81443 MiB, 1024 MiB, 99 %"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut measurements = ParameterGolfRunPod8xH100Measurements::challenge_defaults();
        measurements.schema_version =
            String::from(PARAMETER_GOLF_RUNPOD_8XH100_MEASUREMENTS_VERSION);
        measurements.step_observations = vec![
            ParameterGolfDistributedStepObservation::new(1, 0, 40, 128),
            ParameterGolfDistributedStepObservation::new(2, 40, 80, 128),
        ];
        measurements.validation_observed_ms = 10;
        measurements.validation_total_sequence_count = 32;
        // The bridge keeps the challenge geometry (sequence_length 1024), so
        // the NonOverlapping plan from commit a0688399 expects each rank's
        // scored-token span to cover sequence_count * 1024 tokens.
        measurements.validation_shard_observations = (0..8)
            .map(|rank| ParameterGolfDistributedValidationShardObservation {
                rank,
                sequence_start: (rank * 4) as u64,
                sequence_count: 4,
                evaluation_unit_start: (rank * 4) as u64,
                evaluation_unit_count: 4,
                scored_token_start: (rank * 4 * 1024) as u64,
                scored_token_count: 4 * 1024,
                loss_sum: 8.0 + rank as f64,
                token_count: 4096,
                byte_count: 3072,
                observed_ms: 9 + rank as u64,
            })
            .collect();
        measurements.export_observed_ms = 5;
        measurements.memory_observation = Some(ParameterGolfDistributedMemoryObservation {
            peak_device_bytes_per_worker: 70 * 1024 * 1024 * 1024,
            peak_host_bytes_per_worker: 8 * 1024 * 1024 * 1024,
            source: String::from("runpod operator measurement"),
        });
        let receipt = benchmark_parameter_golf_runpod_8xh100_from_measurements(
            model.descriptor(),
            &ParameterGolfTrainingHyperparameters::baseline_defaults(),
            "parameter-golf-runpod-test",
            &inventory,
            measurements,
        )?;
        assert_eq!(
            receipt.disposition,
            ParameterGolfDistributedLaneDisposition::Measured
        );
        assert!(receipt.refusal.is_none());
        assert_eq!(receipt.topology.selected_device_names.len(), 8);
        assert_eq!(
            receipt.topology.selected_device_names[0],
            "NVIDIA H100 80GB HBM3"
        );
        assert!(receipt.validation_aggregation.is_some());
        assert!(receipt
            .memory
            .as_ref()
            .is_some_and(|memory| memory.measurement_posture
                == "observed_runtime_peak_plus_analytic_state_breakdown"));
        Ok(())
    }

    #[test]
    fn runpod_8xh100_measurement_builder_parses_train_log() -> Result<(), Box<dyn Error>> {
        let log = "\
step:1/20000 train_loss:8.2000 train_time:50ms step_avg:50.00ms\n\
step:3/20000 train_loss:8.0000 train_time:170ms step_avg:56.67ms\n\
step:3/20000 val_loss:7.9000 val_bpb:1.2345 train_time:205ms step_avg:68.33ms\n\
distributed_validation_rank_complete rank=0 sequence_start=0 sequence_count=8 loss_sum=12.0000 token_count=8192 byte_count=6144 elapsed_ms=31\n\
distributed_validation_rank_complete rank=1 sequence_start=8 sequence_count=8 loss_sum=12.5000 token_count=8192 byte_count=6144 elapsed_ms=33\n\
distributed_validation_rank_complete rank=2 sequence_start=16 sequence_count=8 loss_sum=13.0000 token_count=8192 byte_count=6144 elapsed_ms=35\n\
distributed_validation_rank_complete rank=3 sequence_start=24 sequence_count=8 loss_sum=13.5000 token_count=8192 byte_count=6144 elapsed_ms=37\n\
distributed_validation_rank_complete rank=4 sequence_start=32 sequence_count=8 loss_sum=14.0000 token_count=8192 byte_count=6144 elapsed_ms=39\n\
distributed_validation_rank_complete rank=5 sequence_start=40 sequence_count=8 loss_sum=14.5000 token_count=8192 byte_count=6144 elapsed_ms=41\n\
distributed_validation_rank_complete rank=6 sequence_start=48 sequence_count=8 loss_sum=15.0000 token_count=8192 byte_count=6144 elapsed_ms=43\n\
distributed_validation_rank_complete rank=7 sequence_start=56 sequence_count=8 loss_sum=15.5000 token_count=8192 byte_count=6144 elapsed_ms=45\n\
peak memory allocated: 11273 MiB reserved: 11438 MiB\n\
final_int8_zlib_roundtrip val_loss:7.8000 val_bpb:1.2100 eval_time:1530ms\n";
        let measurements = build_parameter_golf_runpod_8xh100_measurements_from_train_log(
            log,
            Some("parameter-golf-runpod-test"),
            Some("mesh.parameter_golf.runpod_8xh100"),
            Some("synthetic train_gpt.py peak memory"),
        )?;
        assert_eq!(
            measurements.run_id.as_deref(),
            Some("parameter-golf-runpod-test")
        );
        assert_eq!(measurements.step_observations.len(), 3);
        assert_eq!(
            measurements.step_observations[0],
            ParameterGolfDistributedStepObservation::new(1, 0, 50, 524_288)
        );
        assert_eq!(
            measurements.step_observations[1],
            ParameterGolfDistributedStepObservation::new(2, 50, 110, 524_288)
        );
        assert_eq!(
            measurements.step_observations[2],
            ParameterGolfDistributedStepObservation::new(3, 110, 170, 524_288)
        );
        assert_eq!(measurements.validation_total_sequence_count, 64);
        assert_eq!(measurements.validation_shard_observations.len(), 8);
        assert_eq!(measurements.validation_shard_observations[0].rank, 0);
        assert_eq!(
            measurements.validation_shard_observations[7].sequence_start,
            56
        );
        assert_eq!(measurements.validation_observed_ms, 45);
        assert_eq!(measurements.export_observed_ms, 1_530);
        assert_eq!(
            measurements
                .memory_observation
                .as_ref()
                .expect("memory observation")
                .peak_device_bytes_per_worker,
            11_438 * 1024 * 1024
        );
        Ok(())
    }

    #[test]
    fn runpod_8xh100_measurement_builder_rejects_duplicate_validation_ranks() {
        let log = "\
step:1/1 train_loss:8.2000 train_time:50ms step_avg:50.00ms\n\
distributed_validation_rank_complete rank=0 sequence_start=0 sequence_count=8 loss_sum=12.0000 token_count=8192 byte_count=6144 elapsed_ms=31\n\
distributed_validation_rank_complete rank=0 sequence_start=8 sequence_count=8 loss_sum=12.5000 token_count=8192 byte_count=6144 elapsed_ms=33\n";
        let error =
            build_parameter_golf_runpod_8xh100_measurements_from_train_log(log, None, None, None)
                .expect_err("duplicate distributed validation ranks should be rejected");
        assert!(matches!(
            error,
            ParameterGolfDistributedLaneError::InvalidMeasurements { .. }
        ));
    }

    #[test]
    fn runpod_8xh100_measurement_builder_rejects_logs_without_train_steps() {
        let error = build_parameter_golf_runpod_8xh100_measurements_from_train_log(
            "final_int8_zlib_roundtrip val_loss:7.8 val_bpb:1.21 eval_time:1530ms\n",
            None,
            None,
            None,
        )
        .expect_err("log without train steps should be rejected");
        assert!(matches!(
            error,
            ParameterGolfDistributedLaneError::InvalidMeasurements { .. }
        ));
    }
}
