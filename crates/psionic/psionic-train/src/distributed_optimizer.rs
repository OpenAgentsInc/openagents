use std::collections::BTreeMap;

use psionic_collectives::CollectiveSyncExecutionPlan;
use psionic_core::{DType, TensorData};
use psionic_runtime::{TrainingCollectiveKind, TrainingCollectiveQuantization};
use serde::{Deserialize, Serialize};
use serde_json::to_vec;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FixedBudgetTrainingRun, OptimizerStateResidency, TrainingGradientBatch, TrainingParameterClass,
    TrainingParameterGroupState, TrainingStepInput, TrainingStepReceipt,
    core_loop::TrainingCoreError,
};

/// Distributed optimizer family owned by the train runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingDistributedOptimizerKind {
    /// Pure data-parallel optimizer state replication.
    DataParallel,
    /// ZeRO stage 1 style optimizer-state partitioning.
    ZeroStage1,
    /// ZeRO stage 2 style optimizer-state and gradient partitioning.
    ZeroStage2,
    /// ZeRO stage 3 or FSDP-style full parameter, gradient, and optimizer partitioning.
    ZeroStage3,
    /// Hybrid tensor-parallel plus data-parallel sharding.
    HybridTensorDataParallel,
}

/// Precision family exposed by the distributed optimizer contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingPrecisionMode {
    Fp32,
    Fp16,
    Bf16,
    Int8,
}

impl TrainingPrecisionMode {
    /// Returns the size of one element in bytes.
    #[must_use]
    pub const fn element_size_bytes(self) -> usize {
        match self {
            Self::Fp32 => 4,
            Self::Fp16 | Self::Bf16 => 2,
            Self::Int8 => 1,
        }
    }

    fn matches_dtype(self, dtype: DType) -> bool {
        matches!(
            (self, dtype),
            (Self::Fp32, DType::F32)
                | (Self::Fp16, DType::F16)
                | (Self::Bf16, DType::BF16)
                | (Self::Int8, DType::I8)
        )
    }
}

/// Reduction mode applied while accumulating microbatches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingGradientAccumulationReduction {
    Sum,
    Mean,
}

/// Precision policy for distributed optimizer execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingPrecisionPolicy {
    /// Precision used for train-visible parameters.
    pub parameter_precision: TrainingPrecisionMode,
    /// Precision used for gradient buffers.
    pub gradient_precision: TrainingPrecisionMode,
    /// Precision used for optimizer state.
    pub optimizer_state_precision: TrainingPrecisionMode,
    /// Precision used for master weights when one exists.
    pub master_weight_precision: TrainingPrecisionMode,
    /// Precision used while reducing or synchronizing collective payloads.
    pub reduction_precision: TrainingPrecisionMode,
    /// Collective quantization applied during mesh sync.
    pub communication_quantization: TrainingCollectiveQuantization,
    /// Whether stochastic rounding is required when downcasting.
    pub stochastic_rounding: bool,
    /// Optional dynamic loss scale value for low-precision gradients.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loss_scale: Option<u64>,
}

impl TrainingPrecisionPolicy {
    /// Returns a BF16-forward, FP32-master policy.
    #[must_use]
    pub const fn bf16_master_fp32(
        communication_quantization: TrainingCollectiveQuantization,
    ) -> Self {
        Self {
            parameter_precision: TrainingPrecisionMode::Bf16,
            gradient_precision: TrainingPrecisionMode::Bf16,
            optimizer_state_precision: TrainingPrecisionMode::Fp32,
            master_weight_precision: TrainingPrecisionMode::Fp32,
            reduction_precision: TrainingPrecisionMode::Fp32,
            communication_quantization,
            stochastic_rounding: true,
            loss_scale: None,
        }
    }
}

/// Activation checkpointing or rematerialization mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum TrainingActivationCheckpointPolicy {
    /// No rematerialization.
    Disabled {
        /// Peak activation bytes without checkpointing.
        activation_peak_bytes: u64,
    },
    /// Checkpoint activations every fixed block interval.
    EveryNthBlock {
        /// Interval between checkpointed regions.
        block_interval: u32,
        /// Peak activation bytes without checkpointing.
        activation_peak_bytes_without_checkpointing: u64,
        /// Peak activation bytes with checkpointing enabled.
        activation_peak_bytes_with_checkpointing: u64,
        /// Additional recompute overhead in basis points.
        rematerialization_overhead_bps: u16,
    },
    /// Checkpoint explicitly named regions.
    NamedRegions {
        /// Regions selected for rematerialization.
        checkpointed_regions: Vec<String>,
        /// Peak activation bytes without checkpointing.
        activation_peak_bytes_without_checkpointing: u64,
        /// Peak activation bytes with checkpointing enabled.
        activation_peak_bytes_with_checkpointing: u64,
        /// Additional recompute overhead in basis points.
        rematerialization_overhead_bps: u16,
    },
}

impl TrainingActivationCheckpointPolicy {
    fn activation_peak_bytes(self: &Self) -> u64 {
        match self {
            Self::Disabled {
                activation_peak_bytes,
            } => *activation_peak_bytes,
            Self::EveryNthBlock {
                activation_peak_bytes_with_checkpointing,
                ..
            }
            | Self::NamedRegions {
                activation_peak_bytes_with_checkpointing,
                ..
            } => *activation_peak_bytes_with_checkpointing,
        }
    }

    fn activation_bytes_saved(self: &Self) -> u64 {
        match self {
            Self::Disabled { .. } => 0,
            Self::EveryNthBlock {
                activation_peak_bytes_without_checkpointing,
                activation_peak_bytes_with_checkpointing,
                ..
            }
            | Self::NamedRegions {
                activation_peak_bytes_without_checkpointing,
                activation_peak_bytes_with_checkpointing,
                ..
            } => activation_peak_bytes_without_checkpointing
                .saturating_sub(*activation_peak_bytes_with_checkpointing),
        }
    }
}

/// Gradient accumulation discipline for distributed optimizer steps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingGradientAccumulationPolicy {
    /// Number of microbatches required before an optimizer step may flush.
    pub microbatch_count: u32,
    /// Reduction mode used while combining microbatch gradients.
    pub reduction: TrainingGradientAccumulationReduction,
    /// Collective kind used when flushing gradient state across the mesh.
    pub flush_collective_kind: TrainingCollectiveKind,
}

impl TrainingGradientAccumulationPolicy {
    /// Creates an explicit accumulation contract.
    pub const fn new(
        microbatch_count: u32,
        reduction: TrainingGradientAccumulationReduction,
        flush_collective_kind: TrainingCollectiveKind,
    ) -> Self {
        Self {
            microbatch_count,
            reduction,
            flush_collective_kind,
        }
    }
}

/// Residency class for optimizer-state shards in the distributed plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingOptimizerShardResidency {
    DeviceResident,
    HostOffloaded,
    RemoteOffloaded,
}

/// Address range owned by one shard placement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingShardRange {
    /// Logical element offset from the start of the tensor or state buffer.
    pub offset_elements: usize,
    /// Number of elements owned by this shard.
    pub element_count: usize,
}

impl TrainingShardRange {
    /// Creates a contiguous element range.
    #[must_use]
    pub const fn new(offset_elements: usize, element_count: usize) -> Self {
        Self {
            offset_elements,
            element_count,
        }
    }
}

/// One node or device placement for a parameter, gradient, or optimizer-state shard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingShardPlacement {
    /// Stable shard identifier.
    pub shard_id: usize,
    /// Stable mesh axis carrying this shard.
    pub axis_id: String,
    /// Stable worker or node identifier.
    pub node_id: String,
    /// Stable device label within the node when one exists.
    pub device_label: String,
    /// Replica ordinal under the current sharding posture.
    pub replica_ordinal: usize,
    /// Contiguous tensor or state range realized by this shard.
    pub range: TrainingShardRange,
}

impl TrainingShardPlacement {
    /// Creates one shard placement.
    #[must_use]
    pub fn new(
        shard_id: usize,
        axis_id: impl Into<String>,
        node_id: impl Into<String>,
        device_label: impl Into<String>,
        replica_ordinal: usize,
        range: TrainingShardRange,
    ) -> Self {
        Self {
            shard_id,
            axis_id: axis_id.into(),
            node_id: node_id.into(),
            device_label: device_label.into(),
            replica_ordinal,
            range,
        }
    }
}

/// Parameter sharding family for one parameter or gradient tensor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingParameterShardKind {
    Replicated,
    FullShard,
    TensorParallel,
    PipelineStagePinned,
}

/// Optimizer-state sharding family for one parameter group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingOptimizerStateShardKind {
    Replicated,
    ZeroStage1,
    ZeroStage2,
    ZeroStage3,
}

/// Explicit sharding contract for one parameter or gradient tensor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingParameterShardLayout {
    /// Sharding posture.
    pub kind: TrainingParameterShardKind,
    /// Mesh axis implementing this layout when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis_id: Option<String>,
    /// Ordered shard placements.
    pub placements: Vec<TrainingShardPlacement>,
}

impl TrainingParameterShardLayout {
    /// Creates a layout from explicit kind and placements.
    #[must_use]
    pub fn new(kind: TrainingParameterShardKind, placements: Vec<TrainingShardPlacement>) -> Self {
        Self {
            kind,
            axis_id: None,
            placements,
        }
    }

    /// Attaches the governing mesh axis.
    #[must_use]
    pub fn with_axis_id(mut self, axis_id: impl Into<String>) -> Self {
        self.axis_id = Some(axis_id.into());
        self
    }
}

/// Explicit sharding and residency contract for one optimizer-state family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingOptimizerStateShardLayout {
    /// Optimizer-state sharding posture.
    pub kind: TrainingOptimizerStateShardKind,
    /// Residency for the sharded state.
    pub residency: TrainingOptimizerShardResidency,
    /// Mesh axis implementing this layout when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis_id: Option<String>,
    /// Ordered state-shard placements.
    pub placements: Vec<TrainingShardPlacement>,
}

impl TrainingOptimizerStateShardLayout {
    /// Creates an optimizer-state layout from explicit kind and placements.
    #[must_use]
    pub fn new(
        kind: TrainingOptimizerStateShardKind,
        residency: TrainingOptimizerShardResidency,
        placements: Vec<TrainingShardPlacement>,
    ) -> Self {
        Self {
            kind,
            residency,
            axis_id: None,
            placements,
        }
    }

    /// Attaches the governing mesh axis.
    #[must_use]
    pub fn with_axis_id(mut self, axis_id: impl Into<String>) -> Self {
        self.axis_id = Some(axis_id.into());
        self
    }
}

/// Distributed optimizer contract for one parameter group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedOptimizerGroupContract {
    /// Stable parameter-group identifier.
    pub group_id: String,
    /// High-level parameter family.
    pub class: TrainingParameterClass,
    /// Parameter sharding contract.
    pub parameter_layout: TrainingParameterShardLayout,
    /// Gradient-buffer sharding contract.
    pub gradient_layout: TrainingParameterShardLayout,
    /// Optimizer-state sharding contract.
    pub optimizer_state_layout: TrainingOptimizerStateShardLayout,
    /// Residency for FP32 or higher-precision master weights.
    pub master_weight_residency: OptimizerStateResidency,
}

impl DistributedOptimizerGroupContract {
    /// Creates a group contract from explicit layouts.
    #[must_use]
    pub fn new(
        group_id: impl Into<String>,
        class: TrainingParameterClass,
        parameter_layout: TrainingParameterShardLayout,
        gradient_layout: TrainingParameterShardLayout,
        optimizer_state_layout: TrainingOptimizerStateShardLayout,
        master_weight_residency: OptimizerStateResidency,
    ) -> Self {
        Self {
            group_id: group_id.into(),
            class,
            parameter_layout,
            gradient_layout,
            optimizer_state_layout,
            master_weight_residency,
        }
    }
}

/// Budget envelope for long-running distributed training memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedTrainingMemoryBudget {
    /// Maximum device-resident bytes allowed per worker.
    pub device_budget_bytes: u64,
    /// Maximum host-resident bytes allowed per worker.
    pub host_budget_bytes: u64,
    /// Scratch or workspace reserve bytes that must be kept available.
    pub scratch_reserve_bytes: u64,
}

impl DistributedTrainingMemoryBudget {
    /// Creates a memory-budget contract.
    #[must_use]
    pub const fn new(
        device_budget_bytes: u64,
        host_budget_bytes: u64,
        scratch_reserve_bytes: u64,
    ) -> Self {
        Self {
            device_budget_bytes,
            host_budget_bytes,
            scratch_reserve_bytes,
        }
    }
}

/// Full distributed optimizer contract layered on top of the fixed-budget core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedOptimizerContract {
    /// Stable contract identifier.
    pub optimizer_id: String,
    /// Distributed optimizer family.
    pub optimizer_kind: TrainingDistributedOptimizerKind,
    /// Precision policy for the run.
    pub precision_policy: TrainingPrecisionPolicy,
    /// Gradient accumulation policy.
    pub accumulation_policy: TrainingGradientAccumulationPolicy,
    /// Activation checkpointing or rematerialization policy.
    pub activation_checkpointing: TrainingActivationCheckpointPolicy,
    /// Explicit long-run memory budget.
    pub memory_budget: DistributedTrainingMemoryBudget,
    /// Mesh-wide collective sync plan bound to optimizer flush.
    pub collective_sync_plan: CollectiveSyncExecutionPlan,
    /// Per-group sharding contracts.
    pub groups: Vec<DistributedOptimizerGroupContract>,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl DistributedOptimizerContract {
    /// Creates a contract and derives its digest.
    pub fn new(
        optimizer_id: impl Into<String>,
        optimizer_kind: TrainingDistributedOptimizerKind,
        precision_policy: TrainingPrecisionPolicy,
        accumulation_policy: TrainingGradientAccumulationPolicy,
        activation_checkpointing: TrainingActivationCheckpointPolicy,
        memory_budget: DistributedTrainingMemoryBudget,
        collective_sync_plan: CollectiveSyncExecutionPlan,
        groups: Vec<DistributedOptimizerGroupContract>,
    ) -> Result<Self, DistributedOptimizerError> {
        let optimizer_id = optimizer_id.into();
        if optimizer_id.trim().is_empty() {
            return Err(DistributedOptimizerError::MissingOptimizerId);
        }
        if accumulation_policy.microbatch_count == 0 {
            return Err(DistributedOptimizerError::InvalidMicrobatchCount);
        }
        if groups.is_empty() {
            return Err(DistributedOptimizerError::MissingGroupContracts);
        }
        let mut seen_group_ids = BTreeMap::new();
        for group in &groups {
            if seen_group_ids.insert(group.group_id.clone(), ()).is_some() {
                return Err(DistributedOptimizerError::DuplicateGroupContract {
                    group_id: group.group_id.clone(),
                });
            }
        }
        validate_activation_checkpointing(&activation_checkpointing)?;
        let contract_digest = stable_contract_digest(
            optimizer_id.as_str(),
            optimizer_kind,
            &precision_policy,
            &accumulation_policy,
            &activation_checkpointing,
            memory_budget,
            &collective_sync_plan,
            groups.as_slice(),
        );
        Ok(Self {
            optimizer_id,
            optimizer_kind,
            precision_policy,
            accumulation_policy,
            activation_checkpointing,
            memory_budget,
            collective_sync_plan,
            groups,
            contract_digest,
        })
    }
}

/// Aggregate memory plan derived from the distributed optimizer contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedTrainingMemoryPlanReceipt {
    /// Stable contract digest the plan was derived from.
    pub contract_digest: String,
    /// Logical parameter bytes across all groups.
    pub parameter_logical_bytes: u64,
    /// Logical gradient bytes across all groups.
    pub gradient_logical_bytes: u64,
    /// Logical optimizer-state bytes across all groups.
    pub optimizer_state_logical_bytes: u64,
    /// Logical master-weight bytes across all groups.
    pub master_weight_logical_bytes: u64,
    /// Activation bytes after applying checkpointing.
    pub activation_peak_bytes: u64,
    /// Activation bytes saved by checkpointing.
    pub activation_bytes_saved: u64,
    /// Peak device bytes observed on one worker.
    pub peak_device_bytes_per_worker: u64,
    /// Peak host bytes observed on one worker.
    pub peak_host_bytes_per_worker: u64,
    /// Remote-offloaded optimizer bytes excluded from host/device peaks.
    pub remote_offloaded_optimizer_bytes: u64,
    /// Whether the plan stays within the declared device budget.
    pub within_device_budget: bool,
    /// Whether the plan stays within the declared host budget.
    pub within_host_budget: bool,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Receipt emitted when one microbatch is buffered for a later optimizer step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedMicrobatchReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Batch identifier attached to the microbatch.
    pub batch_id: String,
    /// One-based microbatch ordinal inside the current accumulation window.
    pub microbatch_index: u32,
    /// Number of buffered microbatches after this record.
    pub buffered_microbatches: u32,
    /// Whether the accumulation window is now flushable.
    pub flush_ready: bool,
    /// Aggregate sample count in this microbatch.
    pub sample_count: u32,
    /// Stable digest over the microbatch receipt.
    pub receipt_digest: String,
}

/// One inspectable group-level summary carried by a distributed optimizer step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedGroupStepSummary {
    /// Stable parameter-group identifier.
    pub group_id: String,
    /// Parameter sharding kind.
    pub parameter_shard_kind: TrainingParameterShardKind,
    /// Optimizer-state sharding kind.
    pub optimizer_state_shard_kind: TrainingOptimizerStateShardKind,
    /// Parameter-shard count.
    pub parameter_shard_count: usize,
    /// Optimizer-state shard count.
    pub optimizer_state_shard_count: usize,
    /// Gradient shard count.
    pub gradient_shard_count: usize,
}

/// Full optimizer-step receipt over one distributed accumulation window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DistributedOptimizerStepReceipt {
    /// Stable step identifier.
    pub step_id: String,
    /// Stable contract digest.
    pub contract_digest: String,
    /// Accumulated microbatch receipts included in the step.
    pub microbatches: Vec<DistributedMicrobatchReceipt>,
    /// Precision policy used by the step.
    pub precision_policy: TrainingPrecisionPolicy,
    /// Memory-plan receipt bound to the step.
    pub memory_plan: DistributedTrainingMemoryPlanReceipt,
    /// Collective sync plan bound to the step.
    pub collective_sync_plan: CollectiveSyncExecutionPlan,
    /// Group-level sharding summary carried into the step.
    pub groups: Vec<DistributedGroupStepSummary>,
    /// Underlying fixed-budget trainer-step receipt.
    pub trainer_step: TrainingStepReceipt,
    /// Stable digest over the full distributed step receipt.
    pub receipt_digest: String,
}

/// Stateful distributed optimizer wrapper over the fixed-budget training core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DistributedOptimizerRun {
    /// Explicit distributed optimizer contract.
    pub contract: DistributedOptimizerContract,
    /// Current memory plan under the contract.
    pub memory_plan: DistributedTrainingMemoryPlanReceipt,
    /// Buffered microbatch receipts waiting for flush.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_microbatch_receipts: Vec<DistributedMicrobatchReceipt>,
    /// Last distributed step receipt when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_step: Option<DistributedOptimizerStepReceipt>,
    base_run: FixedBudgetTrainingRun,
    pending_microbatches: Vec<TrainingGradientBatch>,
}

impl DistributedOptimizerRun {
    /// Creates a distributed optimizer run from raw parameter groups and the
    /// existing fixed-budget core.
    pub fn new(
        run_id: impl Into<String>,
        checkpoint_family: impl Into<String>,
        budget: crate::TrainingLoopBudget,
        parameter_groups: Vec<TrainingParameterGroupState>,
        contract: DistributedOptimizerContract,
    ) -> Result<Self, DistributedOptimizerError> {
        validate_contract_against_groups(parameter_groups.as_slice(), &contract)?;
        let memory_plan = build_memory_plan(parameter_groups.as_slice(), &contract);
        if !memory_plan.within_device_budget {
            return Err(DistributedOptimizerError::DeviceMemoryBudgetExceeded {
                peak_bytes: memory_plan.peak_device_bytes_per_worker,
                budget_bytes: contract.memory_budget.device_budget_bytes,
            });
        }
        if !memory_plan.within_host_budget {
            return Err(DistributedOptimizerError::HostMemoryBudgetExceeded {
                peak_bytes: memory_plan.peak_host_bytes_per_worker,
                budget_bytes: contract.memory_budget.host_budget_bytes,
            });
        }
        let base_run = FixedBudgetTrainingRun::new(
            run_id.into(),
            checkpoint_family.into(),
            budget,
            parameter_groups,
        )?;
        Ok(Self {
            contract,
            memory_plan,
            pending_microbatch_receipts: Vec::new(),
            last_step: None,
            base_run,
            pending_microbatches: Vec::new(),
        })
    }

    /// Buffers one microbatch until the accumulation window is ready to flush.
    pub fn record_microbatch(
        &mut self,
        batch: TrainingGradientBatch,
    ) -> Result<DistributedMicrobatchReceipt, DistributedOptimizerError> {
        if self.pending_microbatches.len()
            >= self.contract.accumulation_policy.microbatch_count as usize
        {
            return Err(DistributedOptimizerError::AccumulationWindowAlreadyFull {
                microbatch_count: self.contract.accumulation_policy.microbatch_count,
            });
        }
        validate_batch_against_contract(&batch, &self.contract)?;
        let microbatch_index = self.pending_microbatches.len() as u32 + 1;
        self.pending_microbatches.push(batch.clone());
        let receipt = DistributedMicrobatchReceipt {
            receipt_id: format!("{}-microbatch-{microbatch_index}", self.base_run.run_id()),
            batch_id: batch.batch_id.clone(),
            microbatch_index,
            buffered_microbatches: microbatch_index,
            flush_ready: microbatch_index == self.contract.accumulation_policy.microbatch_count,
            sample_count: batch.sample_count,
            receipt_digest: stable_microbatch_digest(
                self.base_run.run_id(),
                batch.batch_id.as_str(),
                microbatch_index,
                microbatch_index,
                batch.sample_count,
            ),
        };
        self.pending_microbatch_receipts.push(receipt.clone());
        Ok(receipt)
    }

    /// Flushes the current accumulation window into one distributed optimizer step.
    pub fn apply_accumulated_step(
        &mut self,
        started_at_ms: u64,
        finished_at_ms: u64,
    ) -> Result<DistributedOptimizerStepReceipt, DistributedOptimizerError> {
        let expected = self.contract.accumulation_policy.microbatch_count as usize;
        if self.pending_microbatches.len() != expected {
            return Err(DistributedOptimizerError::MicrobatchWindowIncomplete {
                expected_microbatches: expected as u32,
                actual_microbatches: self.pending_microbatches.len() as u32,
            });
        }
        let aggregated = aggregate_microbatches(
            self.pending_microbatches.as_slice(),
            self.contract.accumulation_policy.reduction,
        )?;
        let trainer_step = self.base_run.apply_step(TrainingStepInput::new(
            aggregated,
            started_at_ms,
            finished_at_ms,
        ))?;
        let groups = self
            .contract
            .groups
            .iter()
            .map(|group| DistributedGroupStepSummary {
                group_id: group.group_id.clone(),
                parameter_shard_kind: group.parameter_layout.kind,
                optimizer_state_shard_kind: group.optimizer_state_layout.kind,
                parameter_shard_count: group.parameter_layout.placements.len(),
                optimizer_state_shard_count: group.optimizer_state_layout.placements.len(),
                gradient_shard_count: group.gradient_layout.placements.len(),
            })
            .collect::<Vec<_>>();
        let step_id = format!(
            "{}-distributed-step-{}",
            self.base_run.run_id(),
            trainer_step.schedule.global_step
        );
        let receipt = DistributedOptimizerStepReceipt {
            receipt_digest: stable_distributed_step_digest(
                step_id.as_str(),
                self.contract.contract_digest.as_str(),
                self.pending_microbatch_receipts.as_slice(),
                &self.contract.precision_policy,
                &self.memory_plan,
                &self.contract.collective_sync_plan,
                groups.as_slice(),
                &trainer_step,
            ),
            step_id,
            contract_digest: self.contract.contract_digest.clone(),
            microbatches: self.pending_microbatch_receipts.clone(),
            precision_policy: self.contract.precision_policy.clone(),
            memory_plan: self.memory_plan.clone(),
            collective_sync_plan: self.contract.collective_sync_plan.clone(),
            groups,
            trainer_step,
        };
        self.pending_microbatches.clear();
        self.pending_microbatch_receipts.clear();
        self.last_step = Some(receipt.clone());
        Ok(receipt)
    }

    /// Returns the fixed-budget summary for the underlying run.
    #[must_use]
    pub fn summary(&self) -> crate::TrainingRunSummary {
        self.base_run.summary()
    }

    /// Returns the count of pending microbatches.
    #[must_use]
    pub fn pending_microbatch_count(&self) -> usize {
        self.pending_microbatches.len()
    }
}

/// Contract or runtime failure for distributed optimizer execution.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum DistributedOptimizerError {
    #[error("distributed optimizer contract is missing `optimizer_id`")]
    MissingOptimizerId,
    #[error("distributed optimizer contract must declare at least one group")]
    MissingGroupContracts,
    #[error("distributed optimizer contract repeated group `{group_id}`")]
    DuplicateGroupContract { group_id: String },
    #[error("distributed optimizer microbatch_count must be greater than zero")]
    InvalidMicrobatchCount,
    #[error(
        "activation checkpointing bytes are invalid: without={without_checkpointing}, with={with_checkpointing}"
    )]
    InvalidActivationCheckpointingBytes {
        without_checkpointing: u64,
        with_checkpointing: u64,
    },
    #[error("distributed optimizer contract is missing parameter group `{group_id}`")]
    UnknownParameterGroup { group_id: String },
    #[error(
        "parameter group `{group_id}` class mismatch: expected `{expected:?}`, found `{actual:?}`"
    )]
    ParameterClassMismatch {
        group_id: String,
        expected: TrainingParameterClass,
        actual: TrainingParameterClass,
    },
    #[error(
        "parameter group `{group_id}` dtype `{dtype:?}` does not match distributed precision `{precision:?}`"
    )]
    ParameterPrecisionMismatch {
        group_id: String,
        dtype: DType,
        precision: TrainingPrecisionMode,
    },
    #[error("parameter group `{group_id}` layout is missing shard placements")]
    MissingShardPlacements { group_id: String },
    #[error("parameter group `{group_id}` shard `{shard_id}` was duplicated")]
    DuplicateShardId { group_id: String, shard_id: usize },
    #[error(
        "parameter group `{group_id}` layout covers {covered_elements} elements but tensor has {tensor_elements}"
    )]
    ShardCoverageMismatch {
        group_id: String,
        covered_elements: usize,
        tensor_elements: usize,
    },
    #[error(
        "distributed optimizer accumulation window already holds {microbatch_count} microbatches"
    )]
    AccumulationWindowAlreadyFull { microbatch_count: u32 },
    #[error(
        "distributed optimizer step requires {expected_microbatches} microbatches but only {actual_microbatches} were buffered"
    )]
    MicrobatchWindowIncomplete {
        expected_microbatches: u32,
        actual_microbatches: u32,
    },
    #[error("distributed optimizer device memory peak {peak_bytes} exceeds budget {budget_bytes}")]
    DeviceMemoryBudgetExceeded { peak_bytes: u64, budget_bytes: u64 },
    #[error("distributed optimizer host memory peak {peak_bytes} exceeds budget {budget_bytes}")]
    HostMemoryBudgetExceeded { peak_bytes: u64, budget_bytes: u64 },
    #[error("distributed optimizer batch `{batch_id}` is missing gradient for group `{group_id}`")]
    MissingBatchGradient { batch_id: String, group_id: String },
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
}

fn validate_activation_checkpointing(
    policy: &TrainingActivationCheckpointPolicy,
) -> Result<(), DistributedOptimizerError> {
    match policy {
        TrainingActivationCheckpointPolicy::Disabled { .. } => Ok(()),
        TrainingActivationCheckpointPolicy::EveryNthBlock {
            activation_peak_bytes_without_checkpointing,
            activation_peak_bytes_with_checkpointing,
            ..
        }
        | TrainingActivationCheckpointPolicy::NamedRegions {
            activation_peak_bytes_without_checkpointing,
            activation_peak_bytes_with_checkpointing,
            ..
        } if activation_peak_bytes_with_checkpointing
            > activation_peak_bytes_without_checkpointing =>
        {
            Err(
                DistributedOptimizerError::InvalidActivationCheckpointingBytes {
                    without_checkpointing: *activation_peak_bytes_without_checkpointing,
                    with_checkpointing: *activation_peak_bytes_with_checkpointing,
                },
            )
        }
        _ => Ok(()),
    }
}

fn validate_contract_against_groups(
    parameter_groups: &[TrainingParameterGroupState],
    contract: &DistributedOptimizerContract,
) -> Result<(), DistributedOptimizerError> {
    let groups_by_id = parameter_groups
        .iter()
        .map(|group| (group.group_id.as_str(), group))
        .collect::<BTreeMap<_, _>>();
    for group_contract in &contract.groups {
        let group = groups_by_id
            .get(group_contract.group_id.as_str())
            .ok_or_else(|| DistributedOptimizerError::UnknownParameterGroup {
                group_id: group_contract.group_id.clone(),
            })?;
        if group.class != group_contract.class {
            return Err(DistributedOptimizerError::ParameterClassMismatch {
                group_id: group_contract.group_id.clone(),
                expected: group_contract.class,
                actual: group.class,
            });
        }
        if !contract
            .precision_policy
            .parameter_precision
            .matches_dtype(group.parameter.spec.dtype())
            && !contract
                .precision_policy
                .master_weight_precision
                .matches_dtype(group.parameter.spec.dtype())
        {
            return Err(DistributedOptimizerError::ParameterPrecisionMismatch {
                group_id: group_contract.group_id.clone(),
                dtype: group.parameter.spec.dtype(),
                precision: contract.precision_policy.parameter_precision,
            });
        }
        validate_layout(
            group_contract.group_id.as_str(),
            group.parameter.spec.storage_size(),
            &group_contract.parameter_layout,
        )?;
        validate_layout(
            group_contract.group_id.as_str(),
            group.parameter.spec.storage_size(),
            &group_contract.gradient_layout,
        )?;
        validate_optimizer_layout(
            group_contract.group_id.as_str(),
            group.parameter.spec.storage_size(),
            &group_contract.optimizer_state_layout,
        )?;
    }
    Ok(())
}

fn validate_layout(
    group_id: &str,
    tensor_elements: usize,
    layout: &TrainingParameterShardLayout,
) -> Result<(), DistributedOptimizerError> {
    if layout.placements.is_empty() {
        return Err(DistributedOptimizerError::MissingShardPlacements {
            group_id: String::from(group_id),
        });
    }
    let mut seen_shards = BTreeMap::new();
    let covered_elements = layout
        .placements
        .iter()
        .try_fold(0_usize, |acc, placement| {
            if seen_shards.insert(placement.shard_id, ()).is_some() {
                return Err(DistributedOptimizerError::DuplicateShardId {
                    group_id: String::from(group_id),
                    shard_id: placement.shard_id,
                });
            }
            Ok(acc.saturating_add(placement.range.element_count))
        })?;
    match layout.kind {
        TrainingParameterShardKind::Replicated => {
            if layout
                .placements
                .iter()
                .any(|placement| placement.range.element_count != tensor_elements)
            {
                return Err(DistributedOptimizerError::ShardCoverageMismatch {
                    group_id: String::from(group_id),
                    covered_elements,
                    tensor_elements,
                });
            }
        }
        _ if covered_elements != tensor_elements => {
            return Err(DistributedOptimizerError::ShardCoverageMismatch {
                group_id: String::from(group_id),
                covered_elements,
                tensor_elements,
            });
        }
        _ => {}
    }
    Ok(())
}

fn validate_optimizer_layout(
    group_id: &str,
    tensor_elements: usize,
    layout: &TrainingOptimizerStateShardLayout,
) -> Result<(), DistributedOptimizerError> {
    if layout.placements.is_empty() {
        return Err(DistributedOptimizerError::MissingShardPlacements {
            group_id: String::from(group_id),
        });
    }
    let mut seen_shards = BTreeMap::new();
    let covered_elements = layout
        .placements
        .iter()
        .try_fold(0_usize, |acc, placement| {
            if seen_shards.insert(placement.shard_id, ()).is_some() {
                return Err(DistributedOptimizerError::DuplicateShardId {
                    group_id: String::from(group_id),
                    shard_id: placement.shard_id,
                });
            }
            Ok(acc.saturating_add(placement.range.element_count))
        })?;
    match layout.kind {
        TrainingOptimizerStateShardKind::Replicated
        | TrainingOptimizerStateShardKind::ZeroStage1 => {
            if layout
                .placements
                .iter()
                .any(|placement| placement.range.element_count != tensor_elements)
            {
                return Err(DistributedOptimizerError::ShardCoverageMismatch {
                    group_id: String::from(group_id),
                    covered_elements,
                    tensor_elements,
                });
            }
        }
        _ if covered_elements != tensor_elements => {
            return Err(DistributedOptimizerError::ShardCoverageMismatch {
                group_id: String::from(group_id),
                covered_elements,
                tensor_elements,
            });
        }
        _ => {}
    }
    Ok(())
}

fn validate_batch_against_contract(
    batch: &TrainingGradientBatch,
    contract: &DistributedOptimizerContract,
) -> Result<(), DistributedOptimizerError> {
    for group in &contract.groups {
        if !batch.gradients.contains_key(group.group_id.as_str()) {
            return Err(DistributedOptimizerError::MissingBatchGradient {
                batch_id: batch.batch_id.clone(),
                group_id: group.group_id.clone(),
            });
        }
    }
    Ok(())
}

fn build_memory_plan(
    parameter_groups: &[TrainingParameterGroupState],
    contract: &DistributedOptimizerContract,
) -> DistributedTrainingMemoryPlanReceipt {
    let groups_by_id = parameter_groups
        .iter()
        .map(|group| (group.group_id.as_str(), group))
        .collect::<BTreeMap<_, _>>();
    let mut parameter_logical_bytes = 0_u64;
    let mut gradient_logical_bytes = 0_u64;
    let mut optimizer_state_logical_bytes = 0_u64;
    let mut master_weight_logical_bytes = 0_u64;
    let mut device_bytes_by_worker = BTreeMap::<String, u64>::new();
    let mut host_bytes_by_worker = BTreeMap::<String, u64>::new();
    let mut remote_offloaded_optimizer_bytes = 0_u64;

    for group_contract in &contract.groups {
        let group = groups_by_id
            .get(group_contract.group_id.as_str())
            .unwrap_or_else(|| unreachable!("contract validated before memory planning"));
        let elements = group.parameter.spec.storage_size() as u64;
        let parameter_bytes = elements.saturating_mul(
            contract
                .precision_policy
                .parameter_precision
                .element_size_bytes() as u64,
        );
        let gradient_bytes = elements.saturating_mul(
            contract
                .precision_policy
                .gradient_precision
                .element_size_bytes() as u64,
        );
        let optimizer_factor =
            optimizer_state_multiplier(group.optimizer.kind, group.optimizer.momentum);
        let optimizer_bytes = elements
            .saturating_mul(
                contract
                    .precision_policy
                    .optimizer_state_precision
                    .element_size_bytes() as u64,
            )
            .saturating_mul(optimizer_factor);
        let master_weight_bytes = if contract.precision_policy.master_weight_precision
            == contract.precision_policy.parameter_precision
        {
            0
        } else {
            elements.saturating_mul(
                contract
                    .precision_policy
                    .master_weight_precision
                    .element_size_bytes() as u64,
            )
        };
        parameter_logical_bytes = parameter_logical_bytes.saturating_add(parameter_bytes);
        gradient_logical_bytes = gradient_logical_bytes.saturating_add(gradient_bytes);
        optimizer_state_logical_bytes =
            optimizer_state_logical_bytes.saturating_add(optimizer_bytes);
        master_weight_logical_bytes =
            master_weight_logical_bytes.saturating_add(master_weight_bytes);

        distribute_layout_bytes(
            &mut device_bytes_by_worker,
            group_contract.parameter_layout.placements.as_slice(),
            contract
                .precision_policy
                .parameter_precision
                .element_size_bytes() as u64,
        );
        distribute_layout_bytes(
            &mut device_bytes_by_worker,
            group_contract.gradient_layout.placements.as_slice(),
            contract
                .precision_policy
                .gradient_precision
                .element_size_bytes() as u64,
        );
        match group_contract.optimizer_state_layout.residency {
            TrainingOptimizerShardResidency::DeviceResident => distribute_layout_bytes(
                &mut device_bytes_by_worker,
                group_contract.optimizer_state_layout.placements.as_slice(),
                contract
                    .precision_policy
                    .optimizer_state_precision
                    .element_size_bytes() as u64
                    * optimizer_factor,
            ),
            TrainingOptimizerShardResidency::HostOffloaded => distribute_layout_bytes(
                &mut host_bytes_by_worker,
                group_contract.optimizer_state_layout.placements.as_slice(),
                contract
                    .precision_policy
                    .optimizer_state_precision
                    .element_size_bytes() as u64
                    * optimizer_factor,
            ),
            TrainingOptimizerShardResidency::RemoteOffloaded => {
                remote_offloaded_optimizer_bytes = remote_offloaded_optimizer_bytes.saturating_add(
                    group_contract
                        .optimizer_state_layout
                        .placements
                        .iter()
                        .map(|placement| placement.range.element_count as u64)
                        .sum::<u64>()
                        .saturating_mul(
                            contract
                                .precision_policy
                                .optimizer_state_precision
                                .element_size_bytes() as u64,
                        )
                        .saturating_mul(optimizer_factor),
                );
            }
        }
        distribute_master_weight_bytes(
            &mut device_bytes_by_worker,
            &mut host_bytes_by_worker,
            group_contract,
            master_weight_bytes,
        );
    }

    let peak_device_bytes = device_bytes_by_worker
        .values()
        .copied()
        .max()
        .unwrap_or_default()
        .saturating_add(contract.activation_checkpointing.activation_peak_bytes())
        .saturating_add(contract.memory_budget.scratch_reserve_bytes);
    let peak_host_bytes = host_bytes_by_worker
        .values()
        .copied()
        .max()
        .unwrap_or_default();
    let receipt_digest = stable_memory_plan_digest(
        contract.contract_digest.as_str(),
        parameter_logical_bytes,
        gradient_logical_bytes,
        optimizer_state_logical_bytes,
        master_weight_logical_bytes,
        contract.activation_checkpointing.activation_peak_bytes(),
        contract.activation_checkpointing.activation_bytes_saved(),
        peak_device_bytes,
        peak_host_bytes,
        remote_offloaded_optimizer_bytes,
        peak_device_bytes <= contract.memory_budget.device_budget_bytes,
        peak_host_bytes <= contract.memory_budget.host_budget_bytes,
    );
    DistributedTrainingMemoryPlanReceipt {
        contract_digest: contract.contract_digest.clone(),
        parameter_logical_bytes,
        gradient_logical_bytes,
        optimizer_state_logical_bytes,
        master_weight_logical_bytes,
        activation_peak_bytes: contract.activation_checkpointing.activation_peak_bytes(),
        activation_bytes_saved: contract.activation_checkpointing.activation_bytes_saved(),
        peak_device_bytes_per_worker: peak_device_bytes,
        peak_host_bytes_per_worker: peak_host_bytes,
        remote_offloaded_optimizer_bytes,
        within_device_budget: peak_device_bytes <= contract.memory_budget.device_budget_bytes,
        within_host_budget: peak_host_bytes <= contract.memory_budget.host_budget_bytes,
        receipt_digest,
    }
}

fn distribute_layout_bytes(
    bytes_by_worker: &mut BTreeMap<String, u64>,
    placements: &[TrainingShardPlacement],
    element_size_bytes: u64,
) {
    for placement in placements {
        let bytes = (placement.range.element_count as u64).saturating_mul(element_size_bytes);
        let entry = bytes_by_worker
            .entry(placement.node_id.clone())
            .or_default();
        *entry = entry.saturating_add(bytes);
    }
}

fn distribute_master_weight_bytes(
    device_bytes_by_worker: &mut BTreeMap<String, u64>,
    host_bytes_by_worker: &mut BTreeMap<String, u64>,
    group_contract: &DistributedOptimizerGroupContract,
    master_weight_bytes: u64,
) {
    if master_weight_bytes == 0 {
        return;
    }
    let target = match group_contract.master_weight_residency {
        OptimizerStateResidency::DeviceResident => device_bytes_by_worker,
        OptimizerStateResidency::HostResident | OptimizerStateResidency::Offloaded => {
            host_bytes_by_worker
        }
    };
    let shard_count = group_contract.parameter_layout.placements.len().max(1) as u64;
    let bytes_per_placement = master_weight_bytes / shard_count;
    for placement in &group_contract.parameter_layout.placements {
        let entry = target.entry(placement.node_id.clone()).or_default();
        *entry = entry.saturating_add(bytes_per_placement);
    }
}

fn optimizer_state_multiplier(kind: TrainingOptimizerConfigKind, momentum: Option<f32>) -> u64 {
    match kind {
        TrainingOptimizerConfigKind::Sgd | TrainingOptimizerConfigKind::Lars => {
            if momentum.is_some() { 1 } else { 0 }
        }
        TrainingOptimizerConfigKind::Adam
        | TrainingOptimizerConfigKind::AdamW
        | TrainingOptimizerConfigKind::Lamb => 2,
    }
}

type TrainingOptimizerConfigKind = crate::TrainingOptimizerKind;

fn aggregate_microbatches(
    batches: &[TrainingGradientBatch],
    reduction: TrainingGradientAccumulationReduction,
) -> Result<TrainingGradientBatch, DistributedOptimizerError> {
    let sample_count = batches
        .iter()
        .fold(0_u32, |acc, batch| acc.saturating_add(batch.sample_count));
    let loss = if batches.is_empty() {
        0.0
    } else {
        let loss_sum = batches.iter().fold(0.0_f32, |acc, batch| acc + batch.loss);
        match reduction {
            TrainingGradientAccumulationReduction::Sum => loss_sum,
            TrainingGradientAccumulationReduction::Mean => loss_sum / batches.len() as f32,
        }
    };
    let mut gradients = BTreeMap::new();
    let divisor = match reduction {
        TrainingGradientAccumulationReduction::Sum => 1.0_f32,
        TrainingGradientAccumulationReduction::Mean => batches.len() as f32,
    };
    let first = batches
        .first()
        .unwrap_or_else(|| unreachable!("microbatch window is validated before aggregation"));
    for (group_id, first_gradient) in &first.gradients {
        let mut values = tensor_values(first_gradient, group_id.as_str())?;
        for batch in batches.iter().skip(1) {
            let gradient = batch.gradients.get(group_id.as_str()).ok_or_else(|| {
                DistributedOptimizerError::MissingBatchGradient {
                    batch_id: batch.batch_id.clone(),
                    group_id: group_id.clone(),
                }
            })?;
            for (slot, value) in values
                .iter_mut()
                .zip(tensor_values(gradient, group_id.as_str())?)
            {
                *slot += value;
            }
        }
        if divisor > 1.0 {
            for value in &mut values {
                *value /= divisor;
            }
        }
        gradients.insert(
            group_id.clone(),
            crate::TrainingTensorBuffer::from_f32(
                group_id.clone(),
                first_gradient.spec.clone(),
                values,
            )?,
        );
    }
    Ok(TrainingGradientBatch::new(
        format!(
            "accumulated:{}",
            batches
                .iter()
                .map(|batch| batch.batch_id.as_str())
                .collect::<Vec<_>>()
                .join("+")
        ),
        loss,
        sample_count,
        gradients,
    ))
}

fn tensor_values(
    tensor: &crate::TrainingTensorBuffer,
    group_id: &str,
) -> Result<Vec<f32>, DistributedOptimizerError> {
    match &tensor.data {
        TensorData::F32(values) => Ok(values.clone()),
        TensorData::QuantizedBlocks(_) => Err(DistributedOptimizerError::TrainingCore(
            TrainingCoreError::UnsupportedTensorDType {
                group_id: String::from(group_id),
                dtype: tensor.spec.dtype(),
            },
        )),
    }
}

fn stable_contract_digest(
    optimizer_id: &str,
    optimizer_kind: TrainingDistributedOptimizerKind,
    precision_policy: &TrainingPrecisionPolicy,
    accumulation_policy: &TrainingGradientAccumulationPolicy,
    activation_checkpointing: &TrainingActivationCheckpointPolicy,
    memory_budget: DistributedTrainingMemoryBudget,
    collective_sync_plan: &CollectiveSyncExecutionPlan,
    groups: &[DistributedOptimizerGroupContract],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_distributed_optimizer_contract|");
    hasher.update(optimizer_id.as_bytes());
    hasher.update(b"|");
    hasher.update(stable_json_bytes(&optimizer_kind));
    hasher.update(stable_json_bytes(precision_policy));
    hasher.update(stable_json_bytes(accumulation_policy));
    hasher.update(stable_json_bytes(activation_checkpointing));
    hasher.update(stable_json_bytes(&memory_budget));
    hasher.update(stable_json_bytes(collective_sync_plan));
    for group in groups {
        hasher.update(b"|group|");
        hasher.update(stable_json_bytes(group));
    }
    hex::encode(hasher.finalize())
}

fn stable_memory_plan_digest(
    contract_digest: &str,
    parameter_logical_bytes: u64,
    gradient_logical_bytes: u64,
    optimizer_state_logical_bytes: u64,
    master_weight_logical_bytes: u64,
    activation_peak_bytes: u64,
    activation_bytes_saved: u64,
    peak_device_bytes: u64,
    peak_host_bytes: u64,
    remote_offloaded_optimizer_bytes: u64,
    within_device_budget: bool,
    within_host_budget: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_distributed_training_memory_plan|");
    hasher.update(contract_digest.as_bytes());
    for value in [
        parameter_logical_bytes,
        gradient_logical_bytes,
        optimizer_state_logical_bytes,
        master_weight_logical_bytes,
        activation_peak_bytes,
        activation_bytes_saved,
        peak_device_bytes,
        peak_host_bytes,
        remote_offloaded_optimizer_bytes,
    ] {
        hasher.update(b"|");
        hasher.update(value.to_string().as_bytes());
    }
    hasher.update(b"|");
    hasher.update(if within_device_budget { b"1" } else { b"0" });
    hasher.update(b"|");
    hasher.update(if within_host_budget { b"1" } else { b"0" });
    hex::encode(hasher.finalize())
}

fn stable_microbatch_digest(
    run_id: &str,
    batch_id: &str,
    microbatch_index: u32,
    buffered_microbatches: u32,
    sample_count: u32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_distributed_microbatch_receipt|");
    hasher.update(run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(batch_id.as_bytes());
    hasher.update(b"|");
    hasher.update(microbatch_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(buffered_microbatches.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(sample_count.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_distributed_step_digest(
    step_id: &str,
    contract_digest: &str,
    microbatches: &[DistributedMicrobatchReceipt],
    precision_policy: &TrainingPrecisionPolicy,
    memory_plan: &DistributedTrainingMemoryPlanReceipt,
    collective_sync_plan: &CollectiveSyncExecutionPlan,
    groups: &[DistributedGroupStepSummary],
    trainer_step: &TrainingStepReceipt,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_distributed_optimizer_step|");
    hasher.update(step_id.as_bytes());
    hasher.update(b"|");
    hasher.update(contract_digest.as_bytes());
    for microbatch in microbatches {
        hasher.update(b"|microbatch|");
        hasher.update(microbatch.receipt_digest.as_bytes());
    }
    hasher.update(stable_json_bytes(precision_policy));
    hasher.update(stable_json_bytes(memory_plan));
    hasher.update(stable_json_bytes(collective_sync_plan));
    for group in groups {
        hasher.update(b"|group|");
        hasher.update(stable_json_bytes(group));
    }
    hasher.update(b"|trainer_step|");
    hasher.update(trainer_step.receipt_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    to_vec(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use psionic_collectives::{
        CollectiveMeshMember, CollectiveSyncCadencePolicy, CollectiveTransportFeedback,
        ElasticCollectivePlanner, QuantizedCollectiveBenchmark, QuantizedCollectiveBenchmarkSample,
    };

    use super::{
        DistributedOptimizerContract, DistributedOptimizerError, DistributedOptimizerGroupContract,
        DistributedOptimizerRun, DistributedTrainingMemoryBudget,
        TrainingActivationCheckpointPolicy, TrainingDistributedOptimizerKind,
        TrainingGradientAccumulationPolicy, TrainingGradientAccumulationReduction,
        TrainingOptimizerShardResidency, TrainingOptimizerStateShardKind,
        TrainingOptimizerStateShardLayout, TrainingParameterShardKind,
        TrainingParameterShardLayout, TrainingPrecisionPolicy, TrainingShardPlacement,
        TrainingShardRange,
    };
    use crate::{
        OptimizerStateResidency, TrainingGradientBatch, TrainingLoopBudget,
        TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy, TrainingParameterClass,
        TrainingParameterGroupState, TrainingStepSchedule, TrainingTensorBuffer,
    };
    use psionic_core::{DType, Device, Shape, TensorSpec};
    use psionic_runtime::{
        ClusterCommunicationClass, TrainingCollectiveKind, TrainingCollectiveQuantization,
        TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind, TrainingElasticMembershipContext,
    };

    fn planner() -> Result<ElasticCollectivePlanner, Box<dyn std::error::Error>> {
        let mut planner = ElasticCollectivePlanner::new(
            "mesh-train",
            "cuda",
            ClusterCommunicationClass::TensorCollectiveMesh,
            vec![
                TrainingDeviceMeshAxis::new("dp", TrainingDeviceMeshAxisKind::DataParallel, 2)
                    .with_collective_group_size(2),
                TrainingDeviceMeshAxis::new("tp", TrainingDeviceMeshAxisKind::TensorParallel, 2)
                    .with_collective_group_size(2),
            ],
        );
        planner.observe_mesh(
            TrainingElasticMembershipContext::new(
                1,
                "cluster-state-1",
                "topology-1",
                vec![
                    String::from("worker-a"),
                    String::from("worker-b"),
                    String::from("worker-c"),
                    String::from("worker-d"),
                ],
            ),
            vec![
                CollectiveMeshMember::new("worker-a", 0, 0, "cuda:0"),
                CollectiveMeshMember::new("worker-b", 1, 1, "cuda:1"),
                CollectiveMeshMember::new("worker-c", 2, 2, "cuda:2"),
                CollectiveMeshMember::new("worker-d", 3, 3, "cuda:3"),
            ],
        )?;
        planner.record_benchmark(QuantizedCollectiveBenchmark::new(
            TrainingCollectiveKind::AllReduce,
            TrainingCollectiveQuantization::Int8Symmetric,
            QuantizedCollectiveBenchmarkSample::new(2_400, 32 * 1024 * 1024, 0),
            QuantizedCollectiveBenchmarkSample::new(1_200, 8 * 1024 * 1024, 55),
            100,
            1_000,
        ));
        planner.observe_transport_feedback(
            CollectiveTransportFeedback::new(6_000, 150, 2, 1).with_detail("healthy nvlink mesh"),
        );
        Ok(planner)
    }

    fn contract() -> Result<DistributedOptimizerContract, Box<dyn std::error::Error>> {
        let mut planner = planner()?;
        let collective_sync_plan = planner.plan_sync(
            4,
            TrainingCollectiveKind::AllReduce,
            8 * 1024 * 1024,
            TrainingCollectiveQuantization::Int8Symmetric,
            &CollectiveSyncCadencePolicy::new(),
        )?;
        Ok(DistributedOptimizerContract::new(
            "optimizer://weather-train",
            TrainingDistributedOptimizerKind::ZeroStage3,
            TrainingPrecisionPolicy::bf16_master_fp32(
                TrainingCollectiveQuantization::Int8Symmetric,
            ),
            TrainingGradientAccumulationPolicy::new(
                2,
                TrainingGradientAccumulationReduction::Mean,
                TrainingCollectiveKind::AllReduce,
            ),
            TrainingActivationCheckpointPolicy::EveryNthBlock {
                block_interval: 2,
                activation_peak_bytes_without_checkpointing: 2_000_000_000,
                activation_peak_bytes_with_checkpointing: 900_000_000,
                rematerialization_overhead_bps: 1_200,
            },
            DistributedTrainingMemoryBudget::new(2_200_000_000, 1_400_000_000, 128_000_000),
            collective_sync_plan,
            vec![
                DistributedOptimizerGroupContract::new(
                    "decoder.weight",
                    TrainingParameterClass::Matrix,
                    TrainingParameterShardLayout::new(
                        TrainingParameterShardKind::FullShard,
                        vec![
                            TrainingShardPlacement::new(
                                0,
                                "tp",
                                "worker-a",
                                "cuda:0",
                                0,
                                TrainingShardRange::new(0, 2),
                            ),
                            TrainingShardPlacement::new(
                                1,
                                "tp",
                                "worker-b",
                                "cuda:1",
                                0,
                                TrainingShardRange::new(2, 2),
                            ),
                        ],
                    )
                    .with_axis_id("tp"),
                    TrainingParameterShardLayout::new(
                        TrainingParameterShardKind::FullShard,
                        vec![
                            TrainingShardPlacement::new(
                                0,
                                "tp",
                                "worker-a",
                                "cuda:0",
                                0,
                                TrainingShardRange::new(0, 2),
                            ),
                            TrainingShardPlacement::new(
                                1,
                                "tp",
                                "worker-b",
                                "cuda:1",
                                0,
                                TrainingShardRange::new(2, 2),
                            ),
                        ],
                    )
                    .with_axis_id("tp"),
                    TrainingOptimizerStateShardLayout::new(
                        TrainingOptimizerStateShardKind::ZeroStage3,
                        TrainingOptimizerShardResidency::HostOffloaded,
                        vec![
                            TrainingShardPlacement::new(
                                0,
                                "dp",
                                "worker-a",
                                "cuda:0",
                                0,
                                TrainingShardRange::new(0, 2),
                            ),
                            TrainingShardPlacement::new(
                                1,
                                "dp",
                                "worker-c",
                                "cuda:2",
                                0,
                                TrainingShardRange::new(2, 2),
                            ),
                        ],
                    )
                    .with_axis_id("dp"),
                    OptimizerStateResidency::HostResident,
                ),
                DistributedOptimizerGroupContract::new(
                    "decoder.bias",
                    TrainingParameterClass::Bias,
                    TrainingParameterShardLayout::new(
                        TrainingParameterShardKind::Replicated,
                        vec![
                            TrainingShardPlacement::new(
                                0,
                                "dp",
                                "worker-a",
                                "cuda:0",
                                0,
                                TrainingShardRange::new(0, 2),
                            ),
                            TrainingShardPlacement::new(
                                1,
                                "dp",
                                "worker-b",
                                "cuda:1",
                                1,
                                TrainingShardRange::new(0, 2),
                            ),
                        ],
                    )
                    .with_axis_id("dp"),
                    TrainingParameterShardLayout::new(
                        TrainingParameterShardKind::Replicated,
                        vec![
                            TrainingShardPlacement::new(
                                0,
                                "dp",
                                "worker-a",
                                "cuda:0",
                                0,
                                TrainingShardRange::new(0, 2),
                            ),
                            TrainingShardPlacement::new(
                                1,
                                "dp",
                                "worker-b",
                                "cuda:1",
                                1,
                                TrainingShardRange::new(0, 2),
                            ),
                        ],
                    )
                    .with_axis_id("dp"),
                    TrainingOptimizerStateShardLayout::new(
                        TrainingOptimizerStateShardKind::Replicated,
                        TrainingOptimizerShardResidency::DeviceResident,
                        vec![
                            TrainingShardPlacement::new(
                                0,
                                "dp",
                                "worker-a",
                                "cuda:0",
                                0,
                                TrainingShardRange::new(0, 2),
                            ),
                            TrainingShardPlacement::new(
                                1,
                                "dp",
                                "worker-b",
                                "cuda:1",
                                1,
                                TrainingShardRange::new(0, 2),
                            ),
                        ],
                    )
                    .with_axis_id("dp"),
                    OptimizerStateResidency::DeviceResident,
                ),
            ],
        )?)
    }

    fn parameter_groups() -> Result<Vec<TrainingParameterGroupState>, Box<dyn std::error::Error>> {
        Ok(vec![
            TrainingParameterGroupState::new(
                "decoder.weight",
                TrainingParameterClass::Matrix,
                TrainingTensorBuffer::from_f32(
                    "decoder.weight",
                    TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu()),
                    vec![0.1, 0.2, 0.3, 0.4],
                )?,
                TrainingOptimizerConfig::adamw(0.001, 0.9, 0.999, 1e-8),
                TrainingOptimizerResidencyPolicy::device_step_offload_idle(),
            )?,
            TrainingParameterGroupState::new(
                "decoder.bias",
                TrainingParameterClass::Bias,
                TrainingTensorBuffer::from_f32(
                    "decoder.bias",
                    TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu()),
                    vec![0.0, 0.1],
                )?,
                TrainingOptimizerConfig::sgd(0.01).with_momentum(0.9),
                TrainingOptimizerResidencyPolicy::host_only(),
            )?,
        ])
    }

    fn microbatch(
        batch_id: &str,
        weight_gradient: [f32; 4],
        bias_gradient: [f32; 2],
    ) -> Result<TrainingGradientBatch, Box<dyn std::error::Error>> {
        Ok(TrainingGradientBatch::new(
            batch_id,
            0.25,
            4,
            BTreeMap::from([
                (
                    String::from("decoder.weight"),
                    TrainingTensorBuffer::from_f32(
                        "decoder.weight",
                        TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu()),
                        weight_gradient.to_vec(),
                    )?,
                ),
                (
                    String::from("decoder.bias"),
                    TrainingTensorBuffer::from_f32(
                        "decoder.bias",
                        TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu()),
                        bias_gradient.to_vec(),
                    )?,
                ),
            ]),
        ))
    }

    #[test]
    fn distributed_optimizer_contract_surfaces_precision_and_memory_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let contract = contract()?;
        let memory_plan = super::build_memory_plan(parameter_groups()?.as_slice(), &contract);

        assert_eq!(contract.groups.len(), 2);
        assert_eq!(
            contract.precision_policy.parameter_precision,
            super::TrainingPrecisionMode::Bf16
        );
        assert_eq!(
            contract.precision_policy.master_weight_precision,
            super::TrainingPrecisionMode::Fp32
        );
        assert!(memory_plan.activation_bytes_saved > 0);
        assert!(memory_plan.within_device_budget);
        assert!(memory_plan.within_host_budget);
        assert!(memory_plan.remote_offloaded_optimizer_bytes == 0);
        assert!(!contract.contract_digest.is_empty());
        Ok(())
    }

    #[test]
    fn distributed_optimizer_run_accumulates_microbatches_and_flushes_step()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut run = DistributedOptimizerRun::new(
            "distributed-run",
            "train.weather.agent",
            TrainingLoopBudget::new(2, 1, 1)?,
            parameter_groups()?,
            contract()?,
        )?;

        let first = run.record_microbatch(microbatch(
            "microbatch-a",
            [0.01, 0.02, 0.03, 0.04],
            [0.005, 0.01],
        )?)?;
        assert!(!first.flush_ready);
        let second = run.record_microbatch(microbatch(
            "microbatch-b",
            [0.02, 0.03, 0.04, 0.05],
            [0.007, 0.011],
        )?)?;
        assert!(second.flush_ready);

        let receipt = run.apply_accumulated_step(1_000, 1_030)?;
        assert_eq!(receipt.microbatches.len(), 2);
        assert_eq!(receipt.groups.len(), 2);
        assert_eq!(
            receipt.trainer_step.schedule,
            TrainingStepSchedule {
                global_step: 1,
                window_index: 1,
                step_in_window: 1,
                cadence_index: 1,
                window_in_cadence: 1,
            }
        );
        assert_eq!(run.pending_microbatch_count(), 0);
        assert_eq!(run.summary().completed_steps, 1);
        assert!(!receipt.collective_sync_plan.stages.is_empty());
        Ok(())
    }

    #[test]
    fn distributed_optimizer_contract_refuses_incomplete_shard_coverage() {
        let invalid = DistributedOptimizerContract::new(
            "optimizer://invalid",
            TrainingDistributedOptimizerKind::ZeroStage3,
            TrainingPrecisionPolicy::bf16_master_fp32(TrainingCollectiveQuantization::None),
            TrainingGradientAccumulationPolicy::new(
                1,
                TrainingGradientAccumulationReduction::Sum,
                TrainingCollectiveKind::AllReduce,
            ),
            TrainingActivationCheckpointPolicy::Disabled {
                activation_peak_bytes: 512,
            },
            DistributedTrainingMemoryBudget::new(4_096, 4_096, 0),
            planner()
                .expect("planner should build")
                .plan_sync(
                    1,
                    TrainingCollectiveKind::AllReduce,
                    1_024,
                    TrainingCollectiveQuantization::Int8Symmetric,
                    &CollectiveSyncCadencePolicy::new(),
                )
                .expect("plan should build"),
            vec![DistributedOptimizerGroupContract::new(
                "decoder.weight",
                TrainingParameterClass::Matrix,
                TrainingParameterShardLayout::new(
                    TrainingParameterShardKind::FullShard,
                    vec![TrainingShardPlacement::new(
                        0,
                        "tp",
                        "worker-a",
                        "cuda:0",
                        0,
                        TrainingShardRange::new(0, 1),
                    )],
                )
                .with_axis_id("tp"),
                TrainingParameterShardLayout::new(
                    TrainingParameterShardKind::FullShard,
                    vec![TrainingShardPlacement::new(
                        0,
                        "tp",
                        "worker-a",
                        "cuda:0",
                        0,
                        TrainingShardRange::new(0, 1),
                    )],
                )
                .with_axis_id("tp"),
                TrainingOptimizerStateShardLayout::new(
                    TrainingOptimizerStateShardKind::ZeroStage3,
                    TrainingOptimizerShardResidency::DeviceResident,
                    vec![TrainingShardPlacement::new(
                        0,
                        "tp",
                        "worker-a",
                        "cuda:0",
                        0,
                        TrainingShardRange::new(0, 1),
                    )],
                ),
                OptimizerStateResidency::DeviceResident,
            )],
        )
        .and_then(|contract| {
            DistributedOptimizerRun::new(
                "distributed-run",
                "train.weather.agent",
                TrainingLoopBudget::new(1, 1, 1).expect("budget should build"),
                parameter_groups().expect("groups should build"),
                contract,
            )
            .map(|_| ())
        });

        assert!(matches!(
            invalid,
            Err(DistributedOptimizerError::ShardCoverageMismatch { .. })
        ));
    }
}
