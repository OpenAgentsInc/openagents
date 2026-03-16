use std::collections::{BTreeMap, BTreeSet};

use psionic_core::{PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope, TensorId};
use psionic_ir::ExecutionPlan;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    BackendSelection, DeliveredExecutionContext, DeviceDiscovery, DeviceInventoryQualifiers,
    ExecutionBackend, ExecutionMetrics, ExecutionPartition, ExecutionResult,
    ExecutionShardAssignment, ExecutionTopologyKind, ExecutionTopologyPlan, RuntimeError,
    ShardedModelArtifactRef, ShardedModelLayoutKind, ShardedModelManifest,
    ShardedModelManifestError,
};

/// Inspectable weight classes for one representative local sharding contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalModelWeightClass {
    /// Token embedding table.
    TokenEmbedding,
    /// Final LM head or comparable output projection.
    OutputProjection,
    /// Attention query projection.
    AttentionQuery,
    /// Attention key projection.
    AttentionKey,
    /// Attention value projection.
    AttentionValue,
    /// Attention output projection.
    AttentionOutput,
    /// Feed-forward gate projection.
    FeedForwardGate,
    /// Feed-forward up projection.
    FeedForwardUp,
    /// Feed-forward down projection.
    FeedForwardDown,
    /// Attention input or post-attention norm weights.
    AttentionNorm,
    /// Feed-forward norm weights.
    FeedForwardNorm,
    /// KV-cache state bound to the local shard set.
    KvCache,
}

/// Declarative sharding strategy for one weight class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LocalWeightShardingStrategy {
    /// Keep the weight or state replicated on every participating device.
    Replicated,
    /// Bind the class to the layer range owned by each shard.
    LayerRangeOwned,
    /// Partition the class over one tensor axis.
    TensorAxis {
        /// Tensor axis partitioned across the local shard set.
        axis: usize,
    },
}

impl LocalWeightShardingStrategy {
    fn matches_layout(self, layout: ShardedModelLayoutKind) -> bool {
        match self {
            Self::Replicated => true,
            Self::LayerRangeOwned => layout == ShardedModelLayoutKind::LayerSharded,
            Self::TensorAxis { .. } => layout == ShardedModelLayoutKind::TensorSharded,
        }
    }

    fn is_primary_for_layout(self, layout: ShardedModelLayoutKind) -> bool {
        match self {
            Self::Replicated => layout == ShardedModelLayoutKind::Replicated,
            Self::LayerRangeOwned => layout == ShardedModelLayoutKind::LayerSharded,
            Self::TensorAxis { .. } => layout == ShardedModelLayoutKind::TensorSharded,
        }
    }
}

/// One inspectable sharding rule for a representative model-family weight class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalWeightShardingRule {
    /// Weight class this rule applies to.
    pub class: LocalModelWeightClass,
    /// Strategy used for the class.
    pub strategy: LocalWeightShardingStrategy,
    /// Plain-language explanation when the rule needs one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl LocalWeightShardingRule {
    /// Creates one weight-class sharding rule.
    #[must_use]
    pub fn new(class: LocalModelWeightClass, strategy: LocalWeightShardingStrategy) -> Self {
        Self {
            class,
            strategy,
            detail: None,
        }
    }

    /// Attaches plain-language detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Declared shard execution mode for one local sharding policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalShardingExecutionMode {
    /// The backend only needs whole-model or replica execution for each shard.
    WholeModelOrReplica,
    /// The backend must execute explicit layer or tensor partitions.
    Partitioned,
}

/// Declared cross-device collective posture for one local sharding policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalCollectivePosture {
    /// No cross-device collective is part of the current runtime contract.
    None,
    /// Later collectives or activation handoff remain caller-managed future work.
    CallerManagedFuture,
}

/// Declared result or evidence posture for one local sharding policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalExecutionOutcomePosture {
    /// The current local runner only guarantees per-shard metrics and evidence.
    MetricsOnlyEvidence,
}

/// Validation failure for one local sharding policy declaration.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LocalShardingPolicyError {
    /// The policy identifier was blank.
    #[error("local sharding policy is missing a policy_id")]
    MissingPolicyId,
    /// Policy version zero is not allowed.
    #[error("local sharding policy version must be greater than zero")]
    InvalidPolicyVersion,
    /// The declared execution mode does not match the layout family.
    #[error(
        "local sharding policy layout {layout:?} does not allow execution mode {execution_mode:?}"
    )]
    LayoutExecutionModeMismatch {
        /// Declared layout.
        layout: ShardedModelLayoutKind,
        /// Declared execution mode.
        execution_mode: LocalShardingExecutionMode,
    },
    /// The declared collective posture does not match the layout family.
    #[error(
        "local sharding policy layout {layout:?} does not allow collective posture {collective_posture:?}"
    )]
    LayoutCollectivePostureMismatch {
        /// Declared layout.
        layout: ShardedModelLayoutKind,
        /// Declared collective posture.
        collective_posture: LocalCollectivePosture,
    },
    /// The declared outcome posture does not match the current local runner scope.
    #[error(
        "local sharding policy layout {layout:?} does not allow outcome posture {outcome_posture:?}"
    )]
    LayoutOutcomePostureMismatch {
        /// Declared layout.
        layout: ShardedModelLayoutKind,
        /// Declared outcome posture.
        outcome_posture: LocalExecutionOutcomePosture,
    },
}

/// Explicit local sharding policy bound to one layout family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalShardingPolicy {
    /// Stable policy identifier.
    pub policy_id: String,
    /// Version of the policy contract.
    pub policy_version: u32,
    /// Layout family this policy applies to.
    pub layout: ShardedModelLayoutKind,
    /// Execution mode required by the backend for the shard set.
    pub execution_mode: LocalShardingExecutionMode,
    /// Cross-device collective posture for the current policy.
    pub collective_posture: LocalCollectivePosture,
    /// Output or evidence posture for the current policy.
    pub outcome_posture: LocalExecutionOutcomePosture,
    /// Plain-language policy detail when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Stable digest over the policy.
    pub policy_digest: String,
}

impl LocalShardingPolicy {
    /// Creates a validated local sharding policy and derives its stable digest.
    pub fn new(
        policy_id: impl Into<String>,
        policy_version: u32,
        layout: ShardedModelLayoutKind,
        execution_mode: LocalShardingExecutionMode,
        collective_posture: LocalCollectivePosture,
        outcome_posture: LocalExecutionOutcomePosture,
    ) -> Result<Self, LocalShardingPolicyError> {
        let policy_id = policy_id.into();
        if policy_id.trim().is_empty() {
            return Err(LocalShardingPolicyError::MissingPolicyId);
        }
        if policy_version == 0 {
            return Err(LocalShardingPolicyError::InvalidPolicyVersion);
        }
        match layout {
            ShardedModelLayoutKind::Replicated => {
                if execution_mode != LocalShardingExecutionMode::WholeModelOrReplica {
                    return Err(LocalShardingPolicyError::LayoutExecutionModeMismatch {
                        layout,
                        execution_mode,
                    });
                }
                if collective_posture != LocalCollectivePosture::None {
                    return Err(LocalShardingPolicyError::LayoutCollectivePostureMismatch {
                        layout,
                        collective_posture,
                    });
                }
            }
            ShardedModelLayoutKind::LayerSharded | ShardedModelLayoutKind::TensorSharded => {
                if execution_mode != LocalShardingExecutionMode::Partitioned {
                    return Err(LocalShardingPolicyError::LayoutExecutionModeMismatch {
                        layout,
                        execution_mode,
                    });
                }
                if collective_posture != LocalCollectivePosture::CallerManagedFuture {
                    return Err(LocalShardingPolicyError::LayoutCollectivePostureMismatch {
                        layout,
                        collective_posture,
                    });
                }
            }
        }
        if outcome_posture != LocalExecutionOutcomePosture::MetricsOnlyEvidence {
            return Err(LocalShardingPolicyError::LayoutOutcomePostureMismatch {
                layout,
                outcome_posture,
            });
        }
        Ok(Self::build(
            policy_id,
            policy_version,
            layout,
            execution_mode,
            collective_posture,
            outcome_posture,
            None,
        ))
    }

    /// Returns the current default v1 policy for one layout family.
    #[must_use]
    pub fn for_layout(layout: ShardedModelLayoutKind) -> Self {
        match layout {
            ShardedModelLayoutKind::Replicated => Self::build(
                String::from("local_replicated_metrics_only_v1"),
                1,
                layout,
                LocalShardingExecutionMode::WholeModelOrReplica,
                LocalCollectivePosture::None,
                LocalExecutionOutcomePosture::MetricsOnlyEvidence,
                Some(String::from(
                    "the first-cut local multi-device runtime records per-replica metrics and evidence but does not yet claim replica output reconciliation",
                )),
            ),
            ShardedModelLayoutKind::LayerSharded => Self::build(
                String::from("local_layer_sharded_metrics_only_v1"),
                1,
                layout,
                LocalShardingExecutionMode::Partitioned,
                LocalCollectivePosture::CallerManagedFuture,
                LocalExecutionOutcomePosture::MetricsOnlyEvidence,
                Some(String::from(
                    "the first-cut local multi-device runtime can execute layer partitions but keeps activation handoff and output assembly outside the current evidence-only contract",
                )),
            ),
            ShardedModelLayoutKind::TensorSharded => Self::build(
                String::from("local_tensor_sharded_metrics_only_v1"),
                1,
                layout,
                LocalShardingExecutionMode::Partitioned,
                LocalCollectivePosture::CallerManagedFuture,
                LocalExecutionOutcomePosture::MetricsOnlyEvidence,
                Some(String::from(
                    "the first-cut local multi-device runtime can execute tensor partitions but keeps collectives and final output assembly outside the current evidence-only contract",
                )),
            ),
        }
    }

    /// Attaches plain-language policy detail and refreshes the digest.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self.policy_digest = stable_local_sharding_policy_digest(
            self.policy_id.as_str(),
            self.policy_version,
            self.layout,
            self.execution_mode,
            self.collective_posture,
            self.outcome_posture,
            self.detail.as_deref(),
        );
        self
    }

    /// Returns whether the policy requires a partition-capable backend.
    #[must_use]
    pub fn requires_partition_capable_runtime(&self) -> bool {
        self.execution_mode == LocalShardingExecutionMode::Partitioned
    }

    /// Returns whether the policy supports one concrete partition shape.
    #[must_use]
    pub fn supports_partition(&self, partition: &ExecutionPartition) -> bool {
        match (self.execution_mode, partition) {
            (
                LocalShardingExecutionMode::WholeModelOrReplica,
                ExecutionPartition::WholeModel | ExecutionPartition::Replica { .. },
            ) => true,
            (
                LocalShardingExecutionMode::Partitioned,
                ExecutionPartition::LayerRange { .. } | ExecutionPartition::TensorRange { .. },
            ) => true,
            _ => false,
        }
    }

    /// Returns stable signature lines for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("policy_id={}", self.policy_id),
            format!("policy_version={}", self.policy_version),
            format!("layout={:?}", self.layout),
            format!("execution_mode={:?}", self.execution_mode),
            format!("collective_posture={:?}", self.collective_posture),
            format!("outcome_posture={:?}", self.outcome_posture),
            format!("policy_digest={}", self.policy_digest),
        ];
        if let Some(detail) = &self.detail {
            lines.push(format!("detail={detail}"));
        }
        lines
    }

    fn build(
        policy_id: String,
        policy_version: u32,
        layout: ShardedModelLayoutKind,
        execution_mode: LocalShardingExecutionMode,
        collective_posture: LocalCollectivePosture,
        outcome_posture: LocalExecutionOutcomePosture,
        detail: Option<String>,
    ) -> Self {
        let policy_digest = stable_local_sharding_policy_digest(
            policy_id.as_str(),
            policy_version,
            layout,
            execution_mode,
            collective_posture,
            outcome_posture,
            detail.as_deref(),
        );
        Self {
            policy_id,
            policy_version,
            layout,
            execution_mode,
            collective_posture,
            outcome_posture,
            detail,
            policy_digest,
        }
    }
}

/// Stable refusal taxonomy for local multi-device contract or runner boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalMultiDeviceRefusalReason {
    /// One declared weight strategy does not fit the requested layout.
    UnsupportedWeightStrategy,
    /// The selected backend does not match the contract backend.
    SelectionBackendMismatch,
    /// The explicit topology backend does not match the contract backend.
    TopologyBackendMismatch,
    /// The explicit topology kind does not match the contract layout.
    TopologyLayoutMismatch,
    /// The realized device count falls outside the contract bounds.
    DeviceCountOutOfBounds,
    /// The selected-device set does not match the topology shard count.
    SelectionDeviceCountMismatch,
    /// The selected-device set duplicated one stable device identifier.
    DuplicateSelectedDevice,
    /// One selected device belongs to the wrong backend family.
    SelectionDeviceBackendMismatch,
    /// One topology assignment referenced a device the selection did not surface.
    MissingSelectedDevice,
    /// One selected device did not surface the memory contract the sharding contract requires.
    DeviceMemoryUnknown,
    /// One selected device did not meet the required memory floor.
    InsufficientDeviceMemory,
    /// One realized partition does not fit the declared sharding policy.
    PolicyPartitionMismatch,
    /// Local multi-device execution requires an explicit topology.
    MissingExecutionTopology,
    /// The local runner does not implement the requested topology kind.
    UnsupportedTopologyKind,
    /// The sharded-model manifest does not match the realized topology contract.
    ManifestTopologyMismatch,
    /// The runtime set does not cover one selected shard device.
    MissingRuntimeForShardDevice,
}

impl LocalMultiDeviceRefusalReason {
    /// Returns the stable subject label for the refusal reason.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedWeightStrategy => "unsupported_weight_strategy",
            Self::SelectionBackendMismatch => "selection_backend_mismatch",
            Self::TopologyBackendMismatch => "topology_backend_mismatch",
            Self::TopologyLayoutMismatch => "topology_layout_mismatch",
            Self::DeviceCountOutOfBounds => "device_count_out_of_bounds",
            Self::SelectionDeviceCountMismatch => "selection_device_count_mismatch",
            Self::DuplicateSelectedDevice => "duplicate_selected_device",
            Self::SelectionDeviceBackendMismatch => "selection_device_backend_mismatch",
            Self::MissingSelectedDevice => "missing_selected_device",
            Self::DeviceMemoryUnknown => "device_memory_unknown",
            Self::InsufficientDeviceMemory => "insufficient_device_memory",
            Self::PolicyPartitionMismatch => "policy_partition_mismatch",
            Self::MissingExecutionTopology => "missing_execution_topology",
            Self::UnsupportedTopologyKind => "unsupported_topology_kind",
            Self::ManifestTopologyMismatch => "manifest_topology_mismatch",
            Self::MissingRuntimeForShardDevice => "missing_runtime_for_shard_device",
        }
    }

    const fn refusal_code(self) -> PsionicRefusalCode {
        match self {
            Self::UnsupportedWeightStrategy => PsionicRefusalCode::UnsupportedLayout,
            Self::UnsupportedTopologyKind => PsionicRefusalCode::UnsupportedBackendCapability,
            Self::ManifestTopologyMismatch => PsionicRefusalCode::SerializationIncompatibility,
            Self::SelectionBackendMismatch
            | Self::TopologyBackendMismatch
            | Self::TopologyLayoutMismatch
            | Self::DeviceCountOutOfBounds
            | Self::SelectionDeviceCountMismatch
            | Self::DuplicateSelectedDevice
            | Self::SelectionDeviceBackendMismatch
            | Self::MissingSelectedDevice
            | Self::DeviceMemoryUnknown
            | Self::InsufficientDeviceMemory
            | Self::PolicyPartitionMismatch
            | Self::MissingExecutionTopology
            | Self::MissingRuntimeForShardDevice => PsionicRefusalCode::TopologyMismatch,
        }
    }
}

/// Validation failure for one local multi-device sharding contract or bound runtime path.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LocalShardingContractError {
    /// The contract identifier was blank.
    #[error("local sharding contract is missing a contract_id")]
    MissingContractId,
    /// The model-family label was blank.
    #[error("local sharding contract is missing a model_family")]
    MissingModelFamily,
    /// The effective backend label was blank.
    #[error("local sharding contract is missing an effective backend")]
    MissingEffectiveBackend,
    /// The supplied sharding policy does not match the contract layout.
    #[error(
        "local sharding contract layout {contract_layout:?} does not match policy layout {policy_layout:?}"
    )]
    PolicyLayoutMismatch {
        /// Contract layout.
        contract_layout: ShardedModelLayoutKind,
        /// Policy layout.
        policy_layout: ShardedModelLayoutKind,
    },
    /// The contract asked for fewer than two devices.
    #[error("local sharding contract requires at least 2 devices, found {min_device_count}")]
    InvalidMinDeviceCount {
        /// Invalid device-count floor.
        min_device_count: usize,
    },
    /// The max device count was smaller than the minimum.
    #[error(
        "local sharding contract max device count {max_device_count} is smaller than min device count {min_device_count}"
    )]
    InvalidDeviceCountBounds {
        /// Minimum device count.
        min_device_count: usize,
        /// Maximum device count.
        max_device_count: usize,
    },
    /// The contract carried no inspectable weight rules.
    #[error("local sharding contract requires at least one weight rule")]
    MissingWeightRules,
    /// The same weight class appeared more than once.
    #[error("local sharding contract duplicated weight class {class:?}")]
    DuplicateWeightClass {
        /// Duplicated weight class.
        class: LocalModelWeightClass,
    },
    /// One rule declared a strategy incompatible with the layout.
    #[error(
        "local sharding contract layout {layout:?} does not allow strategy {strategy:?} for weight class {class:?}"
    )]
    UnsupportedWeightStrategy {
        /// Declared layout.
        layout: ShardedModelLayoutKind,
        /// Mismatched weight class.
        class: LocalModelWeightClass,
        /// Mismatched strategy.
        strategy: LocalWeightShardingStrategy,
    },
    /// No rule expressed the primary sharding mode for the layout.
    #[error("local sharding contract layout {layout:?} is missing a primary sharding rule")]
    MissingPrimaryShardingRule {
        /// Declared layout.
        layout: ShardedModelLayoutKind,
    },
    /// The current backend selection did not target the contract backend.
    #[error(
        "local sharding contract backend `{contract_backend}` does not match selection backend `{selection_backend}`"
    )]
    SelectionBackendMismatch {
        /// Contract backend.
        contract_backend: String,
        /// Runtime-selected backend.
        selection_backend: String,
    },
    /// The explicit topology did not target the contract backend.
    #[error(
        "local sharding contract backend `{contract_backend}` does not match topology backend `{topology_backend}`"
    )]
    TopologyBackendMismatch {
        /// Contract backend.
        contract_backend: String,
        /// Topology backend.
        topology_backend: String,
    },
    /// The realized topology kind did not match the contract layout.
    #[error(
        "local sharding contract layout {contract_layout:?} does not match topology kind {topology_kind:?}"
    )]
    TopologyLayoutMismatch {
        /// Contract layout.
        contract_layout: ShardedModelLayoutKind,
        /// Topology kind.
        topology_kind: ExecutionTopologyKind,
    },
    /// The realized topology used too few or too many devices.
    #[error(
        "local sharding contract expected between {min_device_count} and {max_device_count:?} devices, found {actual_device_count}"
    )]
    DeviceCountOutOfBounds {
        /// Minimum device count.
        min_device_count: usize,
        /// Maximum device count when the contract sets one.
        max_device_count: Option<usize>,
        /// Realized device count.
        actual_device_count: usize,
    },
    /// The selection and topology disagreed on the shard/device count.
    #[error(
        "local sharding contract expected topology shard count {topology_shards} to match selected device count {selected_devices}"
    )]
    SelectionDeviceCountMismatch {
        /// Topology shard count.
        topology_shards: usize,
        /// Selected device count.
        selected_devices: usize,
    },
    /// The same selected device appeared more than once.
    #[error("local sharding contract duplicated selected device `{stable_device_id}`")]
    DuplicateSelectedDevice {
        /// Duplicated stable device identifier.
        stable_device_id: String,
    },
    /// One selected device belonged to the wrong backend family.
    #[error(
        "local sharding contract expected backend `{expected_backend}` but selected device `{stable_device_id}` belongs to `{actual_backend}`"
    )]
    SelectionDeviceBackendMismatch {
        /// Stable device identifier.
        stable_device_id: String,
        /// Contract backend.
        expected_backend: String,
        /// Selected device backend.
        actual_backend: String,
    },
    /// The topology referenced a device the selection did not surface.
    #[error("local sharding contract could not find selected device `{stable_device_id}`")]
    MissingSelectedDevice {
        /// Missing stable device identifier.
        stable_device_id: String,
    },
    /// Memory posture was required but unknown for one selected device.
    #[error(
        "local sharding contract requires {required_bytes} bytes per device but selected device `{stable_device_id}` did not surface memory capacity"
    )]
    DeviceMemoryUnknown {
        /// Stable device identifier.
        stable_device_id: String,
        /// Required bytes per device.
        required_bytes: u64,
    },
    /// One selected device lacked the required memory budget.
    #[error(
        "local sharding contract requires {required_bytes} bytes per device but selected device `{stable_device_id}` only surfaced {available_bytes} bytes"
    )]
    InsufficientDeviceMemory {
        /// Stable device identifier.
        stable_device_id: String,
        /// Required bytes per device.
        required_bytes: u64,
        /// Available bytes surfaced by the runtime.
        available_bytes: u64,
    },
    /// One topology assignment does not fit the declared local sharding policy.
    #[error("local sharding contract policy `{policy_id}` does not allow partition {partition:?}")]
    PolicyPartitionMismatch {
        /// Stable policy identifier.
        policy_id: String,
        /// Unsupported partition.
        partition: ExecutionPartition,
    },
}

impl LocalShardingContractError {
    /// Returns the typed refusal reason when the contract error is a current
    /// sharding or topology refusal.
    #[must_use]
    pub const fn refusal_reason(&self) -> Option<LocalMultiDeviceRefusalReason> {
        match self {
            Self::UnsupportedWeightStrategy { .. } => {
                Some(LocalMultiDeviceRefusalReason::UnsupportedWeightStrategy)
            }
            Self::SelectionBackendMismatch { .. } => {
                Some(LocalMultiDeviceRefusalReason::SelectionBackendMismatch)
            }
            Self::TopologyBackendMismatch { .. } => {
                Some(LocalMultiDeviceRefusalReason::TopologyBackendMismatch)
            }
            Self::TopologyLayoutMismatch { .. } => {
                Some(LocalMultiDeviceRefusalReason::TopologyLayoutMismatch)
            }
            Self::DeviceCountOutOfBounds { .. } => {
                Some(LocalMultiDeviceRefusalReason::DeviceCountOutOfBounds)
            }
            Self::SelectionDeviceCountMismatch { .. } => {
                Some(LocalMultiDeviceRefusalReason::SelectionDeviceCountMismatch)
            }
            Self::DuplicateSelectedDevice { .. } => {
                Some(LocalMultiDeviceRefusalReason::DuplicateSelectedDevice)
            }
            Self::SelectionDeviceBackendMismatch { .. } => {
                Some(LocalMultiDeviceRefusalReason::SelectionDeviceBackendMismatch)
            }
            Self::MissingSelectedDevice { .. } => {
                Some(LocalMultiDeviceRefusalReason::MissingSelectedDevice)
            }
            Self::DeviceMemoryUnknown { .. } => {
                Some(LocalMultiDeviceRefusalReason::DeviceMemoryUnknown)
            }
            Self::InsufficientDeviceMemory { .. } => {
                Some(LocalMultiDeviceRefusalReason::InsufficientDeviceMemory)
            }
            Self::PolicyPartitionMismatch { .. } => {
                Some(LocalMultiDeviceRefusalReason::PolicyPartitionMismatch)
            }
            Self::MissingContractId
            | Self::MissingModelFamily
            | Self::MissingEffectiveBackend
            | Self::PolicyLayoutMismatch { .. }
            | Self::InvalidMinDeviceCount { .. }
            | Self::InvalidDeviceCountBounds { .. }
            | Self::MissingWeightRules
            | Self::DuplicateWeightClass { .. }
            | Self::MissingPrimaryShardingRule { .. } => None,
        }
    }

    /// Returns the canonical refusal when the sharding contract hit a topology
    /// or layout boundary.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        self.refusal_reason().map(|reason| {
            PsionicRefusal::new(
                reason.refusal_code(),
                PsionicRefusalScope::Topology,
                self.to_string(),
            )
            .with_subject(reason.as_str())
        })
    }
}

/// Inspectable local sharding contract bound to one representative model-family path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalShardingContract {
    /// Stable contract identifier.
    pub contract_id: String,
    /// Stable model-family label such as `gguf_decoder:llama`.
    pub model_family: String,
    /// Backend family this local contract targets.
    pub effective_backend: String,
    /// Layout expected for the local shard set.
    pub layout: ShardedModelLayoutKind,
    /// Minimum device count required by the contract.
    pub min_device_count: usize,
    /// Maximum device count allowed by the contract when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_device_count: Option<usize>,
    /// Whether every selected device must stay in the same backend family.
    pub requires_same_backend_family: bool,
    /// Minimum surfaced device memory required per local shard when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_memory_bytes_per_device: Option<u64>,
    /// Explicit sharding policy for the current layout family.
    pub policy: LocalShardingPolicy,
    /// Inspectable rules for the representative model-family weight classes.
    pub weight_rules: Vec<LocalWeightShardingRule>,
    /// Stable digest over the contract.
    pub contract_digest: String,
}

impl LocalShardingContract {
    /// Creates a local sharding contract and derives its stable digest.
    pub fn new(
        contract_id: impl Into<String>,
        model_family: impl Into<String>,
        effective_backend: impl Into<String>,
        layout: ShardedModelLayoutKind,
        min_device_count: usize,
        max_device_count: Option<usize>,
        min_memory_bytes_per_device: Option<u64>,
        weight_rules: Vec<LocalWeightShardingRule>,
    ) -> Result<Self, LocalShardingContractError> {
        Self::new_with_policy(
            contract_id,
            model_family,
            effective_backend,
            layout,
            min_device_count,
            max_device_count,
            min_memory_bytes_per_device,
            LocalShardingPolicy::for_layout(layout),
            weight_rules,
        )
    }

    /// Creates a local sharding contract with an explicit sharding policy.
    pub fn new_with_policy(
        contract_id: impl Into<String>,
        model_family: impl Into<String>,
        effective_backend: impl Into<String>,
        layout: ShardedModelLayoutKind,
        min_device_count: usize,
        max_device_count: Option<usize>,
        min_memory_bytes_per_device: Option<u64>,
        policy: LocalShardingPolicy,
        weight_rules: Vec<LocalWeightShardingRule>,
    ) -> Result<Self, LocalShardingContractError> {
        let contract_id = contract_id.into();
        if contract_id.trim().is_empty() {
            return Err(LocalShardingContractError::MissingContractId);
        }
        let model_family = model_family.into();
        if model_family.trim().is_empty() {
            return Err(LocalShardingContractError::MissingModelFamily);
        }
        let effective_backend = effective_backend.into();
        if effective_backend.trim().is_empty() {
            return Err(LocalShardingContractError::MissingEffectiveBackend);
        }
        if policy.layout != layout {
            return Err(LocalShardingContractError::PolicyLayoutMismatch {
                contract_layout: layout,
                policy_layout: policy.layout,
            });
        }
        if min_device_count < 2 {
            return Err(LocalShardingContractError::InvalidMinDeviceCount { min_device_count });
        }
        if let Some(max_device_count) = max_device_count {
            if max_device_count < min_device_count {
                return Err(LocalShardingContractError::InvalidDeviceCountBounds {
                    min_device_count,
                    max_device_count,
                });
            }
        }
        if weight_rules.is_empty() {
            return Err(LocalShardingContractError::MissingWeightRules);
        }
        let mut seen_classes = BTreeSet::new();
        let mut has_primary_rule = false;
        for rule in &weight_rules {
            if !seen_classes.insert(rule.class) {
                return Err(LocalShardingContractError::DuplicateWeightClass { class: rule.class });
            }
            if !rule.strategy.matches_layout(layout) {
                return Err(LocalShardingContractError::UnsupportedWeightStrategy {
                    layout,
                    class: rule.class,
                    strategy: rule.strategy,
                });
            }
            has_primary_rule |= rule.strategy.is_primary_for_layout(layout);
        }
        if !has_primary_rule {
            return Err(LocalShardingContractError::MissingPrimaryShardingRule { layout });
        }
        let contract_digest = stable_local_sharding_contract_digest(
            contract_id.as_str(),
            model_family.as_str(),
            effective_backend.as_str(),
            layout,
            min_device_count,
            max_device_count,
            min_memory_bytes_per_device,
            &policy,
            weight_rules.as_slice(),
        );
        Ok(Self {
            contract_id,
            model_family,
            effective_backend,
            layout,
            min_device_count,
            max_device_count,
            requires_same_backend_family: true,
            min_memory_bytes_per_device,
            policy,
            weight_rules,
            contract_digest,
        })
    }

    /// Validates a runtime backend selection plus explicit topology against this contract.
    pub fn validate_selection(
        &self,
        selection: &BackendSelection,
        topology: &ExecutionTopologyPlan,
    ) -> Result<(), LocalShardingContractError> {
        if selection.effective_backend != self.effective_backend {
            return Err(LocalShardingContractError::SelectionBackendMismatch {
                contract_backend: self.effective_backend.clone(),
                selection_backend: selection.effective_backend.clone(),
            });
        }
        if topology.effective_backend != self.effective_backend {
            return Err(LocalShardingContractError::TopologyBackendMismatch {
                contract_backend: self.effective_backend.clone(),
                topology_backend: topology.effective_backend.clone(),
            });
        }
        if topology.kind != self.layout.topology_kind() {
            return Err(LocalShardingContractError::TopologyLayoutMismatch {
                contract_layout: self.layout,
                topology_kind: topology.kind,
            });
        }
        let actual_device_count = topology.assignments.len();
        if actual_device_count < self.min_device_count
            || self
                .max_device_count
                .is_some_and(|max_device_count| actual_device_count > max_device_count)
        {
            return Err(LocalShardingContractError::DeviceCountOutOfBounds {
                min_device_count: self.min_device_count,
                max_device_count: self.max_device_count,
                actual_device_count,
            });
        }

        let selected_device_descriptors = selection.selected_devices();
        if selected_device_descriptors.len() != actual_device_count {
            return Err(LocalShardingContractError::SelectionDeviceCountMismatch {
                topology_shards: actual_device_count,
                selected_devices: selected_device_descriptors.len(),
            });
        }

        let mut inventory_by_id = BTreeMap::new();
        for (descriptor, inventory) in selected_device_descriptors
            .into_iter()
            .zip(selection.selected_devices_inventory())
        {
            if inventory_by_id
                .insert(inventory.stable_device_id.clone(), inventory.clone())
                .is_some()
            {
                return Err(LocalShardingContractError::DuplicateSelectedDevice {
                    stable_device_id: inventory.stable_device_id,
                });
            }
            if self.requires_same_backend_family && descriptor.backend != self.effective_backend {
                return Err(LocalShardingContractError::SelectionDeviceBackendMismatch {
                    stable_device_id: inventory.stable_device_id,
                    expected_backend: self.effective_backend.clone(),
                    actual_backend: descriptor.backend.clone(),
                });
            }
        }

        for assignment in &topology.assignments {
            if !self.policy.supports_partition(&assignment.partition) {
                return Err(LocalShardingContractError::PolicyPartitionMismatch {
                    policy_id: self.policy.policy_id.clone(),
                    partition: assignment.partition.clone(),
                });
            }
            let Some(device_inventory) = inventory_by_id.get(&assignment.device.stable_device_id)
            else {
                return Err(LocalShardingContractError::MissingSelectedDevice {
                    stable_device_id: assignment.device.stable_device_id.clone(),
                });
            };
            if let Some(required_bytes) = self.min_memory_bytes_per_device {
                let Some(available_bytes) = device_inventory
                    .free_memory_bytes
                    .or(device_inventory.total_memory_bytes)
                else {
                    return Err(LocalShardingContractError::DeviceMemoryUnknown {
                        stable_device_id: assignment.device.stable_device_id.clone(),
                        required_bytes,
                    });
                };
                if available_bytes < required_bytes {
                    return Err(LocalShardingContractError::InsufficientDeviceMemory {
                        stable_device_id: assignment.device.stable_device_id.clone(),
                        required_bytes,
                        available_bytes,
                    });
                }
            }
        }

        Ok(())
    }
}

/// Explicit local multi-device execution request bound to one runtime plan.
pub struct LocalMultiDeviceExecutionRequest<'a, B> {
    /// Compiled or logical plan to execute.
    pub plan: &'a ExecutionPlan,
    /// Backend selection that surfaced the explicit topology.
    pub backend_selection: &'a BackendSelection,
    /// Declarative local sharding contract for the model-family path.
    pub sharding_contract: &'a LocalShardingContract,
    /// Explicit manifest binding shard artifacts to the realized topology.
    pub sharded_model_manifest: &'a ShardedModelManifest,
    /// Host-supplied inputs for the plan.
    pub inputs: &'a BTreeMap<TensorId, B>,
}

impl<'a, B> LocalMultiDeviceExecutionRequest<'a, B> {
    /// Creates a local multi-device execution request.
    #[must_use]
    pub fn new(
        plan: &'a ExecutionPlan,
        backend_selection: &'a BackendSelection,
        sharding_contract: &'a LocalShardingContract,
        sharded_model_manifest: &'a ShardedModelManifest,
        inputs: &'a BTreeMap<TensorId, B>,
    ) -> Self {
        Self {
            plan,
            backend_selection,
            sharding_contract,
            sharded_model_manifest,
            inputs,
        }
    }
}

/// One local shard execution report emitted by the multi-device runner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalShardExecutionReport {
    /// Stable shard identifier.
    pub shard_id: usize,
    /// Stable selected-device identifier.
    pub stable_device_id: String,
    /// Logical partition executed on the device.
    pub partition: ExecutionPartition,
    /// Stable shard artifact reference selected for the partition.
    pub artifact_id: String,
    /// Stable digest of the shard artifact.
    pub artifact_digest: String,
    /// Stable provenance digest for the shard bytes when surfaced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_digest: Option<String>,
    /// Metrics surfaced by the backend for this local shard.
    pub metrics: ExecutionMetrics,
}

impl LocalShardExecutionReport {
    fn new(
        assignment: &ExecutionShardAssignment,
        shard_artifact: &ShardedModelArtifactRef,
        metrics: ExecutionMetrics,
    ) -> Self {
        Self {
            shard_id: assignment.shard_id,
            stable_device_id: assignment.device.stable_device_id.clone(),
            partition: assignment.partition.clone(),
            artifact_id: shard_artifact.artifact_id.clone(),
            artifact_digest: shard_artifact.artifact_digest.clone(),
            provenance_digest: shard_artifact.provenance_digest.clone(),
            metrics,
        }
    }
}

/// Local-only execution report for one same-type multi-device run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalMultiDeviceExecutionReport {
    /// Runtime backend that executed the local shard set.
    pub runtime_backend: String,
    /// Stable model-family label from the sharding contract.
    pub model_family: String,
    /// Stable sharding contract identifier.
    pub sharding_contract_id: String,
    /// Stable sharding contract digest.
    pub sharding_contract_digest: String,
    /// Stable sharding policy identifier.
    pub sharding_policy_id: String,
    /// Stable sharding policy digest.
    pub sharding_policy_digest: String,
    /// Stable sharded-model manifest identifier.
    pub sharded_model_manifest_id: String,
    /// Stable sharded-model manifest digest.
    pub sharded_model_manifest_digest: String,
    /// Realized local execution topology.
    pub execution_topology: ExecutionTopologyPlan,
    /// Selected-device inventory qualifiers for the run.
    pub selected_devices: Vec<DeviceInventoryQualifiers>,
    /// Aggregate metrics across all local shard executions.
    pub aggregate_metrics: ExecutionMetrics,
    /// Per-shard execution reports in shard order.
    pub shard_reports: Vec<LocalShardExecutionReport>,
    /// Local delivered-execution context, intentionally separate from clustered execution.
    pub delivered_execution: DeliveredExecutionContext,
}

/// Explicit refusal or execution failure for one same-type local multi-device run.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LocalMultiDeviceExecutionError {
    /// The caller supplied no runtime instances.
    #[error("local multi-device execution requires at least one runtime instance")]
    MissingRuntimeInstances,
    /// The backend selection did not surface a topology plan.
    #[error("local multi-device execution requires an explicit execution topology")]
    MissingExecutionTopology,
    /// The topology kind is outside the current local runner scope.
    #[error("local multi-device execution does not support topology kind {kind:?}")]
    UnsupportedTopologyKind {
        /// Unsupported topology kind.
        kind: ExecutionTopologyKind,
    },
    /// The sharding contract or selection validation refused the request.
    #[error(transparent)]
    Contract(#[from] LocalShardingContractError),
    /// The sharded-model manifest did not match the realized topology.
    #[error(transparent)]
    Manifest(#[from] ShardedModelManifestError),
    /// Runtime device discovery failed while binding one local shard.
    #[error("local multi-device runtime discovery failed for backend `{backend}`: {message}")]
    RuntimeDiscoveryFailed {
        /// Backend family being queried.
        backend: String,
        /// Runtime error detail.
        message: String,
    },
    /// No supplied runtime instance could serve the shard device.
    #[error(
        "local multi-device execution could not find a runtime for shard {shard_id} on device `{stable_device_id}`"
    )]
    MissingRuntimeForShardDevice {
        /// Shard identifier.
        shard_id: usize,
        /// Stable device identifier.
        stable_device_id: String,
    },
    /// One shard execution failed after the local path was bound.
    #[error(
        "local multi-device execution failed for shard {shard_id} on device `{stable_device_id}`: {message}"
    )]
    ShardExecutionFailed {
        /// Shard identifier.
        shard_id: usize,
        /// Stable device identifier.
        stable_device_id: String,
        /// Runtime error detail.
        message: String,
    },
}

impl LocalMultiDeviceExecutionError {
    /// Returns the typed refusal reason when the execution error reflects a
    /// current capability or topology boundary rather than an operational
    /// runtime failure.
    #[must_use]
    pub fn refusal_reason(&self) -> Option<LocalMultiDeviceRefusalReason> {
        match self {
            Self::MissingExecutionTopology => {
                Some(LocalMultiDeviceRefusalReason::MissingExecutionTopology)
            }
            Self::UnsupportedTopologyKind { .. } => {
                Some(LocalMultiDeviceRefusalReason::UnsupportedTopologyKind)
            }
            Self::Contract(error) => error.refusal_reason(),
            Self::Manifest(_) => Some(LocalMultiDeviceRefusalReason::ManifestTopologyMismatch),
            Self::MissingRuntimeForShardDevice { .. } => {
                Some(LocalMultiDeviceRefusalReason::MissingRuntimeForShardDevice)
            }
            Self::MissingRuntimeInstances
            | Self::RuntimeDiscoveryFailed { .. }
            | Self::ShardExecutionFailed { .. } => None,
        }
    }

    /// Returns the canonical refusal when the execution error reflects an
    /// explicit local multi-device capability or topology boundary.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        self.refusal_reason().map(|reason| {
            PsionicRefusal::new(
                reason.refusal_code(),
                PsionicRefusalScope::Topology,
                self.to_string(),
            )
            .with_subject(reason.as_str())
        })
    }
}

/// Backend contract for same-type local multi-device partition execution.
pub trait LocalShardExecutionBackend: DeviceDiscovery + ExecutionBackend {
    /// Executes one topology assignment on the backend-selected local device.
    fn execute_partition(
        &mut self,
        plan: &ExecutionPlan,
        assignment: &ExecutionShardAssignment,
        inputs: &BTreeMap<TensorId, Self::Buffer>,
    ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError>;
}

/// Default whole-model or replica execution helper for backends adopting the local runner.
pub fn execute_local_partition_or_refuse<B>(
    backend: &mut B,
    plan: &ExecutionPlan,
    assignment: &ExecutionShardAssignment,
    inputs: &BTreeMap<TensorId, B::Buffer>,
) -> Result<ExecutionResult<B::Buffer>, RuntimeError>
where
    B: DeviceDiscovery + ExecutionBackend,
{
    match assignment.partition {
        ExecutionPartition::WholeModel | ExecutionPartition::Replica { .. } => {
            backend.execute(plan, inputs)
        }
        ExecutionPartition::LayerRange { .. } => Err(RuntimeError::Backend(format!(
            "backend `{}` does not implement local layer-sharded partition execution yet",
            backend.backend_name()
        ))),
        ExecutionPartition::TensorRange { .. } => Err(RuntimeError::Backend(format!(
            "backend `{}` does not implement local tensor-sharded partition execution yet",
            backend.backend_name()
        ))),
    }
}

/// Executes one same-type local multi-device plan against partition-capable runtimes.
pub fn execute_local_multi_device_plan<R>(
    runtimes: &mut [R],
    request: LocalMultiDeviceExecutionRequest<'_, R::Buffer>,
) -> Result<LocalMultiDeviceExecutionReport, LocalMultiDeviceExecutionError>
where
    R: LocalShardExecutionBackend,
{
    if runtimes.is_empty() {
        return Err(LocalMultiDeviceExecutionError::MissingRuntimeInstances);
    }

    let Some(execution_topology) = request.backend_selection.execution_topology_plan() else {
        return Err(LocalMultiDeviceExecutionError::MissingExecutionTopology);
    };
    match execution_topology.kind {
        ExecutionTopologyKind::Replicated
        | ExecutionTopologyKind::LayerSharded
        | ExecutionTopologyKind::TensorSharded => {}
        kind => {
            return Err(LocalMultiDeviceExecutionError::UnsupportedTopologyKind { kind });
        }
    }

    request
        .sharding_contract
        .validate_selection(request.backend_selection, &execution_topology)?;
    request
        .sharded_model_manifest
        .validate_against_topology(&execution_topology)?;

    let selected_devices = request.backend_selection.selected_devices_inventory();
    let shard_artifacts = request
        .sharded_model_manifest
        .shards
        .iter()
        .map(|shard| (shard.shard_id, shard))
        .collect::<BTreeMap<_, _>>();
    let mut shard_reports = Vec::with_capacity(execution_topology.assignments.len());
    let mut aggregate_metrics = ExecutionMetrics {
        execution_plan_digest: Some(request.plan.stable_digest()),
        ..ExecutionMetrics::default()
    };

    for assignment in &execution_topology.assignments {
        let Some(shard_artifact) = shard_artifacts.get(&assignment.shard_id) else {
            return Err(LocalMultiDeviceExecutionError::Manifest(
                ShardedModelManifestError::TopologyShardMissing {
                    shard_id: assignment.shard_id,
                },
            ));
        };

        let mut shard_result = None;
        for runtime in runtimes.iter_mut() {
            if runtime.backend_name() != execution_topology.effective_backend {
                continue;
            }
            let discovered_devices = runtime.discover_devices().map_err(|error| {
                LocalMultiDeviceExecutionError::RuntimeDiscoveryFailed {
                    backend: runtime.backend_name().to_string(),
                    message: error.to_string(),
                }
            })?;
            let serves_device = discovered_devices.iter().any(|device| {
                device.inventory_qualifiers().stable_device_id == assignment.device.stable_device_id
            });
            if !serves_device {
                continue;
            }
            let result = runtime
                .execute_partition(request.plan, assignment, request.inputs)
                .map_err(
                    |error| LocalMultiDeviceExecutionError::ShardExecutionFailed {
                        shard_id: assignment.shard_id,
                        stable_device_id: assignment.device.stable_device_id.clone(),
                        message: error.to_string(),
                    },
                )?;
            shard_result = Some(result.metrics);
            break;
        }

        let Some(metrics) = shard_result else {
            return Err(
                LocalMultiDeviceExecutionError::MissingRuntimeForShardDevice {
                    shard_id: assignment.shard_id,
                    stable_device_id: assignment.device.stable_device_id.clone(),
                },
            );
        };
        accumulate_metrics(&mut aggregate_metrics, &metrics);
        shard_reports.push(LocalShardExecutionReport::new(
            assignment,
            shard_artifact,
            metrics,
        ));
    }

    Ok(LocalMultiDeviceExecutionReport {
        runtime_backend: execution_topology.effective_backend.clone(),
        model_family: request.sharding_contract.model_family.clone(),
        sharding_contract_id: request.sharding_contract.contract_id.clone(),
        sharding_contract_digest: request.sharding_contract.contract_digest.clone(),
        sharding_policy_id: request.sharding_contract.policy.policy_id.clone(),
        sharding_policy_digest: request.sharding_contract.policy.policy_digest.clone(),
        sharded_model_manifest_id: request.sharded_model_manifest.manifest_id.clone(),
        sharded_model_manifest_digest: request.sharded_model_manifest.stable_digest(),
        execution_topology: execution_topology.clone(),
        selected_devices: selected_devices.clone(),
        aggregate_metrics,
        shard_reports,
        delivered_execution: DeliveredExecutionContext::new(
            request.backend_selection.effective_backend.clone(),
            Some(execution_topology),
            selected_devices,
        ),
    })
}

fn accumulate_metrics(total: &mut ExecutionMetrics, metrics: &ExecutionMetrics) {
    total.steps_executed += metrics.steps_executed;
    total.kernel_count += metrics.kernel_count;
    total.bytes_moved += metrics.bytes_moved;
    total.plan_cache_hits += metrics.plan_cache_hits;
    total.plan_cache_misses += metrics.plan_cache_misses;
    if total.compile_path.is_none() {
        total.compile_path = metrics.compile_path.clone();
    }
}

fn stable_local_sharding_contract_digest(
    contract_id: &str,
    model_family: &str,
    effective_backend: &str,
    layout: ShardedModelLayoutKind,
    min_device_count: usize,
    max_device_count: Option<usize>,
    min_memory_bytes_per_device: Option<u64>,
    policy: &LocalShardingPolicy,
    weight_rules: &[LocalWeightShardingRule],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"local_sharding_contract|");
    hasher.update(contract_id.as_bytes());
    hasher.update(b"|");
    hasher.update(model_family.as_bytes());
    hasher.update(b"|");
    hasher.update(effective_backend.as_bytes());
    hasher.update(b"|");
    hasher.update(match layout {
        ShardedModelLayoutKind::Replicated => b"replicated".as_slice(),
        ShardedModelLayoutKind::LayerSharded => b"layer_sharded".as_slice(),
        ShardedModelLayoutKind::TensorSharded => b"tensor_sharded".as_slice(),
    });
    hasher.update(b"|min|");
    hasher.update(min_device_count.to_string().as_bytes());
    hasher.update(b"|max|");
    hasher.update(
        max_device_count
            .map_or_else(String::new, |value| value.to_string())
            .as_bytes(),
    );
    hasher.update(b"|memory|");
    hasher.update(
        min_memory_bytes_per_device
            .map_or_else(String::new, |value| value.to_string())
            .as_bytes(),
    );
    hasher.update(b"|policy|");
    hasher.update(policy.policy_digest.as_bytes());
    for rule in weight_rules {
        hasher.update(b"|rule|");
        hasher.update(format!("{:?}", rule.class).as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", rule.strategy).as_bytes());
        hasher.update(b"|");
        hasher.update(rule.detail.as_deref().unwrap_or_default().as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_local_sharding_policy_digest(
    policy_id: &str,
    policy_version: u32,
    layout: ShardedModelLayoutKind,
    execution_mode: LocalShardingExecutionMode,
    collective_posture: LocalCollectivePosture,
    outcome_posture: LocalExecutionOutcomePosture,
    detail: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"local_sharding_policy|");
    hasher.update(policy_id.as_bytes());
    hasher.update(b"|version|");
    hasher.update(policy_version.to_string().as_bytes());
    hasher.update(b"|layout|");
    hasher.update(match layout {
        ShardedModelLayoutKind::Replicated => b"replicated".as_slice(),
        ShardedModelLayoutKind::LayerSharded => b"layer_sharded".as_slice(),
        ShardedModelLayoutKind::TensorSharded => b"tensor_sharded".as_slice(),
    });
    hasher.update(b"|execution|");
    hasher.update(match execution_mode {
        LocalShardingExecutionMode::WholeModelOrReplica => b"whole_model_or_replica".as_slice(),
        LocalShardingExecutionMode::Partitioned => b"partitioned".as_slice(),
    });
    hasher.update(b"|collective|");
    hasher.update(match collective_posture {
        LocalCollectivePosture::None => b"none".as_slice(),
        LocalCollectivePosture::CallerManagedFuture => b"caller_managed_future".as_slice(),
    });
    hasher.update(b"|outcome|");
    hasher.update(match outcome_posture {
        LocalExecutionOutcomePosture::MetricsOnlyEvidence => b"metrics_only_evidence".as_slice(),
    });
    if let Some(detail) = detail {
        hasher.update(b"|detail|");
        hasher.update(detail.as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

    use std::collections::BTreeMap;

    use psionic_core::{
        DType, Device, DeviceKind, PsionicRefusalCode, PsionicRefusalScope, QuantizationMode,
        Shape, TensorId, TensorSpec,
    };
    use psionic_ir::{ExecutionOp, ExecutionPlan, ExecutionStep};

    use crate::{
        BackendSelection, BackendToolchainIdentity, BufferHandle, DeviceDescriptor,
        ExecutionPartition, ExecutionResult, HealthStatus, LocalShardExecutionBackend,
        QuantizationExecution, QuantizationLoadPath, QuantizationSupport, RuntimeError,
        RuntimeHealth, ServedArtifactIdentity, ServedProductBackendPolicy,
    };

    use super::{
        execute_local_multi_device_plan, LocalCollectivePosture, LocalExecutionOutcomePosture,
        LocalModelWeightClass, LocalMultiDeviceExecutionError, LocalMultiDeviceExecutionRequest,
        LocalMultiDeviceRefusalReason, LocalShardingContract, LocalShardingContractError,
        LocalShardingExecutionMode, LocalShardingPolicy, LocalShardingPolicyError,
        LocalWeightShardingRule, LocalWeightShardingStrategy, ShardedModelArtifactRef,
        ShardedModelLayoutKind, ShardedModelManifest,
    };

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct MockBuffer {
        spec: TensorSpec,
    }

    impl BufferHandle for MockBuffer {
        fn spec(&self) -> &TensorSpec {
            &self.spec
        }
    }

    #[derive(Clone, Debug)]
    struct MockLocalShardRuntime {
        device: DeviceDescriptor,
        executed_shards: Vec<usize>,
    }

    impl MockLocalShardRuntime {
        fn new(device: DeviceDescriptor) -> Self {
            Self {
                device,
                executed_shards: Vec::new(),
            }
        }
    }

    impl crate::DeviceDiscovery for MockLocalShardRuntime {
        fn backend_name(&self) -> crate::BackendName {
            "cuda"
        }

        fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
            Ok(vec![self.device.clone()])
        }

        fn health(&self) -> RuntimeHealth {
            RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("ready"),
            }
        }
    }

    impl crate::ExecutionBackend for MockLocalShardRuntime {
        type Buffer = MockBuffer;

        fn execute(
            &mut self,
            plan: &ExecutionPlan,
            _inputs: &BTreeMap<TensorId, Self::Buffer>,
        ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
            Ok(ExecutionResult {
                outputs: BTreeMap::new(),
                metrics: crate::ExecutionMetrics {
                    steps_executed: plan.steps.len(),
                    kernel_count: plan.steps.len(),
                    bytes_moved: 0,
                    plan_cache_hits: 0,
                    plan_cache_misses: 0,
                    execution_plan_digest: Some(plan.stable_digest()),
                    compile_path: None,
                },
            })
        }
    }

    impl LocalShardExecutionBackend for MockLocalShardRuntime {
        fn execute_partition(
            &mut self,
            plan: &ExecutionPlan,
            assignment: &crate::ExecutionShardAssignment,
            _inputs: &BTreeMap<TensorId, Self::Buffer>,
        ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
            self.executed_shards.push(assignment.shard_id);
            Ok(ExecutionResult {
                outputs: BTreeMap::new(),
                metrics: crate::ExecutionMetrics {
                    steps_executed: plan.steps.len(),
                    kernel_count: 1,
                    bytes_moved: 1024 * (assignment.shard_id as u64 + 1),
                    plan_cache_hits: assignment.shard_id,
                    plan_cache_misses: 1,
                    execution_plan_digest: Some(format!(
                        "{}#shard{}",
                        plan.stable_digest(),
                        assignment.shard_id
                    )),
                    compile_path: None,
                },
            })
        }
    }

    #[test]
    fn local_sharding_policy_defaults_are_serializable_and_metrics_only(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let policy = LocalShardingPolicy::for_layout(ShardedModelLayoutKind::TensorSharded);

        assert_eq!(policy.policy_version, 1);
        assert_eq!(policy.layout, ShardedModelLayoutKind::TensorSharded);
        assert_eq!(
            policy.execution_mode,
            LocalShardingExecutionMode::Partitioned
        );
        assert_eq!(
            policy.collective_posture,
            LocalCollectivePosture::CallerManagedFuture
        );
        assert_eq!(
            policy.outcome_posture,
            LocalExecutionOutcomePosture::MetricsOnlyEvidence
        );
        assert!(policy.requires_partition_capable_runtime());
        assert!(policy.supports_partition(&ExecutionPartition::TensorRange {
            axis: 1,
            start: 0,
            end: 32,
        }));
        assert!(!policy.supports_partition(&ExecutionPartition::WholeModel));
        assert!(!policy.policy_digest.is_empty());

        let encoded = serde_json::to_string(&policy)?;
        let decoded: LocalShardingPolicy = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, policy);
        assert!(decoded
            .stable_signature_lines()
            .iter()
            .any(|line| line.starts_with("policy_digest=")));
        Ok(())
    }

    #[test]
    fn local_sharding_policy_refuses_invalid_layout_execution_pair() {
        let error = LocalShardingPolicy::new(
            "bad-policy",
            1,
            ShardedModelLayoutKind::Replicated,
            LocalShardingExecutionMode::Partitioned,
            LocalCollectivePosture::CallerManagedFuture,
            LocalExecutionOutcomePosture::MetricsOnlyEvidence,
        )
        .expect_err("replicated layout should refuse partitioned mode");
        assert_eq!(
            error,
            LocalShardingPolicyError::LayoutExecutionModeMismatch {
                layout: ShardedModelLayoutKind::Replicated,
                execution_mode: LocalShardingExecutionMode::Partitioned,
            }
        );
    }

    #[test]
    fn local_multi_device_plan_runner_executes_tensor_sharded_workload_without_cluster_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let device0 = sample_cuda_device(0, 16 * 1024 * 1024 * 1024);
        let device1 = sample_cuda_device(1, 16 * 1024 * 1024 * 1024);
        let execution_topology = crate::ExecutionTopologyPlan::tensor_sharded(
            "cuda",
            1,
            vec![
                (device0.inventory_qualifiers(), 0, 32),
                (device1.inventory_qualifiers(), 32, 64),
            ],
        );
        let selection = BackendSelection::direct_with_policy(
            "cuda",
            Some(device0.clone()),
            vec![String::from("matmul"), String::from("add")],
            ServedProductBackendPolicy::same_backend_only(),
        )
        .with_selected_devices(vec![device0.clone(), device1.clone()])
        .with_execution_topology(Some(execution_topology.clone()));

        let contract = LocalShardingContract::new(
            "gguf-decoder-llama-tp-v1",
            "gguf_decoder:llama",
            "cuda",
            ShardedModelLayoutKind::TensorSharded,
            2,
            Some(2),
            Some(8 * 1024 * 1024 * 1024),
            vec![
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::TokenEmbedding,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::AttentionQuery,
                    LocalWeightShardingStrategy::TensorAxis { axis: 0 },
                ),
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::AttentionOutput,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::KvCache,
                    LocalWeightShardingStrategy::Replicated,
                ),
            ],
        )?;
        let manifest = sample_tensor_manifest()?;
        let plan = sample_plan();
        let inputs = BTreeMap::new();
        let mut runtimes = vec![
            MockLocalShardRuntime::new(device0.clone()),
            MockLocalShardRuntime::new(device1.clone()),
        ];

        let report = execute_local_multi_device_plan(
            runtimes.as_mut_slice(),
            LocalMultiDeviceExecutionRequest::new(&plan, &selection, &contract, &manifest, &inputs),
        )?;

        assert_eq!(report.runtime_backend, "cuda");
        assert_eq!(report.model_family, "gguf_decoder:llama");
        assert_eq!(
            report.sharding_policy_id,
            "local_tensor_sharded_metrics_only_v1"
        );
        assert_eq!(report.sharding_policy_digest, contract.policy.policy_digest);
        assert_eq!(
            report.execution_topology.kind,
            crate::ExecutionTopologyKind::TensorSharded
        );
        assert_eq!(report.shard_reports.len(), 2);
        assert_eq!(
            report.aggregate_metrics.steps_executed,
            plan.steps.len() * 2
        );
        assert_eq!(report.aggregate_metrics.kernel_count, 2);
        assert_eq!(report.aggregate_metrics.bytes_moved, 3 * 1024);
        assert_eq!(report.delivered_execution.cluster_execution, None);
        assert_eq!(
            report.delivered_execution.topology_kind(),
            Some(crate::ExecutionTopologyKind::TensorSharded)
        );
        assert_eq!(runtimes[0].executed_shards, vec![0]);
        assert_eq!(runtimes[1].executed_shards, vec![1]);
        assert_eq!(report.shard_reports[0].artifact_id, "decoder.tensor0_32");
        assert_eq!(report.shard_reports[1].artifact_id, "decoder.tensor32_64");
        Ok(())
    }

    #[test]
    fn local_sharding_contract_refuses_backend_memory_and_device_count_mismatches(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let device0 = sample_cuda_device(0, 4 * 1024 * 1024 * 1024);
        let device1 = sample_cuda_device(1, 4 * 1024 * 1024 * 1024);
        let execution_topology = crate::ExecutionTopologyPlan::tensor_sharded(
            "cuda",
            1,
            vec![
                (device0.inventory_qualifiers(), 0, 32),
                (device1.inventory_qualifiers(), 32, 64),
            ],
        );
        let contract = LocalShardingContract::new(
            "gguf-decoder-llama-tp-v1",
            "gguf_decoder:llama",
            "cuda",
            ShardedModelLayoutKind::TensorSharded,
            2,
            Some(2),
            Some(8 * 1024 * 1024 * 1024),
            vec![
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::TokenEmbedding,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::AttentionOutput,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
            ],
        )?;

        let metal_selection = BackendSelection::direct_with_policy(
            "metal",
            Some(device0.clone()),
            vec![String::from("matmul")],
            ServedProductBackendPolicy::same_backend_only(),
        )
        .with_selected_devices(vec![device0.clone(), device1.clone()])
        .with_execution_topology(Some(execution_topology.clone()));
        let backend_error = contract
            .validate_selection(&metal_selection, &execution_topology)
            .expect_err("backend mismatch should refuse");
        assert_eq!(
            backend_error,
            LocalShardingContractError::SelectionBackendMismatch {
                contract_backend: String::from("cuda"),
                selection_backend: String::from("metal"),
            }
        );

        let low_memory_selection = BackendSelection::direct_with_policy(
            "cuda",
            Some(device0.clone()),
            vec![String::from("matmul")],
            ServedProductBackendPolicy::same_backend_only(),
        )
        .with_selected_devices(vec![device0.clone(), device1.clone()])
        .with_execution_topology(Some(execution_topology.clone()));
        let memory_error = contract
            .validate_selection(&low_memory_selection, &execution_topology)
            .expect_err("insufficient memory should refuse");
        assert_eq!(
            memory_error,
            LocalShardingContractError::InsufficientDeviceMemory {
                stable_device_id: String::from("cuda:0"),
                required_bytes: 8 * 1024 * 1024 * 1024,
                available_bytes: 4 * 1024 * 1024 * 1024,
            }
        );

        let selection_count_error = contract
            .validate_selection(
                &BackendSelection::direct_with_policy(
                    "cuda",
                    Some(device0.clone()),
                    vec![String::from("matmul")],
                    ServedProductBackendPolicy::same_backend_only(),
                )
                .with_selected_devices(vec![device0])
                .with_execution_topology(Some(execution_topology)),
                &crate::ExecutionTopologyPlan::tensor_sharded(
                    "cuda",
                    1,
                    vec![
                        (device1.inventory_qualifiers(), 0, 32),
                        (
                            sample_cuda_device(2, 4 * 1024 * 1024 * 1024).inventory_qualifiers(),
                            32,
                            64,
                        ),
                    ],
                ),
            )
            .expect_err("selection device count mismatch should refuse");
        assert_eq!(
            selection_count_error,
            LocalShardingContractError::SelectionDeviceCountMismatch {
                topology_shards: 2,
                selected_devices: 1,
            }
        );
        Ok(())
    }

    #[test]
    fn local_sharding_contract_refusal_taxonomy_surfaces_topology_mismatch(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let device0 = sample_cuda_device(0, 16 * 1024 * 1024 * 1024);
        let device1 = sample_cuda_device(1, 16 * 1024 * 1024 * 1024);
        let execution_topology = crate::ExecutionTopologyPlan::tensor_sharded(
            "cuda",
            1,
            vec![
                (device0.inventory_qualifiers(), 0, 32),
                (device1.inventory_qualifiers(), 32, 64),
            ],
        );
        let contract = LocalShardingContract::new(
            "gguf-decoder-llama-tp-v1",
            "gguf_decoder:llama",
            "cuda",
            ShardedModelLayoutKind::TensorSharded,
            2,
            Some(2),
            Some(8 * 1024 * 1024 * 1024),
            vec![
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::TokenEmbedding,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::AttentionOutput,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
            ],
        )?;
        let selection = BackendSelection::direct_with_policy(
            "metal",
            Some(device0),
            vec![String::from("matmul")],
            ServedProductBackendPolicy::same_backend_only(),
        )
        .with_selected_devices(vec![device1])
        .with_execution_topology(Some(execution_topology.clone()));
        let refusal = contract
            .validate_selection(&selection, &execution_topology)
            .expect_err("backend mismatch should refuse");
        let refusal = refusal.refusal();
        assert!(refusal.is_some());
        let Some(refusal) = refusal else {
            return Ok(());
        };
        assert_eq!(refusal.code, PsionicRefusalCode::TopologyMismatch);
        assert_eq!(refusal.scope, PsionicRefusalScope::Topology);
        assert_eq!(
            refusal.subject.as_deref(),
            Some("selection_backend_mismatch")
        );
        Ok(())
    }

    #[test]
    fn local_sharding_contract_policy_refuses_partition_kind_mismatch(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let device0 = sample_cuda_device(0, 16 * 1024 * 1024 * 1024);
        let device1 = sample_cuda_device(1, 16 * 1024 * 1024 * 1024);
        let contract = LocalShardingContract::new(
            "gguf-decoder-llama-tp-v1",
            "gguf_decoder:llama",
            "cuda",
            ShardedModelLayoutKind::TensorSharded,
            2,
            Some(2),
            Some(8 * 1024 * 1024 * 1024),
            vec![
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::TokenEmbedding,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::AttentionOutput,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
            ],
        )?;
        let malformed_topology = crate::ExecutionTopologyPlan {
            effective_backend: String::from("cuda"),
            kind: crate::ExecutionTopologyKind::TensorSharded,
            assignments: vec![
                crate::ExecutionShardAssignment {
                    shard_id: 0,
                    device: crate::ExecutionDevicePlacement::from_inventory(
                        &device0.inventory_qualifiers(),
                        0,
                    ),
                    partition: ExecutionPartition::WholeModel,
                },
                crate::ExecutionShardAssignment {
                    shard_id: 1,
                    device: crate::ExecutionDevicePlacement::from_inventory(
                        &device1.inventory_qualifiers(),
                        1,
                    ),
                    partition: ExecutionPartition::WholeModel,
                },
            ],
        };
        let selection = BackendSelection::direct_with_policy(
            "cuda",
            Some(device0.clone()),
            vec![String::from("matmul")],
            ServedProductBackendPolicy::same_backend_only(),
        )
        .with_selected_devices(vec![device0, device1])
        .with_execution_topology(Some(malformed_topology.clone()));

        let error = contract
            .validate_selection(&selection, &malformed_topology)
            .expect_err("whole-model partitions should refuse under tensor policy");
        assert_eq!(
            error,
            LocalShardingContractError::PolicyPartitionMismatch {
                policy_id: String::from("local_tensor_sharded_metrics_only_v1"),
                partition: ExecutionPartition::WholeModel,
            }
        );
        assert_eq!(
            error.refusal_reason(),
            Some(LocalMultiDeviceRefusalReason::PolicyPartitionMismatch)
        );
        Ok(())
    }

    #[test]
    fn local_multi_device_plan_runner_refuses_missing_runtime_for_selected_device(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let device0 = sample_cuda_device(0, 16 * 1024 * 1024 * 1024);
        let device1 = sample_cuda_device(1, 16 * 1024 * 1024 * 1024);
        let execution_topology = crate::ExecutionTopologyPlan::tensor_sharded(
            "cuda",
            1,
            vec![
                (device0.inventory_qualifiers(), 0, 32),
                (device1.inventory_qualifiers(), 32, 64),
            ],
        );
        let selection = BackendSelection::direct_with_policy(
            "cuda",
            Some(device0.clone()),
            vec![String::from("matmul"), String::from("add")],
            ServedProductBackendPolicy::same_backend_only(),
        )
        .with_selected_devices(vec![device0.clone(), device1.clone()])
        .with_execution_topology(Some(execution_topology));
        let contract = LocalShardingContract::new(
            "gguf-decoder-llama-tp-v1",
            "gguf_decoder:llama",
            "cuda",
            ShardedModelLayoutKind::TensorSharded,
            2,
            Some(2),
            Some(8 * 1024 * 1024 * 1024),
            vec![
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::TokenEmbedding,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::AttentionOutput,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
            ],
        )?;
        let manifest = sample_tensor_manifest()?;
        let plan = sample_plan();
        let inputs = BTreeMap::new();
        let mut runtimes = vec![MockLocalShardRuntime::new(device0)];

        let error = execute_local_multi_device_plan(
            runtimes.as_mut_slice(),
            LocalMultiDeviceExecutionRequest::new(&plan, &selection, &contract, &manifest, &inputs),
        )
        .expect_err("missing runtime should refuse");
        assert_eq!(
            error,
            LocalMultiDeviceExecutionError::MissingRuntimeForShardDevice {
                shard_id: 1,
                stable_device_id: String::from("cuda:1"),
            }
        );
        assert_eq!(
            error.refusal_reason(),
            Some(LocalMultiDeviceRefusalReason::MissingRuntimeForShardDevice)
        );
        let refusal = error
            .refusal()
            .expect("missing runtime should map to refusal");
        assert_eq!(refusal.code, PsionicRefusalCode::TopologyMismatch);
        assert_eq!(
            refusal.subject.as_deref(),
            Some("missing_runtime_for_shard_device")
        );
        Ok(())
    }

    #[test]
    fn local_multi_device_execution_refusal_taxonomy_surfaces_missing_topology_and_pipeline_gap(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let device0 = sample_cuda_device(0, 16 * 1024 * 1024 * 1024);
        let device1 = sample_cuda_device(1, 16 * 1024 * 1024 * 1024);
        let contract = LocalShardingContract::new(
            "gguf-decoder-llama-tp-v1",
            "gguf_decoder:llama",
            "cuda",
            ShardedModelLayoutKind::TensorSharded,
            2,
            Some(2),
            Some(8 * 1024 * 1024 * 1024),
            vec![
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::TokenEmbedding,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
                LocalWeightShardingRule::new(
                    LocalModelWeightClass::AttentionOutput,
                    LocalWeightShardingStrategy::TensorAxis { axis: 1 },
                ),
            ],
        )?;
        let manifest = sample_tensor_manifest()?;
        let plan = sample_plan();
        let inputs = BTreeMap::new();
        let mut runtimes = vec![MockLocalShardRuntime::new(device0.clone())];

        let missing_topology_selection = BackendSelection::direct_with_policy(
            "cuda",
            None,
            vec![String::from("matmul")],
            ServedProductBackendPolicy::same_backend_only(),
        );
        let missing_topology = execute_local_multi_device_plan(
            runtimes.as_mut_slice(),
            LocalMultiDeviceExecutionRequest::new(
                &plan,
                &missing_topology_selection,
                &contract,
                &manifest,
                &inputs,
            ),
        )
        .expect_err("missing topology should refuse");
        assert_eq!(
            missing_topology.refusal_reason(),
            Some(LocalMultiDeviceRefusalReason::MissingExecutionTopology)
        );

        let pipeline_topology = crate::ExecutionTopologyPlan::pipeline_sharded(
            "cuda",
            vec![
                (device0.inventory_qualifiers(), 0, 16),
                (device1.inventory_qualifiers(), 16, 32),
            ],
        );
        let pipeline_selection = BackendSelection::direct_with_policy(
            "cuda",
            Some(device0),
            vec![String::from("matmul")],
            ServedProductBackendPolicy::same_backend_only(),
        )
        .with_selected_devices(vec![
            sample_cuda_device(0, 16 * 1024 * 1024 * 1024),
            device1,
        ])
        .with_execution_topology(Some(pipeline_topology));
        let pipeline_error = execute_local_multi_device_plan(
            runtimes.as_mut_slice(),
            LocalMultiDeviceExecutionRequest::new(
                &plan,
                &pipeline_selection,
                &contract,
                &manifest,
                &inputs,
            ),
        )
        .expect_err("pipeline topology should refuse");
        assert_eq!(
            pipeline_error.refusal_reason(),
            Some(LocalMultiDeviceRefusalReason::UnsupportedTopologyKind)
        );
        let refusal = pipeline_error
            .refusal()
            .expect("unsupported pipeline topology should map to refusal");
        assert_eq!(
            refusal.code,
            PsionicRefusalCode::UnsupportedBackendCapability
        );
        assert_eq!(refusal.scope, PsionicRefusalScope::Topology);
        assert_eq!(
            refusal.subject.as_deref(),
            Some("unsupported_topology_kind")
        );
        Ok(())
    }

    fn sample_plan() -> ExecutionPlan {
        let input = TensorId(1);
        let hidden = TensorId(2);
        let output = TensorId(3);
        let spec = TensorSpec::new(Shape::new(vec![1, 4]), DType::F32, Device::cpu());
        ExecutionPlan {
            graph_digest: String::from("graph-digest"),
            steps: vec![
                ExecutionStep {
                    output: hidden,
                    op: ExecutionOp::Matmul,
                    spec: spec.clone(),
                    inputs: vec![input],
                },
                ExecutionStep {
                    output,
                    op: ExecutionOp::Add,
                    spec,
                    inputs: vec![hidden],
                },
            ],
            outputs: vec![output],
        }
    }

    fn sample_tensor_manifest() -> Result<ShardedModelManifest, Box<dyn std::error::Error>> {
        let served_artifact = ServedArtifactIdentity::new(
            "fixture-word-decoder-v0",
            "v0",
            "bundle-digest",
            Some(String::from("model-blob-digest")),
            Some(String::from("tokenizer-digest")),
            Some(String::from("template-digest")),
            "defaults-digest",
            "gguf",
            QuantizationMode::GgmlQ4_0,
            BackendToolchainIdentity::new("cuda", "cuda@0.1.0", vec![]),
        );
        Ok(ShardedModelManifest::new(
            "tensor-manifest",
            served_artifact,
            ShardedModelLayoutKind::TensorSharded,
        )
        .with_shard(ShardedModelArtifactRef::new(
            0,
            "decoder.tensor0_32",
            "tensor-digest-0",
            ExecutionPartition::TensorRange {
                axis: 1,
                start: 0,
                end: 32,
            },
        ))
        .with_shard(ShardedModelArtifactRef::new(
            1,
            "decoder.tensor32_64",
            "tensor-digest-1",
            ExecutionPartition::TensorRange {
                axis: 1,
                start: 32,
                end: 64,
            },
        )))
    }

    fn sample_cuda_device(index: usize, memory_capacity_bytes: u64) -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(
                DeviceKind::Cuda,
                u16::try_from(index).expect("fixture index should fit in u16"),
                Some(format!("cuda:{index}")),
            ),
            device_name: Some(format!("CUDA Test Device {index}")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: vec![QuantizationSupport {
                mode: QuantizationMode::None,
                load_path: QuantizationLoadPath::DenseF32,
                execution: QuantizationExecution::Native,
            }],
            memory_capacity_bytes: Some(memory_capacity_bytes),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: None,
        }
    }
}
