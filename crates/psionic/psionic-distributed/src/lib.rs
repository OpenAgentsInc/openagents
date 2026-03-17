//! Public framework-distributed semantics above Psionic mesh truth.
//!
//! This crate intentionally lands the first bounded framework-distributed
//! slices: explicit group initialization from current mesh facts, honest
//! singleton fallback, global-group reuse, plan-backed subgroup split
//! semantics, a reference-first collective helper layer above that group
//! surface, tree-aware gradient reduction, bounded tensor-parallel and
//! FSDP-style framework helpers, and a bounded launch/config planning shell
//! that maps explicit hostfile-like input onto Psionic cluster, sandbox, and
//! mesh truth.
//! Explicit backend-family mapping for ring, mpi, nccl, and jaccl-class
//! requests now also exists above that substrate, while transport-backed
//! collective execution and broader helper families still land later.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};

use psionic_array::{Array, ArrayContext, ArrayError, Tree, TreeError, TreeSpec};
use psionic_cluster::{
    ClusterBackendReadinessStatus, ClusterMembershipRecord, ClusterMembershipStatus,
    ClusterNodeTelemetry, ClusterState,
};
use psionic_core::{DType, Shape, TensorData};
use psionic_nn::{LayerError, Linear, NnTensor};
use psionic_runtime::{
    ClusterCommitAuthorityEvidence, ClusterCommunicationClass, ClusterCommunicationEligibility,
    ClusterExecutionContext, ClusterExecutionDisposition, ClusterSelectedNode,
    ClusterTransportClass, TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind,
    TrainingDeviceMeshContext, TrainingElasticMembershipContext,
};
use psionic_sandbox::{
    ProviderSandboxEntrypointType, ProviderSandboxEnvironmentVar, ProviderSandboxExecutionClass,
    ProviderSandboxJobRequest, ProviderSandboxProfile, ProviderSandboxResourceRequest,
};
use psionic_train::{
    DistributedOptimizerContract, DistributedOptimizerGroupContract, OptimizerResidencyTransition,
    OptimizerResidencyTransitionReason, OptimizerStateResidency, TrainingCoreError,
    TrainingDistributedOptimizerKind, TrainingGradientBatch, TrainingOptimizerConfig,
    TrainingOptimizerError, TrainingOptimizerState, TrainingOptimizerStateShardKind,
    TrainingParameterGroupState, TrainingParameterShardKind, TrainingSchedulerBinding,
    TrainingSchedulerKind, TrainingShardPlacement, TrainingShardRange, TrainingTensorBuffer,
    apply_training_optimizer_step, scheduled_learning_rate,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "public framework-distributed groups, bounded collective, tensor-parallel, and FSDP-style helpers, and launch/config planning above Psionic runtime mesh truth";

/// Default threshold used to group small gradient leaves into one all-reduce payload.
pub const DEFAULT_GROUPED_ALL_REDUCE_THRESHOLD_BYTES: usize = 64 * 1024;

const RESERVED_DISTRIBUTED_ENV_KEYS: [&str; 11] = [
    "PSIONIC_DISTRIBUTED_RANK",
    "PSIONIC_DISTRIBUTED_LOCAL_RANK",
    "PSIONIC_DISTRIBUTED_WORLD_SIZE",
    "PSIONIC_DISTRIBUTED_NODE_ID",
    "PSIONIC_DISTRIBUTED_CLUSTER_ID",
    "PSIONIC_DISTRIBUTED_CLUSTER_STATE_DIGEST",
    "PSIONIC_DISTRIBUTED_TOPOLOGY_DIGEST",
    "PSIONIC_DISTRIBUTED_MESH_ID",
    "PSIONIC_DISTRIBUTED_MESH_REVISION",
    "PSIONIC_DISTRIBUTED_EFFECTIVE_BACKEND",
    "PSIONIC_DISTRIBUTED_GROUP_ID",
];

/// Requested distributed backend family for the public group surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedBackend {
    /// Use whatever current Psionic-owned distributed group has already been bootstrapped.
    #[default]
    Any,
    /// Request the MLX-style ring family once mapped onto Psionic topology profiles.
    Ring,
    /// Request the MLX-style MPI family once mapped onto Psionic topology profiles.
    Mpi,
    /// Request the MLX-style NCCL family once mapped onto Psionic topology profiles.
    Nccl,
    /// Request the MLX-style JACCL family once mapped onto Psionic topology profiles.
    Jaccl,
}

impl DistributedBackend {
    /// Returns the stable string label for this backend family.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Ring => "ring",
            Self::Mpi => "mpi",
            Self::Nccl => "nccl",
            Self::Jaccl => "jaccl",
        }
    }
}

impl fmt::Display for DistributedBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// High-level collective topology profile currently exposed by the public
/// distributed surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedCollectiveTopologyProfile {
    /// No peer process is involved; collective calls collapse to singleton behavior.
    SingletonLocal,
    /// Peer ranks communicate through loopback transport on one host.
    LoopbackMesh,
    /// Peer ranks communicate through trusted-LAN datagram transport.
    TrustedLanDatagramMesh,
    /// Peer ranks communicate through trusted-LAN stream transport.
    TrustedLanStreamMesh,
    /// Peer ranks communicate through wider-network authenticated streams.
    WiderNetworkStreamMesh,
    /// Peer ranks span multiple transport classes at once.
    MixedMesh,
}

impl DistributedCollectiveTopologyProfile {
    /// Returns the stable string label for this topology profile.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingletonLocal => "singleton_local",
            Self::LoopbackMesh => "loopback_mesh",
            Self::TrustedLanDatagramMesh => "trusted_lan_datagram_mesh",
            Self::TrustedLanStreamMesh => "trusted_lan_stream_mesh",
            Self::WiderNetworkStreamMesh => "wider_network_stream_mesh",
            Self::MixedMesh => "mixed_mesh",
        }
    }
}

impl fmt::Display for DistributedCollectiveTopologyProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Machine-readable backend-family capability for the current public distributed
/// topology.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedBackendCapability {
    /// Backend family originally requested by the caller.
    pub requested_backend: DistributedBackend,
    /// Backend family actually resolved for the current public topology.
    pub resolved_backend: DistributedBackend,
    /// Effective runtime backend carried by the underlying mesh.
    pub effective_backend: String,
    /// Communication class carried by the underlying mesh.
    pub communication_class: ClusterCommunicationClass,
    /// Transport class carried by the underlying mesh or launch plan.
    pub transport: ClusterTransportClass,
    /// High-level public topology profile derived from the current world.
    pub topology_profile: DistributedCollectiveTopologyProfile,
    /// Whether vendor transport-backed collectives are exposed publicly today.
    pub backend_transport_available: bool,
    /// Boundary note that keeps current mapping claims honest.
    pub detail: String,
}

/// Typed refusal returned when a named backend family does not match current
/// Psionic topology truth.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DistributedBackendMappingError {
    /// The requested backend family only maps onto tensor-collective meshes.
    #[error(
        "distributed backend `{backend}` requires communication class `tensor_collective_mesh`, but current topology uses `{actual_class:?}`"
    )]
    CommunicationClassMismatch {
        /// Requested backend family.
        backend: DistributedBackend,
        /// Actual communication class surfaced by the mesh.
        actual_class: ClusterCommunicationClass,
    },
    /// The requested backend family does not match the currently declared public
    /// transport profile.
    #[error(
        "distributed backend `{backend}` is incompatible with topology profile `{topology_profile}` over transport `{transport:?}`: {detail}"
    )]
    TopologyProfileMismatch {
        /// Requested backend family.
        backend: DistributedBackend,
        /// Topology profile derived from current mesh truth.
        topology_profile: DistributedCollectiveTopologyProfile,
        /// Transport class carried by the topology.
        transport: ClusterTransportClass,
        /// Human-readable mismatch detail.
        detail: String,
    },
    /// The requested backend family requires a different effective runtime
    /// backend.
    #[error(
        "distributed backend `{backend}` requires effective backend `{required_backend}`, but current topology uses `{effective_backend}`"
    )]
    EffectiveBackendMismatch {
        /// Requested backend family.
        backend: DistributedBackend,
        /// Required effective runtime backend label.
        required_backend: String,
        /// Current effective runtime backend label.
        effective_backend: String,
    },
    /// The requested backend family depends on a transport profile Psionic does
    /// not yet expose publicly.
    #[error(
        "distributed backend `{backend}` requires a Psionic topology profile that is not public yet: {detail}"
    )]
    TopologyProfileUnavailable {
        /// Requested backend family.
        backend: DistributedBackend,
        /// Missing public topology-profile detail.
        detail: String,
    },
}

/// One explicit member in a public distributed group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedGroupMember {
    /// Stable node identifier.
    pub node_id: String,
    /// Stable rank in the current group.
    pub rank: usize,
    /// Stable shard or replica slot carried over from the mesh substrate.
    pub shard_id: usize,
    /// Plain-language device label for this member.
    pub device_label: String,
}

impl DistributedGroupMember {
    /// Creates one explicit group member.
    #[must_use]
    pub fn new(
        node_id: impl Into<String>,
        rank: usize,
        shard_id: usize,
        device_label: impl Into<String>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            rank,
            shard_id,
            device_label: device_label.into(),
        }
    }
}

/// Explicit mesh bootstrap facts used to initialize one distributed group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedGroupBootstrap {
    /// Runtime-visible mesh posture that constrains the group.
    pub mesh: TrainingDeviceMeshContext,
    /// Runtime-visible transport class attributed to the current mesh.
    #[serde(default = "default_distributed_bootstrap_transport")]
    pub transport: ClusterTransportClass,
    /// Stable local node identifier inside the bootstrapped mesh.
    pub local_node_id: String,
    /// Ordered member ledger with explicit rank facts.
    pub members: Vec<DistributedGroupMember>,
}

impl DistributedGroupBootstrap {
    /// Creates one explicit mesh bootstrap payload.
    #[must_use]
    pub fn new(
        mesh: TrainingDeviceMeshContext,
        local_node_id: impl Into<String>,
        members: Vec<DistributedGroupMember>,
    ) -> Self {
        Self {
            mesh,
            transport: default_distributed_bootstrap_transport(),
            local_node_id: local_node_id.into(),
            members,
        }
    }

    /// Overrides the transport class attributed to the bootstrapped mesh.
    #[must_use]
    pub const fn with_transport(mut self, transport: ClusterTransportClass) -> Self {
        self.transport = transport;
        self
    }
}

/// Public configuration for initializing or reusing one distributed group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedInitOptions {
    /// Whether the call must refuse instead of returning a singleton fallback.
    pub strict: bool,
    /// Requested distributed backend family.
    pub backend: DistributedBackend,
    /// Optional explicit mesh bootstrap payload for the current process.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap: Option<DistributedGroupBootstrap>,
}

impl Default for DistributedInitOptions {
    fn default() -> Self {
        Self {
            strict: false,
            backend: DistributedBackend::Any,
            bootstrap: None,
        }
    }
}

impl DistributedInitOptions {
    /// Creates the default non-strict `backend = any` initialization request.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides the strictness posture.
    #[must_use]
    pub const fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    /// Overrides the requested backend family.
    #[must_use]
    pub const fn with_backend(mut self, backend: DistributedBackend) -> Self {
        self.backend = backend;
        self
    }

    /// Attaches an explicit mesh bootstrap payload.
    #[must_use]
    pub fn with_bootstrap(mut self, bootstrap: DistributedGroupBootstrap) -> Self {
        self.bootstrap = Some(bootstrap);
        self
    }
}

/// One explicit subgroup color/key assignment used to realize `Group.split(...)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedSplitAssignment {
    /// Stable node identifier in the parent group.
    pub node_id: String,
    /// MLX-style subgroup color for that member.
    pub color: i32,
    /// Optional rank-ordering key for the new subgroup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<i64>,
}

impl DistributedSplitAssignment {
    /// Creates one subgroup color assignment.
    #[must_use]
    pub fn new(node_id: impl Into<String>, color: i32) -> Self {
        Self {
            node_id: node_id.into(),
            color,
            key: None,
        }
    }

    /// Attaches an explicit subgroup ordering key.
    #[must_use]
    pub const fn with_key(mut self, key: i64) -> Self {
        self.key = Some(key);
        self
    }
}

/// Explicit per-member subgroup plan used by the current bounded split surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedSplitPlan {
    /// Explicit assignments for all members in the parent group.
    pub assignments: Vec<DistributedSplitAssignment>,
}

impl DistributedSplitPlan {
    /// Creates one explicit split plan.
    #[must_use]
    pub fn new(assignments: Vec<DistributedSplitAssignment>) -> Self {
        Self { assignments }
    }
}

/// High-level provenance for the current distributed group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedGroupKind {
    /// No bootstrapped global group was available, so the surface returned a singleton.
    SingletonFallback,
    /// The group came directly from explicit mesh/bootstrap truth.
    BootstrappedMesh,
    /// The group came from splitting a broader parent group through an explicit plan.
    SplitSubgroup,
}

/// Machine-readable snapshot of one public distributed group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedGroupSnapshot {
    /// Stable digest for this specific public group.
    pub group_id: String,
    /// High-level provenance for this group.
    pub kind: DistributedGroupKind,
    /// Requested distributed backend family for the group.
    pub requested_backend: DistributedBackend,
    /// Effective runtime backend carried by the underlying mesh.
    pub effective_backend: String,
    /// Communication class carried by the underlying mesh.
    pub communication_class: ClusterCommunicationClass,
    /// Transport class carried by the underlying mesh.
    pub transport: ClusterTransportClass,
    /// Backend-family capability mapped onto current topology truth.
    pub backend_capability: DistributedBackendCapability,
    /// Stable mesh identifier carried by the underlying runtime context.
    pub mesh_id: String,
    /// Monotonic mesh revision carried by the underlying runtime context.
    pub mesh_revision: u64,
    /// Stable local node identifier inside this group.
    pub local_node_id: String,
    /// Local rank inside this group.
    pub rank: usize,
    /// Total size of this group.
    pub size: usize,
    /// Ordered members that define the group rank layout.
    pub members: Vec<DistributedGroupMember>,
    /// Parent group identifier when this group came from one explicit split.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_group_id: Option<String>,
}

/// One public collective helper exposed above the distributed-group surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedCollectiveOperation {
    /// All-reduce sum over the current group.
    AllSum,
    /// First-axis gather over the current group.
    AllGather,
    /// Sum-only reduce-scatter over the current group.
    ReduceScatter,
    /// Point-to-point send to one destination rank.
    Send,
    /// Point-to-point receive from one source rank.
    Recv,
}

impl DistributedCollectiveOperation {
    /// Returns the stable label for the collective helper.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllSum => "all_sum",
            Self::AllGather => "all_gather",
            Self::ReduceScatter => "reduce_scatter",
            Self::Send => "send",
            Self::Recv => "recv",
        }
    }
}

impl fmt::Display for DistributedCollectiveOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Support posture for one public collective helper on the current group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedCollectiveSupportStatus {
    /// The helper is a real singleton passthrough on this group.
    SingletonPassthrough,
    /// The helper is available through explicit reference payloads above the group surface.
    ReferenceEmulation,
    /// The helper currently validates local behavior only while transport remains later work.
    ValidationOnly,
    /// The helper explicitly refuses on this group today.
    TypedRefusal,
}

/// Machine-readable collective capability snapshot for one public group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedCollectiveSupport {
    /// Stable group identifier this snapshot describes.
    pub group_id: String,
    /// High-level provenance for the current group.
    pub group_kind: DistributedGroupKind,
    /// Requested backend family for this group.
    pub requested_backend: DistributedBackend,
    /// Effective backend carried by the underlying mesh.
    pub effective_backend: String,
    /// Communication class carried by the underlying mesh.
    pub communication_class: ClusterCommunicationClass,
    /// Transport class carried by the underlying mesh.
    pub transport: ClusterTransportClass,
    /// Backend-family capability mapped onto current topology truth.
    pub backend_capability: DistributedBackendCapability,
    /// Whether vendor transport-backed collectives are available on the current public surface.
    pub backend_transport_available: bool,
    /// Current posture of `all_sum`.
    pub all_sum: DistributedCollectiveSupportStatus,
    /// Current posture of `all_gather`.
    pub all_gather: DistributedCollectiveSupportStatus,
    /// Current posture of `reduce_scatter`.
    pub reduce_scatter: DistributedCollectiveSupportStatus,
    /// Current posture of `send`.
    pub send: DistributedCollectiveSupportStatus,
    /// Current posture of `recv`.
    pub recv: DistributedCollectiveSupportStatus,
    /// Boundary note that keeps current claims honest.
    pub boundary_note: String,
}

/// Host-owned reference storage used by the bounded public collective layer.
#[derive(Clone, Debug, PartialEq)]
pub enum DistributedReferenceStorage {
    /// Floating-point payload used for logical `f32`, `f16`, and `bf16`.
    F32(Vec<f32>),
    /// Integer payload used for logical `i8`.
    I8(Vec<i8>),
}

/// Explicit host-owned tensor payload for bounded collective emulation.
#[derive(Clone, Debug, PartialEq)]
pub struct DistributedReferenceTensor {
    shape: Shape,
    dtype: DType,
    storage: DistributedReferenceStorage,
}

impl DistributedReferenceTensor {
    /// Creates one floating-point reference tensor with explicit logical dtype.
    pub fn float(
        shape: Shape,
        dtype: DType,
        values: impl Into<Vec<f32>>,
    ) -> Result<Self, DistributedCollectiveError> {
        if !matches!(dtype, DType::F32 | DType::F16 | DType::BF16) {
            return Err(DistributedCollectiveError::FloatReferenceDTypeMismatch { dtype });
        }
        let values = values.into();
        validate_reference_element_count(&shape, values.len())?;
        Ok(Self {
            shape,
            dtype,
            storage: DistributedReferenceStorage::F32(values),
        })
    }

    /// Creates one logical `f32` reference tensor.
    pub fn f32(
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<Self, DistributedCollectiveError> {
        Self::float(shape, DType::F32, values)
    }

    /// Creates one logical `i8` reference tensor.
    pub fn i8(
        shape: Shape,
        values: impl Into<Vec<i8>>,
    ) -> Result<Self, DistributedCollectiveError> {
        let values = values.into();
        validate_reference_element_count(&shape, values.len())?;
        Ok(Self {
            shape,
            dtype: DType::I8,
            storage: DistributedReferenceStorage::I8(values),
        })
    }

    /// Materializes one array into an explicit host-owned reference tensor.
    pub fn from_array(array: &Array) -> Result<Self, DistributedCollectiveError> {
        let shape = array.shape().clone();
        let dtype = array.dtype();
        let data = array.to_host_data()?;
        let storage = match dtype {
            DType::F32 | DType::F16 | DType::BF16 => DistributedReferenceStorage::F32(
                data.as_f32_slice()
                    .ok_or(DistributedCollectiveError::HostInteropStorageMismatch { dtype })?
                    .to_vec(),
            ),
            DType::I8 => DistributedReferenceStorage::I8(
                data.as_i8_slice()
                    .ok_or(DistributedCollectiveError::HostInteropStorageMismatch { dtype })?
                    .to_vec(),
            ),
        };
        Ok(Self {
            shape,
            dtype,
            storage,
        })
    }

    /// Returns the logical shape of this reference tensor.
    #[must_use]
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    /// Returns the logical dtype of this reference tensor.
    #[must_use]
    pub const fn dtype(&self) -> DType {
        self.dtype
    }

    /// Rebuilds one lazy array from the host-owned reference tensor.
    pub fn to_array(&self, context: &ArrayContext) -> Result<Array, DistributedCollectiveError> {
        match &self.storage {
            DistributedReferenceStorage::F32(values) => {
                let array = context.constant_f32(self.shape.clone(), values.clone())?;
                if self.dtype == DType::F32 {
                    Ok(array)
                } else {
                    Ok(array.cast(self.dtype)?)
                }
            }
            DistributedReferenceStorage::I8(values) => {
                let promoted = values
                    .iter()
                    .map(|value| f32::from(*value))
                    .collect::<Vec<_>>();
                Ok(context
                    .constant_f32(self.shape.clone(), promoted)?
                    .cast(DType::I8)?)
            }
        }
    }

    fn as_f32_values(&self) -> Vec<f32> {
        match &self.storage {
            DistributedReferenceStorage::F32(values) => values.clone(),
            DistributedReferenceStorage::I8(values) => {
                values.iter().map(|value| f32::from(*value)).collect()
            }
        }
    }

    fn as_i8_values(&self) -> Option<Vec<i8>> {
        match &self.storage {
            DistributedReferenceStorage::F32(_) => None,
            DistributedReferenceStorage::I8(values) => Some(values.clone()),
        }
    }
}

/// Public options for one collective helper over a distributed group.
#[derive(Clone, Debug, Default)]
pub struct DistributedCollectiveOptions {
    /// Explicit group to use; defaults to the current global group or a singleton fallback.
    pub group: Option<DistributedGroup>,
    /// Explicit per-rank reference inputs used to emulate multi-rank behavior.
    pub rank_inputs: BTreeMap<usize, DistributedReferenceTensor>,
}

impl DistributedCollectiveOptions {
    /// Creates one default options payload.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches an explicit group handle.
    #[must_use]
    pub fn with_group(mut self, group: DistributedGroup) -> Self {
        self.group = Some(group);
        self
    }

    /// Adds one rank payload to the explicit reference map.
    #[must_use]
    pub fn with_rank_input(mut self, rank: usize, payload: DistributedReferenceTensor) -> Self {
        self.rank_inputs.insert(rank, payload);
        self
    }

    /// Replaces the explicit reference map.
    #[must_use]
    pub fn with_rank_inputs(
        mut self,
        rank_inputs: BTreeMap<usize, DistributedReferenceTensor>,
    ) -> Self {
        self.rank_inputs = rank_inputs;
        self
    }
}

/// Public options for tree-aware gradient reduction over a distributed group.
#[derive(Clone, Debug)]
pub struct DistributedGradientReductionOptions {
    /// Explicit group override; defaults to the current reusable group or singleton fallback.
    pub group: Option<DistributedGroup>,
    /// Explicit host-owned tree payloads for every rank in the current group.
    pub rank_inputs: BTreeMap<usize, Tree<DistributedReferenceTensor>>,
    /// Maximum packed payload size used when grouping small leaves into one all-reduce call.
    pub small_tensor_bytes_threshold: usize,
}

impl Default for DistributedGradientReductionOptions {
    fn default() -> Self {
        Self {
            group: None,
            rank_inputs: BTreeMap::new(),
            small_tensor_bytes_threshold: DEFAULT_GROUPED_ALL_REDUCE_THRESHOLD_BYTES,
        }
    }
}

impl DistributedGradientReductionOptions {
    /// Creates the default tree-aware gradient-reduction options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides the distributed group used for the reduction.
    #[must_use]
    pub fn with_group(mut self, group: DistributedGroup) -> Self {
        self.group = Some(group);
        self
    }

    /// Adds one explicit rank tree payload.
    #[must_use]
    pub fn with_rank_input_tree(
        mut self,
        rank: usize,
        gradients: Tree<DistributedReferenceTensor>,
    ) -> Self {
        self.rank_inputs.insert(rank, gradients);
        self
    }

    /// Replaces the explicit rank tree payload map.
    #[must_use]
    pub fn with_rank_input_trees(
        mut self,
        rank_inputs: BTreeMap<usize, Tree<DistributedReferenceTensor>>,
    ) -> Self {
        self.rank_inputs = rank_inputs;
        self
    }

    /// Overrides the grouping threshold; `0` disables small-tensor packing.
    #[must_use]
    pub fn with_small_tensor_bytes_threshold(mut self, threshold: usize) -> Self {
        self.small_tensor_bytes_threshold = threshold;
        self
    }
}

/// Direction of tensor-parallel sharding for one linear helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TensorParallelLinearKind {
    /// Replicated input projected onto one output shard.
    AllToSharded,
    /// One input shard projected and reduced back to the full output space.
    ShardedToAll,
}

impl TensorParallelLinearKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllToSharded => "all_to_sharded",
            Self::ShardedToAll => "sharded_to_all",
        }
    }
}

impl fmt::Display for TensorParallelLinearKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Inspectable shard layout for one public tensor-parallel linear wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TensorParallelLinearLayout {
    /// Helper family this layout belongs to.
    pub helper: TensorParallelLinearKind,
    /// Group snapshot the wrapper was partitioned against.
    pub group: DistributedGroupSnapshot,
    /// Global input width before tensor parallel sharding.
    pub global_in_features: usize,
    /// Global output width before tensor parallel sharding.
    pub global_out_features: usize,
    /// Sharded axis for this helper family.
    pub shard_axis: usize,
    /// Inclusive shard start on the sharded axis.
    pub shard_start: usize,
    /// Exclusive shard end on the sharded axis.
    pub shard_end: usize,
}

impl TensorParallelLinearLayout {
    #[must_use]
    pub fn shard_width(&self) -> usize {
        self.shard_end.saturating_sub(self.shard_start)
    }

    #[must_use]
    pub fn local_in_features(&self) -> usize {
        match self.helper {
            TensorParallelLinearKind::AllToSharded => self.global_in_features,
            TensorParallelLinearKind::ShardedToAll => self.shard_width(),
        }
    }

    #[must_use]
    pub fn local_out_features(&self) -> usize {
        match self.helper {
            TensorParallelLinearKind::AllToSharded => self.shard_width(),
            TensorParallelLinearKind::ShardedToAll => self.global_out_features,
        }
    }
}

/// Failure for bounded tensor-parallel linear wrappers.
#[derive(Debug, Error, PartialEq)]
pub enum TensorParallelLinearError {
    /// Layer creation or execution refused on the bounded CPU-reference surface.
    #[error(transparent)]
    Layer(#[from] LayerError),
    /// Collective emulation refused while reconstructing one sharded-to-all output.
    #[error(transparent)]
    Collective(#[from] DistributedCollectiveError),
    /// Array construction or host export refused while bridging into collectives.
    #[error(transparent)]
    Array(#[from] ArrayError),
    /// The logical feature axis cannot be partitioned honestly over the current world.
    #[error(
        "public tensor-parallel `{helper}` cannot shard axis of size {axis_size} across world size {world_size}"
    )]
    FeatureAxisTooSmall {
        /// Helper family that refused.
        helper: TensorParallelLinearKind,
        /// Logical axis size to partition.
        axis_size: usize,
        /// Requested world size.
        world_size: usize,
    },
    /// The caller supplied a group that does not match the wrapper's stored layout.
    #[error(
        "public tensor-parallel `{helper}` expected group `{expected_group_id}` rank {expected_rank}/{expected_world_size}, got group `{actual_group_id}` rank {actual_rank}/{actual_world_size}"
    )]
    GroupMismatch {
        /// Helper family that refused.
        helper: TensorParallelLinearKind,
        /// Group identifier stored on the wrapper.
        expected_group_id: String,
        /// Local rank stored on the wrapper.
        expected_rank: usize,
        /// World size stored on the wrapper.
        expected_world_size: usize,
        /// Actual supplied group identifier.
        actual_group_id: String,
        /// Actual supplied local rank.
        actual_rank: usize,
        /// Actual supplied world size.
        actual_world_size: usize,
    },
    /// The bias-owner rank falls outside the world.
    #[error(
        "public tensor-parallel sharded_to_all bias owner rank {bias_owner_rank} is out of bounds for world size {world_size}"
    )]
    InvalidBiasOwnerRank {
        /// Rank selected to own the full bias vector.
        bias_owner_rank: usize,
        /// World size used by the wrapper.
        world_size: usize,
    },
    /// The wrapper needs all rank wrappers to emulate one multi-rank forward pass.
    #[error(
        "public tensor-parallel sharded_to_all forward requires rank wrappers for all group members; missing {missing_ranks:?}"
    )]
    MissingRankModules {
        /// Missing ranks in ascending order.
        missing_ranks: Vec<usize>,
    },
    /// The supplied rank-wrapper map referenced ranks outside the current group.
    #[error(
        "public tensor-parallel sharded_to_all rank-wrapper map contains invalid ranks {invalid_ranks:?} for group size {group_size}"
    )]
    InvalidRankModules {
        /// Invalid ranks in the supplied map.
        invalid_ranks: Vec<usize>,
        /// Group size used for validation.
        group_size: usize,
    },
    /// One supplied rank wrapper does not match the expected tensor-parallel layout.
    #[error(
        "public tensor-parallel sharded_to_all wrapper for rank {rank} does not match the current layout: {detail}"
    )]
    RankModuleMismatch {
        /// Rank whose wrapper disagreed.
        rank: usize,
        /// Plain-language mismatch detail.
        detail: String,
    },
    /// The wrapper needs all local shard inputs to emulate one multi-rank forward pass.
    #[error(
        "public tensor-parallel sharded_to_all forward requires shard inputs for all group members; missing {missing_ranks:?}"
    )]
    MissingRankInputs {
        /// Missing ranks in ascending order.
        missing_ranks: Vec<usize>,
    },
    /// The supplied rank-input map referenced ranks outside the current group.
    #[error(
        "public tensor-parallel sharded_to_all rank-input map contains invalid ranks {invalid_ranks:?} for group size {group_size}"
    )]
    InvalidRankInputs {
        /// Invalid ranks in the supplied map.
        invalid_ranks: Vec<usize>,
        /// Group size used for validation.
        group_size: usize,
    },
    /// One local or remote shard input disagreed with the expected prefix or local width.
    #[error(
        "public tensor-parallel sharded_to_all rank {rank} input shape mismatch: expected prefix {expected_prefix:?} with trailing width {expected_last_dim}, got {actual:?}"
    )]
    RankInputShapeMismatch {
        /// Rank whose input disagreed.
        rank: usize,
        /// Expected prefix dimensions shared across ranks.
        expected_prefix: Vec<usize>,
        /// Expected trailing local width for this rank.
        expected_last_dim: usize,
        /// Actual supplied dimensions.
        actual: Vec<usize>,
    },
    /// The explicit local-rank shard input did not match the supplied local input.
    #[error(
        "public tensor-parallel sharded_to_all local rank {rank} shard input does not match the supplied local input"
    )]
    LocalRankInputMismatch {
        /// Local rank whose explicit input disagreed.
        rank: usize,
    },
    /// Multi-rank sharded-to-all reconstruction needs explicit remote inputs and wrappers.
    #[error(
        "public tensor-parallel sharded_to_all forward for group `{group_id}` of size {world_size} requires explicit rank wrappers and shard inputs"
    )]
    MultiRankForwardRequiresExplicitInputs {
        /// Group identifier stored on the wrapper.
        group_id: String,
        /// World size stored on the wrapper.
        world_size: usize,
    },
    /// One full or sharded input tensor did not match the helper's expected shape contract.
    #[error(
        "public tensor-parallel `{helper}` input shape mismatch: expected {expected}, got {actual:?}"
    )]
    InputShapeMismatch {
        /// Helper family that refused.
        helper: TensorParallelLinearKind,
        /// Plain-language expected shape.
        expected: String,
        /// Actual supplied dimensions.
        actual: Vec<usize>,
    },
    /// The collective result could not be rebuilt into the bounded `NnTensor` surface.
    #[error(
        "public tensor-parallel sharded_to_all expected dense cpu f32 collective output, found dtype {dtype:?}"
    )]
    OutputDTypeMismatch {
        /// Dtype surfaced by the collective output.
        dtype: DType,
    },
}

/// Public options for bounded multi-rank `ShardedToAllLinear` emulation.
#[derive(Clone, Debug, Default)]
pub struct TensorParallelShardedToAllOptions {
    /// Explicit group override; defaults to the current reusable group or singleton fallback.
    pub group: Option<DistributedGroup>,
    /// Explicit wrappers for every rank in the current group.
    pub rank_modules: BTreeMap<usize, ShardedToAllLinear>,
    /// Explicit local shard inputs for every rank in the current group.
    pub rank_inputs: BTreeMap<usize, NnTensor>,
}

impl TensorParallelShardedToAllOptions {
    /// Creates the default sharded-to-all forward options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides the distributed group used to validate the reconstruction.
    #[must_use]
    pub fn with_group(mut self, group: DistributedGroup) -> Self {
        self.group = Some(group);
        self
    }

    /// Adds one explicit rank wrapper.
    #[must_use]
    pub fn with_rank_module(mut self, rank: usize, module: ShardedToAllLinear) -> Self {
        self.rank_modules.insert(rank, module);
        self
    }

    /// Replaces the explicit rank-wrapper map.
    #[must_use]
    pub fn with_rank_modules(mut self, rank_modules: BTreeMap<usize, ShardedToAllLinear>) -> Self {
        self.rank_modules = rank_modules;
        self
    }

    /// Adds one explicit shard input for one rank.
    #[must_use]
    pub fn with_rank_input(mut self, rank: usize, input: NnTensor) -> Self {
        self.rank_inputs.insert(rank, input);
        self
    }

    /// Replaces the explicit rank-input map.
    #[must_use]
    pub fn with_rank_inputs(mut self, rank_inputs: BTreeMap<usize, NnTensor>) -> Self {
        self.rank_inputs = rank_inputs;
        self
    }
}

/// Bounded MLX-style linear wrapper that keeps the input replicated and shards
/// the output width across ranks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AllToShardedLinear {
    layout: TensorParallelLinearLayout,
    local_linear: Linear,
}

impl AllToShardedLinear {
    /// Builds one local output shard from a full bounded `Linear`.
    pub fn from_linear(
        linear: &Linear,
        group: &DistributedGroup,
    ) -> Result<Self, TensorParallelLinearError> {
        let snapshot = group.snapshot();
        let layout = tensor_parallel_linear_layout(
            TensorParallelLinearKind::AllToSharded,
            snapshot,
            linear.in_features(),
            linear.out_features(),
        )?;
        let local_weight = slice_linear_rows(
            linear.weight_f32()?,
            linear.out_features(),
            linear.in_features(),
            layout.shard_start,
            layout.shard_end,
        );
        let local_bias = linear
            .bias_f32()?
            .map(|bias| bias[layout.shard_start..layout.shard_end].to_vec());
        Ok(Self {
            local_linear: Linear::from_f32_parts(
                format!(
                    "{}.rank{}.all_to_sharded",
                    linear.module().module_id,
                    layout.group.rank
                ),
                layout.local_in_features(),
                layout.local_out_features(),
                local_weight,
                local_bias,
            )?,
            layout,
        })
    }

    #[must_use]
    pub fn layout(&self) -> &TensorParallelLinearLayout {
        &self.layout
    }

    #[must_use]
    pub fn local_linear(&self) -> &Linear {
        &self.local_linear
    }

    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, TensorParallelLinearError> {
        Ok(self.local_linear.forward(input)?)
    }
}

/// Bounded MLX-style linear wrapper that keeps one local input shard and
/// reconstructs the full output with an `all_sum`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShardedToAllLinear {
    layout: TensorParallelLinearLayout,
    local_linear: Linear,
    bias_owner_rank: usize,
}

impl ShardedToAllLinear {
    /// Builds one local input shard from a full bounded `Linear`, storing the
    /// full bias only on rank `0` so the final `all_sum` adds it exactly once.
    pub fn from_linear(
        linear: &Linear,
        group: &DistributedGroup,
    ) -> Result<Self, TensorParallelLinearError> {
        Self::from_linear_with_bias_owner(linear, group, 0)
    }

    /// Builds one local input shard from a full bounded `Linear` while placing
    /// the full bias vector on one explicit owner rank.
    pub fn from_linear_with_bias_owner(
        linear: &Linear,
        group: &DistributedGroup,
        bias_owner_rank: usize,
    ) -> Result<Self, TensorParallelLinearError> {
        let snapshot = group.snapshot();
        if bias_owner_rank >= snapshot.size {
            return Err(TensorParallelLinearError::InvalidBiasOwnerRank {
                bias_owner_rank,
                world_size: snapshot.size,
            });
        }
        let layout = tensor_parallel_linear_layout(
            TensorParallelLinearKind::ShardedToAll,
            snapshot,
            linear.in_features(),
            linear.out_features(),
        )?;
        let local_weight = slice_linear_columns(
            linear.weight_f32()?,
            linear.out_features(),
            linear.in_features(),
            layout.shard_start,
            layout.shard_end,
        );
        let local_bias = if layout.group.rank == bias_owner_rank {
            linear.bias_f32()?.map(ToOwned::to_owned)
        } else {
            None
        };
        Ok(Self {
            local_linear: Linear::from_f32_parts(
                format!(
                    "{}.rank{}.sharded_to_all",
                    linear.module().module_id,
                    layout.group.rank
                ),
                layout.local_in_features(),
                layout.local_out_features(),
                local_weight,
                local_bias,
            )?,
            layout,
            bias_owner_rank,
        })
    }

    #[must_use]
    pub fn layout(&self) -> &TensorParallelLinearLayout {
        &self.layout
    }

    #[must_use]
    pub fn local_linear(&self) -> &Linear {
        &self.local_linear
    }

    #[must_use]
    pub const fn bias_owner_rank(&self) -> usize {
        self.bias_owner_rank
    }

    /// Splits one full input over this wrapper's local input-shard range.
    pub fn shard_input(&self, input: &NnTensor) -> Result<NnTensor, TensorParallelLinearError> {
        slice_last_dim(
            input,
            self.layout.shard_start,
            self.layout.shard_end,
            TensorParallelLinearKind::ShardedToAll,
            self.layout.global_in_features,
        )
    }

    /// Runs only the local partial projection for one sharded input.
    pub fn local_forward_partial(
        &self,
        input: &NnTensor,
    ) -> Result<NnTensor, TensorParallelLinearError> {
        Ok(self.local_linear.forward(input)?)
    }

    /// Runs the bounded forward path. Singleton wrappers run directly; multi-rank
    /// wrappers require `forward_with_options(...)`.
    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, TensorParallelLinearError> {
        if self.layout.group.size == 1 {
            return Ok(self.local_linear.forward(input)?);
        }
        let _ = self.local_linear.forward(input)?;
        Err(
            TensorParallelLinearError::MultiRankForwardRequiresExplicitInputs {
                group_id: self.layout.group.group_id.clone(),
                world_size: self.layout.group.size,
            },
        )
    }

    /// Reconstructs one full output with explicit per-rank wrappers and shard inputs.
    pub fn forward_with_options(
        &self,
        input: &NnTensor,
        options: TensorParallelShardedToAllOptions,
    ) -> Result<NnTensor, TensorParallelLinearError> {
        if self.layout.group.size == 1 {
            return Ok(self.local_linear.forward(input)?);
        }

        let group = resolve_collective_group(options.group)?;
        validate_tensor_parallel_group(
            &self.layout,
            &group.snapshot(),
            TensorParallelLinearKind::ShardedToAll,
        )?;
        validate_rank_modules(&self.layout, &options.rank_modules)?;

        let group_size = self.layout.group.size;
        let invalid_ranks = options
            .rank_inputs
            .keys()
            .copied()
            .filter(|rank| *rank >= group_size)
            .collect::<Vec<_>>();
        if !invalid_ranks.is_empty() {
            return Err(TensorParallelLinearError::InvalidRankInputs {
                invalid_ranks,
                group_size,
            });
        }
        let missing_ranks = (0..group_size)
            .filter(|rank| !options.rank_inputs.contains_key(rank))
            .collect::<Vec<_>>();
        if !missing_ranks.is_empty() {
            return Err(TensorParallelLinearError::MissingRankInputs { missing_ranks });
        }
        if options
            .rank_inputs
            .get(&self.layout.group.rank)
            .is_some_and(|rank_input| rank_input != input)
        {
            return Err(TensorParallelLinearError::LocalRankInputMismatch {
                rank: self.layout.group.rank,
            });
        }
        if input.dims().is_empty() {
            return Err(TensorParallelLinearError::InputShapeMismatch {
                helper: TensorParallelLinearKind::ShardedToAll,
                expected: format!(
                    "rank >= 1 with trailing dimension {}",
                    self.layout.local_in_features()
                ),
                actual: input.dims().to_vec(),
            });
        }

        let expected_prefix = input.dims()[..input.dims().len() - 1].to_vec();
        let array_context = ArrayContext::cpu();
        let mut local_partial = None;
        let mut partial_rank_inputs = BTreeMap::new();

        for rank in 0..group_size {
            let Some(module) = options.rank_modules.get(&rank) else {
                return Err(TensorParallelLinearError::MissingRankModules {
                    missing_ranks: vec![rank],
                });
            };
            let Some(rank_input) = options.rank_inputs.get(&rank) else {
                return Err(TensorParallelLinearError::MissingRankInputs {
                    missing_ranks: vec![rank],
                });
            };
            let expected_last_dim = module.layout.local_in_features();
            validate_sharded_rank_input(
                rank,
                rank_input,
                expected_prefix.as_slice(),
                expected_last_dim,
            )?;
            let partial = module.local_forward_partial(rank_input)?;
            if rank == self.layout.group.rank {
                local_partial = Some(partial.clone());
            }
            partial_rank_inputs.insert(rank, distributed_reference_tensor_from_nn(&partial)?);
        }

        let Some(local_partial) = local_partial else {
            return Err(TensorParallelLinearError::RankModuleMismatch {
                rank: self.layout.group.rank,
                detail: String::from("local rank wrapper did not participate in reconstruction"),
            });
        };
        let local_partial_array = array_from_nn_tensor(&array_context, &local_partial)?;
        let reduced = all_sum(
            &local_partial_array,
            DistributedCollectiveOptions::new()
                .with_group(group)
                .with_rank_inputs(partial_rank_inputs),
        )?;
        nn_tensor_from_array(&reduced)
    }
}

/// Public options for bounded MLX-style `fsdp_apply_gradients` emulation.
#[derive(Clone, Debug, Default)]
pub struct FsdpApplyGradientsOptions {
    /// Explicit distributed group override; defaults to the current reusable
    /// group or singleton fallback.
    pub group: Option<DistributedGroup>,
    /// Explicit remote-rank parameter-group state used to emulate remote shard
    /// optimizer updates. The local rank state comes from the mutable
    /// `parameter_groups` slice passed to `fsdp_apply_gradients`.
    pub remote_rank_group_states: BTreeMap<usize, Vec<TrainingParameterGroupState>>,
    /// Explicit remote-rank gradient batches used to emulate reduce-scatter and
    /// all-gather above the public collective layer.
    pub remote_rank_batches: BTreeMap<usize, TrainingGradientBatch>,
    /// Optional global-norm clip applied after mesh reduction and before local
    /// shard optimizer updates.
    pub clip_global_norm: Option<f32>,
}

impl FsdpApplyGradientsOptions {
    /// Creates the default FSDP helper options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides the distributed group used by the helper.
    #[must_use]
    pub fn with_group(mut self, group: DistributedGroup) -> Self {
        self.group = Some(group);
        self
    }

    /// Adds one remote-rank parameter-group state set.
    #[must_use]
    pub fn with_remote_rank_group_states(
        mut self,
        rank: usize,
        groups: Vec<TrainingParameterGroupState>,
    ) -> Self {
        self.remote_rank_group_states.insert(rank, groups);
        self
    }

    /// Replaces the full remote-rank parameter-group state map.
    #[must_use]
    pub fn with_remote_rank_group_state_map(
        mut self,
        remote_rank_group_states: BTreeMap<usize, Vec<TrainingParameterGroupState>>,
    ) -> Self {
        self.remote_rank_group_states = remote_rank_group_states;
        self
    }

    /// Adds one remote-rank gradient batch.
    #[must_use]
    pub fn with_remote_rank_batch(mut self, rank: usize, batch: TrainingGradientBatch) -> Self {
        self.remote_rank_batches.insert(rank, batch);
        self
    }

    /// Replaces the full remote-rank batch map.
    #[must_use]
    pub fn with_remote_rank_batches(
        mut self,
        remote_rank_batches: BTreeMap<usize, TrainingGradientBatch>,
    ) -> Self {
        self.remote_rank_batches = remote_rank_batches;
        self
    }

    /// Attaches one optional global-norm clip.
    #[must_use]
    pub fn with_clip_global_norm(mut self, clip_global_norm: f32) -> Self {
        self.clip_global_norm = Some(clip_global_norm);
        self
    }
}

/// One group-level summary emitted by `fsdp_apply_gradients`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FsdpGroupApplyReceipt {
    /// Stable parameter-group identifier.
    pub group_id: String,
    /// Parameter sharding posture used for this group.
    pub parameter_shard_kind: TrainingParameterShardKind,
    /// Optimizer-state sharding posture used for this group.
    pub optimizer_state_shard_kind: TrainingOptimizerStateShardKind,
    /// Local shard range updated on the current rank.
    pub local_shard_range: TrainingShardRange,
    /// L2 norm of the reduced full gradient before optional global clipping.
    pub reduced_full_gradient_norm_l2: f32,
    /// L2 norm of the local shard gradient after optional clipping.
    pub local_shard_gradient_norm_l2: f32,
    /// Effective learning rate applied to the shard update.
    pub effective_learning_rate: f32,
    /// Effective weight decay applied to the shard update.
    pub effective_weight_decay: f32,
    /// Scheduler family that contributed to the effective rate when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_kind: Option<TrainingSchedulerKind>,
    /// L2 norm of the local update vector.
    pub local_update_norm_l2: f32,
    /// L2 norm of the full gathered parameter tensor after the update.
    pub gathered_parameter_norm_l2_after: f32,
    /// Residency transitions emitted for the local shard owner.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub residency_transitions: Vec<OptimizerResidencyTransition>,
}

/// Full receipt emitted by `fsdp_apply_gradients`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FsdpApplyGradientsReceipt {
    /// Stable distributed group identifier used by the helper.
    pub distributed_group_id: String,
    /// Stable distributed-optimizer contract digest.
    pub contract_digest: String,
    /// Local rank that owned the current helper call.
    pub local_rank: usize,
    /// Total world size for the helper call.
    pub world_size: usize,
    /// Stable local batch identifier.
    pub batch_id: String,
    /// Optional global-norm clip requested by the caller.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_global_norm: Option<f32>,
    /// Global reduced-gradient norm before optional clipping.
    pub global_gradient_norm_l2: f32,
    /// Effective global clipping scale when one was applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_clipping_scale: Option<f32>,
    /// Group-level step receipts.
    pub groups: Vec<FsdpGroupApplyReceipt>,
    /// Stable digest over the receipt contents.
    pub receipt_digest: String,
}

/// Failure for the bounded public `fsdp_apply_gradients` helper.
#[derive(Debug, Error)]
pub enum FsdpApplyGradientsError {
    /// Public collectives refused while reducing or gathering one FSDP payload.
    #[error(transparent)]
    DistributedCollective(#[from] DistributedCollectiveError),
    /// Array materialization or host export refused while bridging into public collectives.
    #[error(transparent)]
    Array(#[from] ArrayError),
    /// Core tensor validation refused for one parameter or gradient buffer.
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    /// Optimizer math refused for one shard-local update.
    #[error(transparent)]
    TrainingOptimizer(#[from] TrainingOptimizerError),
    /// The helper only truthfully supports ZeRO stage 3 / FSDP-style contracts today.
    #[error("public fsdp_apply_gradients requires `zero_stage3`, found `{optimizer_kind:?}`")]
    UnsupportedOptimizerKind {
        /// Distributed optimizer family declared on the contract.
        optimizer_kind: TrainingDistributedOptimizerKind,
    },
    /// A requested global clip norm was zero or negative.
    #[error(
        "public fsdp_apply_gradients clip_global_norm must be greater than zero, found {clip_global_norm}"
    )]
    InvalidClipGlobalNorm {
        /// Invalid global clip norm.
        clip_global_norm: f32,
    },
    /// The local parameter-group slice does not include one contract group.
    #[error("public fsdp_apply_gradients local parameter groups are missing `{group_id}`")]
    MissingLocalGroup {
        /// Missing contract group identifier.
        group_id: String,
    },
    /// Remote-rank group state is missing for one or more ranks.
    #[error(
        "public fsdp_apply_gradients requires remote parameter-group state for ranks {missing_ranks:?}"
    )]
    MissingRemoteRankStates {
        /// Missing remote ranks.
        missing_ranks: Vec<usize>,
    },
    /// Remote-rank gradient batches are missing for one or more ranks.
    #[error("public fsdp_apply_gradients requires remote batches for ranks {missing_ranks:?}")]
    MissingRemoteRankBatches {
        /// Missing remote ranks.
        missing_ranks: Vec<usize>,
    },
    /// A remote-rank state or batch map referenced invalid ranks.
    #[error(
        "public fsdp_apply_gradients `{kind}` map contains invalid ranks {invalid_ranks:?} for world size {world_size}"
    )]
    InvalidRemoteRanks {
        /// Whether the invalid map was `state` or `batch`.
        kind: &'static str,
        /// Invalid rank ids.
        invalid_ranks: Vec<usize>,
        /// World size used for validation.
        world_size: usize,
    },
    /// A remote-rank state map duplicated one group identifier.
    #[error(
        "public fsdp_apply_gradients remote rank {rank} duplicated parameter group `{group_id}`"
    )]
    DuplicateRemoteGroup {
        /// Rank whose remote group list duplicated an identifier.
        rank: usize,
        /// Group identifier that was duplicated.
        group_id: String,
    },
    /// One remote-rank group set is missing a contract group.
    #[error(
        "public fsdp_apply_gradients remote rank {rank} is missing parameter group `{group_id}`"
    )]
    MissingRemoteGroup {
        /// Rank whose remote group list omitted the group.
        rank: usize,
        /// Missing group identifier.
        group_id: String,
    },
    /// One remote-rank batch omitted a required gradient.
    #[error(
        "public fsdp_apply_gradients remote rank {rank} batch `{batch_id}` is missing gradient for group `{group_id}`"
    )]
    MissingRemoteGradient {
        /// Rank whose batch omitted the gradient.
        rank: usize,
        /// Batch identifier on that rank.
        batch_id: String,
        /// Group identifier omitted by the batch.
        group_id: String,
    },
    /// A remote rank drifted from the local optimizer-facing contract.
    #[error(
        "public fsdp_apply_gradients remote rank {rank} group `{group_id}` does not match the local state: {detail}"
    )]
    RemoteStateMismatch {
        /// Rank whose state drifted.
        rank: usize,
        /// Group identifier that mismatched.
        group_id: String,
        /// Plain-language mismatch detail.
        detail: String,
    },
    /// The contract group layout is outside the first truthful helper scope.
    #[error(
        "public fsdp_apply_gradients group `{group_id}` requires replicated or full-shard parameters plus matching gradient/optimizer-state layouts, found parameter={parameter_kind:?}, gradient={gradient_kind:?}, optimizer_state={optimizer_state_kind:?}"
    )]
    UnsupportedGroupLayout {
        /// Contract group identifier.
        group_id: String,
        /// Parameter sharding posture.
        parameter_kind: TrainingParameterShardKind,
        /// Gradient sharding posture.
        gradient_kind: TrainingParameterShardKind,
        /// Optimizer-state sharding posture.
        optimizer_state_kind: TrainingOptimizerStateShardKind,
    },
    /// One full-shard group did not expose one equal-size shard per rank.
    #[error(
        "public fsdp_apply_gradients group `{group_id}` requires exactly one contiguous equal-size shard per rank"
    )]
    UnevenFullShardLayout {
        /// Contract group identifier.
        group_id: String,
    },
}

/// Applies one bounded MLX-style FSDP-class update above the typed distributed
/// optimizer contract.
pub fn fsdp_apply_gradients(
    parameter_groups: &mut [TrainingParameterGroupState],
    local_batch: &TrainingGradientBatch,
    contract: &DistributedOptimizerContract,
    options: FsdpApplyGradientsOptions,
) -> Result<FsdpApplyGradientsReceipt, FsdpApplyGradientsError> {
    if contract.optimizer_kind != TrainingDistributedOptimizerKind::ZeroStage3 {
        return Err(FsdpApplyGradientsError::UnsupportedOptimizerKind {
            optimizer_kind: contract.optimizer_kind,
        });
    }
    if let Some(clip_global_norm) = options.clip_global_norm
        && clip_global_norm <= 0.0
    {
        return Err(FsdpApplyGradientsError::InvalidClipGlobalNorm { clip_global_norm });
    }

    let group = resolve_collective_group(options.group)?;
    let local_rank = group.rank();
    let world_size = group.size();
    validate_remote_rank_map_keys(
        local_rank,
        world_size,
        options
            .remote_rank_group_states
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        "state",
    )?;
    validate_remote_rank_map_keys(
        local_rank,
        world_size,
        options
            .remote_rank_batches
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        "batch",
    )?;
    if world_size > 1 {
        let missing_states = (0..world_size)
            .filter(|rank| {
                *rank != local_rank && !options.remote_rank_group_states.contains_key(rank)
            })
            .collect::<Vec<_>>();
        if !missing_states.is_empty() {
            return Err(FsdpApplyGradientsError::MissingRemoteRankStates {
                missing_ranks: missing_states,
            });
        }
        let missing_batches = (0..world_size)
            .filter(|rank| *rank != local_rank && !options.remote_rank_batches.contains_key(rank))
            .collect::<Vec<_>>();
        if !missing_batches.is_empty() {
            return Err(FsdpApplyGradientsError::MissingRemoteRankBatches {
                missing_ranks: missing_batches,
            });
        }
    }

    let local_group_indices = local_group_indices(parameter_groups);
    let remote_rank_group_maps = options
        .remote_rank_group_states
        .iter()
        .map(|(rank, groups)| Ok((*rank, remote_group_map(*rank, groups.clone())?)))
        .collect::<Result<BTreeMap<_, _>, FsdpApplyGradientsError>>()?;

    let group_plans = contract
        .groups
        .iter()
        .map(|group_contract| {
            let Some(local_index) = local_group_indices.get(group_contract.group_id.as_str())
            else {
                return Err(FsdpApplyGradientsError::MissingLocalGroup {
                    group_id: group_contract.group_id.clone(),
                });
            };
            let local_group = &parameter_groups[*local_index];
            validate_remote_group_state_contract(
                group_contract.group_id.as_str(),
                local_group,
                &remote_rank_group_maps,
                world_size,
            )?;
            Ok((
                group_contract.group_id.clone(),
                build_fsdp_group_plan(
                    group_contract,
                    local_group.parameter.spec.storage_size(),
                    world_size,
                )?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, FsdpApplyGradientsError>>()?;

    let reduced_full_gradients = contract
        .groups
        .iter()
        .map(|group_contract| {
            let Some(local_gradient) = local_batch.gradients.get(group_contract.group_id.as_str())
            else {
                return Err(TrainingCoreError::MissingGradient {
                    batch_id: local_batch.batch_id.clone(),
                    group_id: group_contract.group_id.clone(),
                }
                .into());
            };
            let Some(local_group_index) = local_group_indices.get(group_contract.group_id.as_str())
            else {
                return Err(FsdpApplyGradientsError::MissingLocalGroup {
                    group_id: group_contract.group_id.clone(),
                });
            };
            let local_group = &parameter_groups[*local_group_index];
            validate_training_buffer_compatibility(
                local_batch.batch_id.as_str(),
                group_contract.group_id.as_str(),
                &local_group.parameter.spec,
                &local_gradient.spec,
            )?;
            let mut reduced =
                training_buffer_values(group_contract.group_id.as_str(), local_gradient)?.to_vec();
            for (rank, batch) in &options.remote_rank_batches {
                let Some(remote_gradient) = batch.gradients.get(group_contract.group_id.as_str())
                else {
                    return Err(FsdpApplyGradientsError::MissingRemoteGradient {
                        rank: *rank,
                        batch_id: batch.batch_id.clone(),
                        group_id: group_contract.group_id.clone(),
                    });
                };
                validate_training_buffer_compatibility(
                    batch.batch_id.as_str(),
                    group_contract.group_id.as_str(),
                    &local_group.parameter.spec,
                    &remote_gradient.spec,
                )?;
                for (destination, value) in reduced.iter_mut().zip(
                    training_buffer_values(group_contract.group_id.as_str(), remote_gradient)?
                        .iter(),
                ) {
                    *destination += value;
                }
            }
            Ok((group_contract.group_id.clone(), reduced))
        })
        .collect::<Result<BTreeMap<_, _>, FsdpApplyGradientsError>>()?;

    let global_gradient_norm_l2 = reduced_full_gradients
        .values()
        .flat_map(|values| values.iter())
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    let global_clipping_scale = match options.clip_global_norm {
        Some(clip_norm) if global_gradient_norm_l2 > clip_norm && global_gradient_norm_l2 > 0.0 => {
            Some(clip_norm / global_gradient_norm_l2)
        }
        Some(_) => Some(1.0),
        None => None,
    };

    let array_context = ArrayContext::cpu();
    let mut group_receipts = Vec::new();
    for group_contract in &contract.groups {
        let local_group_index = *local_group_indices
            .get(group_contract.group_id.as_str())
            .ok_or_else(|| FsdpApplyGradientsError::MissingLocalGroup {
                group_id: group_contract.group_id.clone(),
            })?;
        let plan = group_plans
            .get(group_contract.group_id.as_str())
            .ok_or_else(|| FsdpApplyGradientsError::MissingLocalGroup {
                group_id: group_contract.group_id.clone(),
            })?;
        let reduced_full = reduced_full_gradients
            .get(group_contract.group_id.as_str())
            .ok_or_else(|| FsdpApplyGradientsError::MissingLocalGroup {
                group_id: group_contract.group_id.clone(),
            })?;
        let reduced_full_gradient_norm_l2 = l2_norm(reduced_full.as_slice());

        match &plan.kind {
            FsdpGroupExecutionKind::Replicated => {
                let local_gradient_values = training_buffer_values(
                    group_contract.group_id.as_str(),
                    local_batch
                        .gradients
                        .get(group_contract.group_id.as_str())
                        .ok_or_else(|| TrainingCoreError::MissingGradient {
                            batch_id: local_batch.batch_id.clone(),
                            group_id: group_contract.group_id.clone(),
                        })?,
                )?
                .to_vec();
                let mut rank_inputs = BTreeMap::from([(
                    local_rank,
                    DistributedReferenceTensor::f32(
                        Shape::new(vec![local_gradient_values.len()]),
                        local_gradient_values.clone(),
                    )?,
                )]);
                for (rank, batch) in &options.remote_rank_batches {
                    let remote_gradient = batch
                        .gradients
                        .get(group_contract.group_id.as_str())
                        .ok_or_else(|| FsdpApplyGradientsError::MissingRemoteGradient {
                            rank: *rank,
                            batch_id: batch.batch_id.clone(),
                            group_id: group_contract.group_id.clone(),
                        })?;
                    rank_inputs.insert(
                        *rank,
                        DistributedReferenceTensor::f32(
                            Shape::new(vec![local_gradient_values.len()]),
                            training_buffer_values(
                                group_contract.group_id.as_str(),
                                remote_gradient,
                            )?
                            .to_vec(),
                        )?,
                    );
                }
                let local_gradient_array = array_context.constant_f32(
                    Shape::new(vec![local_gradient_values.len()]),
                    local_gradient_values,
                )?;
                let reduced = all_sum(
                    &local_gradient_array,
                    DistributedCollectiveOptions::new()
                        .with_group(group.clone())
                        .with_rank_inputs(rank_inputs),
                )?;
                let clipped_full = apply_optional_global_clip(
                    array_values(&reduced)?.as_slice(),
                    global_clipping_scale,
                );
                let update = apply_fsdp_local_update(
                    &mut parameter_groups[local_group_index],
                    &TrainingShardRange::new(0, clipped_full.len()),
                    clipped_full.as_slice(),
                )?;
                group_receipts.push(FsdpGroupApplyReceipt {
                    group_id: group_contract.group_id.clone(),
                    parameter_shard_kind: group_contract.parameter_layout.kind,
                    optimizer_state_shard_kind: group_contract.optimizer_state_layout.kind,
                    local_shard_range: TrainingShardRange::new(0, clipped_full.len()),
                    reduced_full_gradient_norm_l2,
                    local_shard_gradient_norm_l2: update.clipped_gradient_norm_l2,
                    effective_learning_rate: update.effective_learning_rate,
                    effective_weight_decay: update.effective_weight_decay,
                    scheduler_kind: update.scheduler_kind,
                    local_update_norm_l2: update.update_norm_l2,
                    gathered_parameter_norm_l2_after: l2_norm(training_buffer_values(
                        group_contract.group_id.as_str(),
                        &parameter_groups[local_group_index].parameter,
                    )?),
                    residency_transitions: update.transitions,
                });
            }
            FsdpGroupExecutionKind::FullShard { shard_ranges } => {
                let local_full_gradient = local_batch
                    .gradients
                    .get(group_contract.group_id.as_str())
                    .ok_or_else(|| TrainingCoreError::MissingGradient {
                        batch_id: local_batch.batch_id.clone(),
                        group_id: group_contract.group_id.clone(),
                    })?;
                let local_gradient_values =
                    training_buffer_values(group_contract.group_id.as_str(), local_full_gradient)?
                        .to_vec();
                let local_gradient_array = array_context.constant_f32(
                    Shape::new(vec![local_gradient_values.len()]),
                    local_gradient_values.clone(),
                )?;
                let mut rank_inputs = BTreeMap::from([(
                    local_rank,
                    DistributedReferenceTensor::f32(
                        Shape::new(vec![local_gradient_values.len()]),
                        local_gradient_values,
                    )?,
                )]);
                for (rank, batch) in &options.remote_rank_batches {
                    let remote_gradient = batch
                        .gradients
                        .get(group_contract.group_id.as_str())
                        .ok_or_else(|| FsdpApplyGradientsError::MissingRemoteGradient {
                            rank: *rank,
                            batch_id: batch.batch_id.clone(),
                            group_id: group_contract.group_id.clone(),
                        })?;
                    rank_inputs.insert(
                        *rank,
                        DistributedReferenceTensor::f32(
                            Shape::new(vec![reduced_full.len()]),
                            training_buffer_values(
                                group_contract.group_id.as_str(),
                                remote_gradient,
                            )?
                            .to_vec(),
                        )?,
                    );
                }
                let _local_scattered = reduce_scatter(
                    &local_gradient_array,
                    DistributedCollectiveOptions::new()
                        .with_group(group.clone())
                        .with_rank_inputs(rank_inputs),
                )?;
                let mut gathered_rank_shards = BTreeMap::new();
                let mut local_update = None;
                for rank in 0..world_size {
                    let shard_range = shard_ranges.get(rank).ok_or_else(|| {
                        FsdpApplyGradientsError::UnevenFullShardLayout {
                            group_id: group_contract.group_id.clone(),
                        }
                    })?;
                    let shard_gradient = apply_optional_global_clip(
                        &reduced_full[shard_range.offset_elements
                            ..shard_range.offset_elements + shard_range.element_count],
                        global_clipping_scale,
                    );
                    if rank == local_rank {
                        let update = apply_fsdp_local_update(
                            &mut parameter_groups[local_group_index],
                            shard_range,
                            shard_gradient.as_slice(),
                        )?;
                        gathered_rank_shards.insert(
                            rank,
                            DistributedReferenceTensor::f32(
                                Shape::new(vec![update.updated_shard_values.len()]),
                                update.updated_shard_values.clone(),
                            )?,
                        );
                        local_update = Some(update);
                    } else {
                        let remote_group = remote_rank_group_maps
                            .get(&rank)
                            .and_then(|groups| groups.get(group_contract.group_id.as_str()))
                            .ok_or_else(|| FsdpApplyGradientsError::MissingRemoteGroup {
                                rank,
                                group_id: group_contract.group_id.clone(),
                            })?;
                        let mut remote_group = remote_group.clone();
                        let remote_update = apply_fsdp_local_update(
                            &mut remote_group,
                            shard_range,
                            shard_gradient.as_slice(),
                        )?;
                        gathered_rank_shards.insert(
                            rank,
                            DistributedReferenceTensor::f32(
                                Shape::new(vec![remote_update.updated_shard_values.len()]),
                                remote_update.updated_shard_values,
                            )?,
                        );
                    }
                }
                let local_update = local_update.ok_or_else(|| {
                    FsdpApplyGradientsError::MissingRemoteRankStates {
                        missing_ranks: vec![local_rank],
                    }
                })?;
                let local_shard_array = array_context.constant_f32(
                    Shape::new(vec![local_update.updated_shard_values.len()]),
                    local_update.updated_shard_values.clone(),
                )?;
                let gathered = all_gather(
                    &local_shard_array,
                    DistributedCollectiveOptions::new()
                        .with_group(group.clone())
                        .with_rank_inputs(gathered_rank_shards),
                )?;
                let gathered_values = array_values(&gathered)?;
                assign_training_buffer_values(
                    group_contract.group_id.as_str(),
                    &mut parameter_groups[local_group_index].parameter,
                    gathered_values.as_slice(),
                )?;
                group_receipts.push(FsdpGroupApplyReceipt {
                    group_id: group_contract.group_id.clone(),
                    parameter_shard_kind: group_contract.parameter_layout.kind,
                    optimizer_state_shard_kind: group_contract.optimizer_state_layout.kind,
                    local_shard_range: shard_ranges[local_rank].clone(),
                    reduced_full_gradient_norm_l2,
                    local_shard_gradient_norm_l2: local_update.clipped_gradient_norm_l2,
                    effective_learning_rate: local_update.effective_learning_rate,
                    effective_weight_decay: local_update.effective_weight_decay,
                    scheduler_kind: local_update.scheduler_kind,
                    local_update_norm_l2: local_update.update_norm_l2,
                    gathered_parameter_norm_l2_after: l2_norm(gathered_values.as_slice()),
                    residency_transitions: local_update.transitions,
                });
            }
        }
    }

    let mut receipt = FsdpApplyGradientsReceipt {
        distributed_group_id: group.group_id().to_string(),
        contract_digest: contract.contract_digest.clone(),
        local_rank,
        world_size,
        batch_id: local_batch.batch_id.clone(),
        clip_global_norm: options.clip_global_norm,
        global_gradient_norm_l2,
        global_clipping_scale,
        groups: group_receipts,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_fsdp_apply_gradients_receipt_digest(&receipt);
    Ok(receipt)
}

/// Public options for one point-to-point helper over a distributed group.
#[derive(Clone, Debug, Default)]
pub struct DistributedPointToPointOptions {
    /// Explicit group to use; defaults to the current global group or a singleton fallback.
    pub group: Option<DistributedGroup>,
    /// Explicit per-peer message payloads used for bounded recv/send validation.
    pub message_payloads: BTreeMap<usize, DistributedReferenceTensor>,
}

impl DistributedPointToPointOptions {
    /// Creates one default options payload.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches an explicit group handle.
    #[must_use]
    pub fn with_group(mut self, group: DistributedGroup) -> Self {
        self.group = Some(group);
        self
    }

    /// Adds one message payload keyed by source or destination rank.
    #[must_use]
    pub fn with_message_payload(
        mut self,
        rank: usize,
        payload: DistributedReferenceTensor,
    ) -> Self {
        self.message_payloads.insert(rank, payload);
        self
    }

    /// Replaces the explicit point-to-point message map.
    #[must_use]
    pub fn with_message_payloads(
        mut self,
        message_payloads: BTreeMap<usize, DistributedReferenceTensor>,
    ) -> Self {
        self.message_payloads = message_payloads;
        self
    }
}

/// One hostfile-like entry describing one framework-visible rank target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedHostfileEntry {
    /// Stable node identifier that must already exist in authoritative cluster state.
    pub node_id: String,
    /// Number of local ranks requested on that node.
    pub slots: usize,
    /// Optional control-plane address declared by the hostfile for validation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advertised_addr: Option<String>,
}

impl DistributedHostfileEntry {
    /// Creates one hostfile-like entry with one slot by default.
    #[must_use]
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            slots: 1,
            advertised_addr: None,
        }
    }

    /// Overrides the number of requested local ranks on the node.
    #[must_use]
    pub fn with_slots(mut self, slots: usize) -> Self {
        self.slots = slots.max(1);
        self
    }

    /// Attaches an explicit control-plane address for validation against cluster truth.
    #[must_use]
    pub fn with_advertised_addr(mut self, advertised_addr: impl Into<String>) -> Self {
        self.advertised_addr = Some(advertised_addr.into());
        self
    }
}

/// Public launch/config input for the bounded framework-distributed planning shell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedLaunchConfig {
    /// Stable launch identifier for the planned job family.
    pub launch_id: String,
    /// Node that is planning or admitting the launch against authoritative cluster truth.
    pub scheduler_node_id: String,
    /// Requested MLX-style distributed backend family.
    pub requested_backend: DistributedBackend,
    /// Effective Psionic runtime backend the selected nodes must be ready to run.
    pub effective_backend: String,
    /// Sandbox execution class used for every planned rank.
    pub execution_class: ProviderSandboxExecutionClass,
    /// Entrypoint interpretation for the sandbox jobs.
    pub entrypoint_type: ProviderSandboxEntrypointType,
    /// Entrypoint string carried into each sandbox request.
    pub entrypoint: String,
    /// Optional inline payload carried into each sandbox request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    /// Argument vector passed to each sandbox request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<String>,
    /// Workspace root for each sandbox request.
    pub workspace_root: PathBuf,
    /// Expected output paths relative to the workspace root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_outputs: Vec<String>,
    /// Requested timeout for each sandbox request.
    pub timeout_request_s: u64,
    /// Requested network posture for each sandbox request.
    pub network_request: String,
    /// Requested filesystem posture for each sandbox request.
    pub filesystem_request: String,
    /// Base environment variables applied to each sandbox request before distributed keys.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment: Vec<ProviderSandboxEnvironmentVar>,
    /// Requested resource envelope for each sandbox request.
    #[serde(default)]
    pub resource_request: ProviderSandboxResourceRequest,
    /// Optional payout reference preserved on each sandbox request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout_reference: Option<String>,
    /// Optional verification posture preserved on each sandbox request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_posture: Option<String>,
    /// Provider identifier preserved on each sandbox request.
    pub provider_id: String,
    /// Optional compute product identifier override; defaults to the execution-class product.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_product_id: Option<String>,
    /// Explicit hostfile-like launch targets in rank order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hostfile_entries: Vec<DistributedHostfileEntry>,
    /// Explicit transport class attributed to the launch plan.
    pub transport: ClusterTransportClass,
    /// Optional explicit mesh axes; defaults to one data-parallel axis over world size.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<TrainingDeviceMeshAxis>,
}

impl DistributedLaunchConfig {
    /// Creates one bounded launch/config request with conservative defaults.
    #[must_use]
    pub fn new(
        launch_id: impl Into<String>,
        scheduler_node_id: impl Into<String>,
        effective_backend: impl Into<String>,
        entrypoint: impl Into<String>,
        workspace_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            launch_id: launch_id.into(),
            scheduler_node_id: scheduler_node_id.into(),
            requested_backend: DistributedBackend::Any,
            effective_backend: effective_backend.into(),
            execution_class: ProviderSandboxExecutionClass::PosixExec,
            entrypoint_type: ProviderSandboxEntrypointType::WorkspaceFile,
            entrypoint: entrypoint.into(),
            payload: None,
            arguments: Vec::new(),
            workspace_root: workspace_root.into(),
            expected_outputs: Vec::new(),
            timeout_request_s: 60,
            network_request: String::from("disabled"),
            filesystem_request: String::from("workspace_only"),
            environment: Vec::new(),
            resource_request: ProviderSandboxResourceRequest::default(),
            payout_reference: None,
            verification_posture: None,
            provider_id: String::from("psionic-distributed"),
            compute_product_id: None,
            hostfile_entries: Vec::new(),
            transport: ClusterTransportClass::TrustedLanStream,
            axes: Vec::new(),
        }
    }

    /// Overrides the requested distributed backend family.
    #[must_use]
    pub const fn with_requested_backend(mut self, requested_backend: DistributedBackend) -> Self {
        self.requested_backend = requested_backend;
        self
    }

    /// Overrides the sandbox execution class.
    #[must_use]
    pub const fn with_execution_class(
        mut self,
        execution_class: ProviderSandboxExecutionClass,
    ) -> Self {
        self.execution_class = execution_class;
        self
    }

    /// Overrides the entrypoint interpretation.
    #[must_use]
    pub const fn with_entrypoint_type(
        mut self,
        entrypoint_type: ProviderSandboxEntrypointType,
    ) -> Self {
        self.entrypoint_type = entrypoint_type;
        self
    }

    /// Attaches an inline payload for every sandbox request.
    #[must_use]
    pub fn with_payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(payload.into());
        self
    }

    /// Replaces the argument vector.
    #[must_use]
    pub fn with_arguments(mut self, arguments: Vec<String>) -> Self {
        self.arguments = arguments;
        self
    }

    /// Replaces the expected-output set.
    #[must_use]
    pub fn with_expected_outputs(mut self, expected_outputs: Vec<String>) -> Self {
        self.expected_outputs = expected_outputs;
        self
    }

    /// Overrides the timeout request for each sandbox job.
    #[must_use]
    pub fn with_timeout_request_s(mut self, timeout_request_s: u64) -> Self {
        self.timeout_request_s = timeout_request_s.max(1);
        self
    }

    /// Overrides the network request.
    #[must_use]
    pub fn with_network_request(mut self, network_request: impl Into<String>) -> Self {
        self.network_request = network_request.into();
        self
    }

    /// Overrides the filesystem request.
    #[must_use]
    pub fn with_filesystem_request(mut self, filesystem_request: impl Into<String>) -> Self {
        self.filesystem_request = filesystem_request.into();
        self
    }

    /// Replaces the base environment set.
    #[must_use]
    pub fn with_environment(mut self, environment: Vec<ProviderSandboxEnvironmentVar>) -> Self {
        self.environment = environment;
        self
    }

    /// Replaces the resource envelope.
    #[must_use]
    pub fn with_resource_request(
        mut self,
        resource_request: ProviderSandboxResourceRequest,
    ) -> Self {
        self.resource_request = resource_request;
        self
    }

    /// Attaches a payout reference.
    #[must_use]
    pub fn with_payout_reference(mut self, payout_reference: impl Into<String>) -> Self {
        self.payout_reference = Some(payout_reference.into());
        self
    }

    /// Attaches a verification posture.
    #[must_use]
    pub fn with_verification_posture(mut self, verification_posture: impl Into<String>) -> Self {
        self.verification_posture = Some(verification_posture.into());
        self
    }

    /// Overrides the provider identifier used in sandbox requests.
    #[must_use]
    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = provider_id.into();
        self
    }

    /// Overrides the compute-product identifier used in sandbox requests.
    #[must_use]
    pub fn with_compute_product_id(mut self, compute_product_id: impl Into<String>) -> Self {
        self.compute_product_id = Some(compute_product_id.into());
        self
    }

    /// Replaces the hostfile-like launch targets.
    #[must_use]
    pub fn with_hostfile_entries(
        mut self,
        hostfile_entries: Vec<DistributedHostfileEntry>,
    ) -> Self {
        self.hostfile_entries = hostfile_entries;
        self
    }

    /// Overrides the transport class attributed to the launch plan.
    #[must_use]
    pub const fn with_transport(mut self, transport: ClusterTransportClass) -> Self {
        self.transport = transport;
        self
    }

    /// Replaces the explicit mesh axes.
    #[must_use]
    pub fn with_axes(mut self, axes: Vec<TrainingDeviceMeshAxis>) -> Self {
        self.axes = axes;
        self
    }
}

/// One per-rank launch assignment emitted by the bounded planning shell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedLaunchAssignment {
    /// Stable node identifier receiving this rank.
    pub node_id: String,
    /// Global rank inside the launch plan.
    pub rank: usize,
    /// Local rank inside the selected node.
    pub local_rank: usize,
    /// Group member identity materialized for this rank.
    pub member: DistributedGroupMember,
    /// Local bootstrap payload this rank would use to initialize the group.
    pub group_bootstrap: DistributedGroupBootstrap,
    /// Final environment variables passed to the sandbox job.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment: Vec<ProviderSandboxEnvironmentVar>,
    /// Sandbox request that would launch the rank.
    pub sandbox_job: ProviderSandboxJobRequest,
}

/// Machine-readable result of one bounded distributed launch/config plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedLaunchPlan {
    /// Stable launch identifier.
    pub launch_id: String,
    /// Stable digest over the planned launch.
    pub plan_digest: String,
    /// Requested MLX-style backend family.
    pub requested_backend: DistributedBackend,
    /// Effective Psionic runtime backend for the launch.
    pub effective_backend: String,
    /// Backend-family capability mapped onto the planned topology.
    pub backend_capability: DistributedBackendCapability,
    /// Total number of planned ranks.
    pub world_size: usize,
    /// Original hostfile-like entries in rank order.
    pub hostfile_entries: Vec<DistributedHostfileEntry>,
    /// Runtime-visible elastic-membership facts for the planned world.
    pub elastic_membership: TrainingElasticMembershipContext,
    /// Runtime-visible device-mesh facts for the planned world.
    pub device_mesh: TrainingDeviceMeshContext,
    /// Stable distributed-group identifier implied by the plan.
    pub group_id: String,
    /// Ordered group members implied by the plan.
    pub members: Vec<DistributedGroupMember>,
    /// Cluster execution evidence describing the planned launch.
    pub cluster_execution: ClusterExecutionContext,
    /// Ordered per-rank launch assignments.
    pub assignments: Vec<DistributedLaunchAssignment>,
}

/// Initialization failure for the public distributed group surface.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DistributedInitError {
    /// Strict initialization requested a reusable group, but none exists yet.
    #[error(
        "public distributed group init with backend `{backend}` requires an explicit bootstrap or existing global group"
    )]
    GlobalGroupUnavailable {
        /// Requested backend family.
        backend: DistributedBackend,
    },
    /// Requested backend family does not match current topology truth.
    #[error(transparent)]
    BackendFamilyMapping(#[from] DistributedBackendMappingError),
    /// Bootstrapping requires at least one explicit group member.
    #[error("public distributed group bootstrap requires at least one member")]
    BootstrapMembersEmpty,
    /// The bootstrap attempted to register the same node twice.
    #[error("public distributed group bootstrap duplicates node `{node_id}`")]
    DuplicateMemberNodeId {
        /// Node identifier that appeared more than once.
        node_id: String,
    },
    /// The bootstrap attempted to reuse one rank.
    #[error("public distributed group bootstrap duplicates rank {rank}")]
    DuplicateMemberRank {
        /// Rank that appeared more than once.
        rank: usize,
    },
    /// The bootstrap omitted one or more ranks from the ordered member ledger.
    #[error(
        "public distributed group bootstrap ranks must be contiguous from 0, found {actual:?} for size {size}"
    )]
    NonContiguousRanks {
        /// Actual ranks observed in the bootstrap.
        actual: Vec<usize>,
        /// Expected size of the group/member ledger.
        size: usize,
    },
    /// The local node id did not appear in the member ledger.
    #[error(
        "public distributed group bootstrap local node `{local_node_id}` is not in the member ledger"
    )]
    LocalNodeMissing {
        /// Local node identifier requested by the bootstrap.
        local_node_id: String,
    },
    /// The member ledger and mesh member set disagree.
    #[error(
        "public distributed group bootstrap member set mismatch: mesh members {mesh_members:?}, group members {group_members:?}"
    )]
    MeshMemberMismatch {
        /// Distinct node ids carried by the mesh context.
        mesh_members: Vec<String>,
        /// Distinct node ids carried by the explicit member ledger.
        group_members: Vec<String>,
    },
}

/// Split failure for the public distributed group surface.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DistributedGroupError {
    /// The current group has no registered split plan.
    #[error("public distributed group split requires an explicit split plan on the parent group")]
    SplitPlanUnavailable,
    /// The current split plan duplicates one node.
    #[error("public distributed split plan duplicates node `{node_id}`")]
    DuplicateSplitAssignment {
        /// Node identifier that appeared more than once.
        node_id: String,
    },
    /// The current split plan references a node outside the group.
    #[error("public distributed split plan references unknown node `{node_id}`")]
    UnknownSplitNode {
        /// Node identifier that does not exist in the parent group.
        node_id: String,
    },
    /// The current split plan omitted one or more group members.
    #[error("public distributed split plan omitted assignments for nodes {missing_node_ids:?}")]
    MissingSplitAssignments {
        /// Group members that were not assigned a color.
        missing_node_ids: Vec<String>,
    },
    /// The local caller arguments disagree with the explicit split plan.
    #[error(
        "public distributed split local assignment mismatch: expected color {expected_color} key {expected_key}, got color {actual_color} key {actual_key}"
    )]
    LocalAssignmentMismatch {
        /// Color declared by the plan for the local member.
        expected_color: i32,
        /// Effective key declared by the plan for the local member.
        expected_key: i64,
        /// Color supplied by the caller.
        actual_color: i32,
        /// Effective key supplied by the caller.
        actual_key: i64,
    },
    /// Singleton groups cannot be subdivided further.
    #[error("cannot split a singleton public distributed group")]
    CannotSplitSingleton,
    /// The requested color selected no members.
    #[error("public distributed split color {color} selected no members")]
    EmptySplitColor {
        /// Color that selected no members.
        color: i32,
    },
}

/// Collective failure for the public distributed helper surface.
#[derive(Debug, Error, PartialEq)]
pub enum DistributedCollectiveError {
    /// Group resolution reused the public init path and refused there.
    #[error(transparent)]
    GroupInit(#[from] DistributedInitError),
    /// Array materialization or reconstruction refused on the bounded reference path.
    #[error(transparent)]
    Array(#[from] ArrayError),
    /// Floating-point reference storage only admits floating-point logical dtypes.
    #[error("public distributed float reference storage cannot use dtype {dtype:?}")]
    FloatReferenceDTypeMismatch {
        /// Logical dtype requested for the floating reference payload.
        dtype: DType,
    },
    /// One explicit host-owned reference tensor length disagrees with the declared shape.
    #[error(
        "public distributed reference tensor for shape {shape:?} expects {expected_elements} elements, got {actual_elements}"
    )]
    ReferenceElementCountMismatch {
        /// Declared shape.
        shape: Shape,
        /// Expected element count derived from the shape.
        expected_elements: usize,
        /// Actual provided element count.
        actual_elements: usize,
    },
    /// Host export returned a storage family that disagrees with the array dtype.
    #[error(
        "public distributed host export does not provide the expected storage family for dtype {dtype:?}"
    )]
    HostInteropStorageMismatch {
        /// Dtype that disagreed with the host export family.
        dtype: DType,
    },
    /// Multi-rank emulation requires a full explicit rank payload map.
    #[error(
        "public distributed `{operation}` requires explicit rank payloads for all group members; missing {missing_ranks:?}"
    )]
    MissingRankInputs {
        /// Collective helper that refused.
        operation: DistributedCollectiveOperation,
        /// Missing ranks in ascending order.
        missing_ranks: Vec<usize>,
    },
    /// The explicit rank payload map referenced ranks outside the group.
    #[error(
        "public distributed `{operation}` rank payload map contains invalid ranks {invalid_ranks:?} for group size {group_size}"
    )]
    InvalidRankInputs {
        /// Collective helper that refused.
        operation: DistributedCollectiveOperation,
        /// Invalid rank ids.
        invalid_ranks: Vec<usize>,
        /// Group size used for validation.
        group_size: usize,
    },
    /// One rank payload disagreed with the local input array contract.
    #[error(
        "public distributed `{operation}` rank {rank} payload shape/dtype mismatch: expected shape {expected_shape:?} dtype {expected_dtype:?}, got shape {actual_shape:?} dtype {actual_dtype:?}"
    )]
    RankInputMismatch {
        /// Collective helper that refused.
        operation: DistributedCollectiveOperation,
        /// Rank that provided the mismatched payload.
        rank: usize,
        /// Expected shape from the local input array.
        expected_shape: Shape,
        /// Actual shape from the provided rank payload.
        actual_shape: Shape,
        /// Expected dtype from the local input array.
        expected_dtype: DType,
        /// Actual dtype from the provided rank payload.
        actual_dtype: DType,
    },
    /// The explicit local-rank payload did not match the supplied array.
    #[error(
        "public distributed `{operation}` local rank {rank} payload does not match the supplied local array"
    )]
    LocalRankInputMismatch {
        /// Collective helper that refused.
        operation: DistributedCollectiveOperation,
        /// Local rank for the current group.
        rank: usize,
    },
    /// Send semantics refuse on singleton groups, matching MLX.
    #[error("cannot send on a singleton public distributed group")]
    CannotSendSingleton,
    /// Recv semantics refuse on singleton groups, matching MLX.
    #[error("cannot recv on a singleton public distributed group")]
    CannotRecvSingleton,
    /// One destination rank is outside the current group.
    #[error("invalid destination {destination} for public distributed group size {group_size}")]
    InvalidDestination {
        /// Requested destination rank.
        destination: usize,
        /// Current group size.
        group_size: usize,
    },
    /// One source rank is outside the current group.
    #[error("invalid source {source_rank} for public distributed group size {group_size}")]
    InvalidSource {
        /// Requested source rank.
        source_rank: usize,
        /// Current group size.
        group_size: usize,
    },
    /// The bounded recv path requires one explicit host-owned inbound payload.
    #[error(
        "public distributed recv requires an explicit inbound payload for source rank {source_rank}"
    )]
    MissingReceivePayload {
        /// Source rank that lacked an explicit payload.
        source_rank: usize,
    },
    /// The explicit recv payload disagreed with the requested shape or dtype.
    #[error(
        "public distributed recv payload for source rank {source_rank} mismatched: expected shape {expected_shape:?} dtype {expected_dtype:?}, got shape {actual_shape:?} dtype {actual_dtype:?}"
    )]
    ReceivePayloadMismatch {
        /// Source rank that provided the mismatched payload.
        source_rank: usize,
        /// Requested shape.
        expected_shape: Shape,
        /// Provided shape.
        actual_shape: Shape,
        /// Requested dtype.
        expected_dtype: DType,
        /// Provided dtype.
        actual_dtype: DType,
    },
    /// Optional outbound validation found a payload mismatch.
    #[error(
        "public distributed send payload for destination rank {destination} did not match the supplied local array"
    )]
    SendPayloadMismatch {
        /// Destination rank whose expected payload mismatched the local array.
        destination: usize,
    },
    /// Reduce-scatter requires one leading axis to scatter over.
    #[error("public distributed reduce_scatter requires rank >= 1 input")]
    ReduceScatterScalarInput,
    /// Reduce-scatter requires the first axis to divide evenly across the group.
    #[error(
        "public distributed reduce_scatter requires axis 0 length {first_axis} to be divisible by group size {group_size}"
    )]
    ReduceScatterNonDivisibleAxis0 {
        /// Length of axis 0.
        first_axis: usize,
        /// Current group size.
        group_size: usize,
    },
}

/// Launch/config failure for the public framework-distributed planning shell.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DistributedLaunchError {
    /// Launch planning reuses the public distributed-group bootstrap validation.
    #[error(transparent)]
    GroupInit(#[from] DistributedInitError),
    /// Requested backend family does not match current topology truth.
    #[error(transparent)]
    BackendFamilyMapping(DistributedBackendMappingError),
    /// One hostfile-like input produced no usable entries.
    #[error("public distributed hostfile must contain at least one entry")]
    HostfileEmpty,
    /// One hostfile line declared the same node twice.
    #[error("public distributed hostfile duplicates node `{node_id}`")]
    DuplicateHostfileNodeId {
        /// Node identifier that appeared more than once.
        node_id: String,
    },
    /// One hostfile line requested an invalid slot count.
    #[error("public distributed hostfile line {line} has invalid slots `{value}`")]
    InvalidHostfileSlots {
        /// One-based hostfile line number.
        line: usize,
        /// Raw slot value that refused.
        value: String,
    },
    /// One hostfile line carried an unsupported directive.
    #[error("public distributed hostfile line {line} has unsupported token `{token}`")]
    UnsupportedHostfileToken {
        /// One-based hostfile line number.
        line: usize,
        /// Unsupported token.
        token: String,
    },
    /// The current bounded launch planner only supports one rank per node.
    #[error(
        "public distributed launch/config currently supports one rank per node only; node `{node_id}` requested slots={slots}"
    )]
    MultiSlotHostfileEntryUnsupported {
        /// Node that requested more than one rank.
        node_id: String,
        /// Requested slot count.
        slots: usize,
    },
    /// One hostfile node does not exist in the authoritative cluster state.
    #[error(
        "public distributed launch/config node `{node_id}` is not present in the cluster state"
    )]
    ClusterNodeMissing {
        /// Node identifier that was missing.
        node_id: String,
    },
    /// One hostfile node is not an active ready member.
    #[error(
        "public distributed launch/config node `{node_id}` is not ready for new work; found membership status `{status:?}`"
    )]
    ClusterNodeNotReady {
        /// Node identifier that refused planning.
        node_id: String,
        /// Membership status observed in cluster truth.
        status: ClusterMembershipStatus,
    },
    /// One explicit hostfile address disagreed with authoritative cluster truth.
    #[error(
        "public distributed launch/config node `{node_id}` hostfile address `{hostfile_addr}` does not match cluster address `{cluster_addr}`"
    )]
    HostfileAddressMismatch {
        /// Node identifier that mismatched.
        node_id: String,
        /// Address declared by the hostfile entry.
        hostfile_addr: String,
        /// Address declared by authoritative cluster state.
        cluster_addr: String,
    },
    /// The plan requires node telemetry for backend readiness checks.
    #[error("public distributed launch/config node `{node_id}` is missing cluster telemetry")]
    ClusterTelemetryMissing {
        /// Node identifier that lacked telemetry.
        node_id: String,
    },
    /// The selected runtime backend is not ready on one node.
    #[error(
        "public distributed launch/config backend `{backend}` is not ready on node `{node_id}`; found readiness `{status:?}`"
    )]
    BackendNotReady {
        /// Node identifier that refused planning.
        node_id: String,
        /// Effective runtime backend requested by the launch.
        backend: String,
        /// Readiness observed in cluster telemetry.
        status: ClusterBackendReadinessStatus,
    },
    /// The planning node must itself exist in cluster truth.
    #[error(
        "public distributed launch/config scheduler node `{node_id}` is not present in the cluster state"
    )]
    SchedulerNodeMissing {
        /// Scheduler/coordinator node id.
        node_id: String,
    },
    /// The sandbox profile must match the requested execution class.
    #[error(
        "public distributed launch/config execution class `{requested:?}` does not match sandbox profile class `{profile:?}`"
    )]
    SandboxExecutionClassMismatch {
        /// Execution class requested by the launch config.
        requested: ProviderSandboxExecutionClass,
        /// Execution class carried by the sandbox profile.
        profile: ProviderSandboxExecutionClass,
    },
    /// The sandbox runtime is not currently ready for the selected profile.
    #[error(
        "public distributed launch/config sandbox profile `{profile_id}` is not runtime-ready for `{execution_class:?}`"
    )]
    SandboxRuntimeNotReady {
        /// Sandbox profile identifier.
        profile_id: String,
        /// Execution class requested by the launch config.
        execution_class: ProviderSandboxExecutionClass,
    },
    /// Requested timeout exceeded the sandbox profile limit.
    #[error(
        "public distributed launch/config timeout {requested_timeout_s}s exceeds sandbox profile limit {profile_limit_s}s"
    )]
    TimeoutExceedsSandboxProfile {
        /// Timeout requested by the launch config.
        requested_timeout_s: u64,
        /// Timeout limit enforced by the sandbox profile.
        profile_limit_s: u64,
    },
    /// Zero-second timeout is invalid for sandbox jobs.
    #[error("public distributed launch/config timeout_request_s must be greater than zero")]
    TimeoutRequestZero,
    /// Requested resource envelope exceeded the sandbox profile limit.
    #[error(
        "public distributed launch/config {resource} request {requested} exceeds sandbox profile limit {profile_limit}"
    )]
    ResourceRequestExceedsSandboxProfile {
        /// Resource kind that exceeded the profile.
        resource: &'static str,
        /// Requested resource value.
        requested: u64,
        /// Profile limit for that resource.
        profile_limit: u64,
    },
    /// Requested network posture disagreed with the selected sandbox profile.
    #[error(
        "public distributed launch/config network request `{requested}` does not match sandbox profile network mode `{profile}`"
    )]
    NetworkRequestMismatch {
        /// Requested network posture.
        requested: String,
        /// Network mode enforced by the sandbox profile.
        profile: String,
    },
    /// Requested filesystem posture disagreed with the selected sandbox profile.
    #[error(
        "public distributed launch/config filesystem request `{requested}` does not match sandbox profile filesystem mode `{profile}`"
    )]
    FilesystemRequestMismatch {
        /// Requested filesystem posture.
        requested: String,
        /// Filesystem mode enforced by the sandbox profile.
        profile: String,
    },
    /// This sandbox profile forbids environment injection, including required distributed keys.
    #[error(
        "public distributed launch/config sandbox profile `{profile_id}` forbids injected environment, so required distributed keys cannot be emitted"
    )]
    SandboxEnvironmentInjectionForbidden {
        /// Sandbox profile identifier.
        profile_id: String,
    },
    /// The compute product must remain aligned to the sandbox execution class.
    #[error(
        "public distributed launch/config compute product `{compute_product_id}` does not match execution-class product `{expected_product_id}`"
    )]
    ComputeProductIdMismatch {
        /// Compute product requested by the launch config.
        compute_product_id: String,
        /// Product identifier implied by the execution class.
        expected_product_id: String,
    },
    /// Inline payload entrypoints require explicit payload content.
    #[error("public distributed launch/config inline payload entrypoints require payload content")]
    InlinePayloadMissing,
    /// Command entrypoints remain bounded to container execution, matching sandbox truth.
    #[error(
        "public distributed launch/config command entrypoints are only supported for container execution, not `{execution_class:?}`"
    )]
    CommandEntrypointUnsupported {
        /// Execution class requested by the launch config.
        execution_class: ProviderSandboxExecutionClass,
    },
    /// Workspace-file entrypoints must stay inside the workspace root.
    #[error(
        "public distributed launch/config workspace entrypoint `{entrypoint}` must be a non-empty relative workspace path without parent traversal"
    )]
    InvalidWorkspaceEntrypoint {
        /// Invalid workspace-relative entrypoint.
        entrypoint: String,
    },
    /// Expected outputs must stay inside the workspace root.
    #[error(
        "public distributed launch/config expected output `{path}` must be a relative workspace path without parent traversal"
    )]
    InvalidExpectedOutput {
        /// Invalid output path.
        path: String,
    },
    /// Base environment may not override distributed reserved keys.
    #[error("public distributed launch/config cannot override reserved environment key `{key}`")]
    ReservedEnvironmentOverride {
        /// Reserved environment key.
        key: String,
    },
    /// Explicit mesh axes must multiply to the selected world size.
    #[error(
        "public distributed launch/config mesh axes multiply to {axis_product}, but world size is {world_size}"
    )]
    AxisProductMismatch {
        /// Product of declared axis extents.
        axis_product: usize,
        /// Planned world size.
        world_size: usize,
    },
}

/// Tree-aware gradient reduction failure above the public collective layer.
#[derive(Debug, Error, PartialEq)]
pub enum DistributedGradientReductionError {
    /// Lower collective semantics still govern grouped gradient reduction.
    #[error(transparent)]
    Collective(#[from] DistributedCollectiveError),
    /// Tree rebuilding still uses the public deterministic tree utilities.
    #[error(transparent)]
    Tree(#[from] TreeError),
    /// Every rank must provide the same tree structure as the local gradients.
    #[error(
        "public distributed gradient tree for rank {rank} has structure {actual:?}, expected {expected:?}"
    )]
    RankInputTreeStructureMismatch {
        /// Rank whose tree structure disagreed with the local tree.
        rank: usize,
        /// Local gradient tree structure.
        expected: TreeSpec,
        /// Remote gradient tree structure.
        actual: TreeSpec,
    },
    /// Gradient averaging only supports floating-point leaves on the current public surface.
    #[error(
        "public distributed average_gradients requires floating-point leaves, found dtype {dtype:?} at leaf {leaf_index}"
    )]
    NonFloatingGradientLeaf {
        /// Leaf index in deterministic traversal order.
        leaf_index: usize,
        /// Dtype that refused averaging.
        dtype: DType,
    },
}

#[derive(Clone, Debug)]
struct DistributedGroupState {
    group_id: String,
    kind: DistributedGroupKind,
    requested_backend: DistributedBackend,
    mesh: TrainingDeviceMeshContext,
    transport: ClusterTransportClass,
    local_node_id: String,
    members: Vec<DistributedGroupMember>,
    local_rank: usize,
    parent_group_id: Option<String>,
}

#[derive(Default)]
struct GlobalDistributedGroups {
    groups: BTreeMap<DistributedBackend, Arc<DistributedGroupState>>,
}

struct DistributedLaunchEnvironmentFacts<'a> {
    rank: usize,
    local_rank: usize,
    world_size: usize,
    node_id: &'a str,
    cluster_id: &'a str,
    cluster_state_digest: &'a str,
    topology_digest: &'a str,
    device_mesh: &'a TrainingDeviceMeshContext,
    group_id: &'a str,
    effective_backend: &'a str,
}

struct DistributedLaunchDigestFacts<'a> {
    cluster_id: &'a str,
    cluster_state_digest: &'a str,
    topology_digest: &'a str,
    elastic_membership: &'a TrainingElasticMembershipContext,
    device_mesh: &'a TrainingDeviceMeshContext,
    group_id: &'a str,
    members: &'a [DistributedGroupMember],
    cluster_execution: &'a ClusterExecutionContext,
    assignments: &'a [DistributedLaunchAssignment],
}

/// Public framework-visible distributed group handle.
#[derive(Clone, Debug)]
pub struct DistributedGroup {
    state: Arc<DistributedGroupState>,
    split_plan: Option<Arc<DistributedSplitPlan>>,
}

impl DistributedGroup {
    fn from_state(state: Arc<DistributedGroupState>) -> Self {
        Self {
            state,
            split_plan: None,
        }
    }

    fn singleton(requested_backend: DistributedBackend) -> Self {
        let member = DistributedGroupMember::new("singleton", 0, 0, "cpu:0");
        let membership = TrainingElasticMembershipContext::new(
            0,
            "singleton_cluster_state",
            "singleton_topology",
            vec![member.node_id.clone()],
        );
        let mesh = TrainingDeviceMeshContext::new(
            "singleton.mesh",
            0,
            "cpu",
            ClusterCommunicationClass::TensorCollectiveMesh,
            membership,
            vec![member.node_id.clone()],
        );
        let members = vec![member];
        let state = DistributedGroupState {
            group_id: stable_group_id(
                DistributedGroupKind::SingletonFallback,
                requested_backend,
                &mesh,
                ClusterTransportClass::LocalOnly,
                &members,
                None,
            ),
            kind: DistributedGroupKind::SingletonFallback,
            requested_backend,
            mesh,
            transport: ClusterTransportClass::LocalOnly,
            local_node_id: String::from("singleton"),
            members,
            local_rank: 0,
            parent_group_id: None,
        };
        Self::from_state(Arc::new(state))
    }

    /// Returns the stable public group identifier.
    #[must_use]
    pub fn group_id(&self) -> &str {
        &self.state.group_id
    }

    /// Returns the high-level provenance for this group.
    #[must_use]
    pub fn kind(&self) -> DistributedGroupKind {
        self.state.kind
    }

    /// Returns the requested distributed backend family.
    #[must_use]
    pub fn requested_backend(&self) -> DistributedBackend {
        self.state.requested_backend
    }

    /// Returns the effective runtime backend carried by the underlying mesh.
    #[must_use]
    pub fn effective_backend(&self) -> &str {
        &self.state.mesh.effective_backend
    }

    /// Returns the communication class carried by the underlying mesh.
    #[must_use]
    pub fn communication_class(&self) -> ClusterCommunicationClass {
        self.state.mesh.communication_class
    }

    /// Returns the stable local node identifier.
    #[must_use]
    pub fn local_node_id(&self) -> &str {
        &self.state.local_node_id
    }

    /// Returns the local rank in the current group.
    #[must_use]
    pub fn rank(&self) -> usize {
        self.state.local_rank
    }

    /// Returns the total size of the current group.
    #[must_use]
    pub fn size(&self) -> usize {
        self.state.members.len()
    }

    /// Returns whether the current group is a singleton fallback.
    #[must_use]
    pub fn is_singleton(&self) -> bool {
        self.size() == 1
    }

    /// Returns the ordered member ledger for this group.
    #[must_use]
    pub fn members(&self) -> &[DistributedGroupMember] {
        &self.state.members
    }

    /// Returns the underlying mesh context.
    #[must_use]
    pub fn mesh(&self) -> &TrainingDeviceMeshContext {
        &self.state.mesh
    }

    /// Returns the machine-readable snapshot for this group.
    #[must_use]
    pub fn snapshot(&self) -> DistributedGroupSnapshot {
        let backend_capability = self.backend_capability();
        DistributedGroupSnapshot {
            group_id: self.state.group_id.clone(),
            kind: self.state.kind,
            requested_backend: self.state.requested_backend,
            effective_backend: self.state.mesh.effective_backend.clone(),
            communication_class: self.state.mesh.communication_class,
            transport: self.state.transport,
            backend_capability,
            mesh_id: self.state.mesh.mesh_id.clone(),
            mesh_revision: self.state.mesh.mesh_revision,
            local_node_id: self.state.local_node_id.clone(),
            rank: self.state.local_rank,
            size: self.state.members.len(),
            members: self.state.members.clone(),
            parent_group_id: self.state.parent_group_id.clone(),
        }
    }

    /// Returns the backend-family capability mapped onto current topology truth.
    #[must_use]
    pub fn backend_capability(&self) -> DistributedBackendCapability {
        resolve_distributed_backend_capability(
            self.state.kind,
            self.state.requested_backend,
            self.state.mesh.effective_backend.as_str(),
            self.state.mesh.communication_class,
            self.state.transport,
            self.state.members.len(),
        )
    }

    /// Returns the current capability snapshot for public collective helpers on this group.
    #[must_use]
    pub fn collective_support(&self) -> DistributedCollectiveSupport {
        let backend_capability = self.backend_capability();
        let (all_sum, all_gather, reduce_scatter, send, recv, boundary_note) = if self
            .is_singleton()
        {
            (
                DistributedCollectiveSupportStatus::SingletonPassthrough,
                DistributedCollectiveSupportStatus::SingletonPassthrough,
                DistributedCollectiveSupportStatus::SingletonPassthrough,
                DistributedCollectiveSupportStatus::TypedRefusal,
                DistributedCollectiveSupportStatus::TypedRefusal,
                String::from(
                    "Singleton passthrough for all_sum, all_gather, and reduce_scatter is real today; send and recv still refuse because there is no peer process in the group. Backend-family requests collapse to a singleton fallback until an explicit distributed mesh is bootstrapped.",
                ),
            )
        } else {
            (
                DistributedCollectiveSupportStatus::ReferenceEmulation,
                DistributedCollectiveSupportStatus::ReferenceEmulation,
                DistributedCollectiveSupportStatus::ReferenceEmulation,
                DistributedCollectiveSupportStatus::ValidationOnly,
                DistributedCollectiveSupportStatus::ReferenceEmulation,
                format!(
                    "Multi-rank collective helpers are currently bounded to explicit host-owned reference payloads above the group surface; backend family `{}` maps onto current Psionic topology as `{}` with communication class `{:?}`, but backend transport execution still remains later work. {}",
                    backend_capability.resolved_backend,
                    backend_capability.topology_profile,
                    backend_capability.communication_class,
                    backend_capability.detail,
                ),
            )
        };
        DistributedCollectiveSupport {
            group_id: self.state.group_id.clone(),
            group_kind: self.state.kind,
            requested_backend: self.state.requested_backend,
            effective_backend: self.state.mesh.effective_backend.clone(),
            communication_class: self.state.mesh.communication_class,
            transport: self.state.transport,
            backend_capability: backend_capability.clone(),
            backend_transport_available: backend_capability.backend_transport_available,
            all_sum,
            all_gather,
            reduce_scatter,
            send,
            recv,
            boundary_note,
        }
    }

    /// Returns a cloned group handle carrying one explicit split plan.
    #[must_use]
    pub fn with_split_plan(&self, split_plan: DistributedSplitPlan) -> Self {
        Self {
            state: Arc::clone(&self.state),
            split_plan: Some(Arc::new(split_plan)),
        }
    }

    /// Splits the current group using the registered explicit split plan.
    pub fn split(&self, color: i32, key: i64) -> Result<Self, DistributedGroupError> {
        if self.is_singleton() {
            return Err(DistributedGroupError::CannotSplitSingleton);
        }
        let Some(split_plan) = self.split_plan.as_ref() else {
            return Err(DistributedGroupError::SplitPlanUnavailable);
        };

        let assignments = split_assignment_map(split_plan.as_ref(), &self.state.members)?;
        let local_member = &self.state.members[self.state.local_rank];
        let local_assignment = assignments
            .get(local_member.node_id.as_str())
            .ok_or_else(|| DistributedGroupError::MissingSplitAssignments {
                missing_node_ids: vec![local_member.node_id.clone()],
            })?;
        let expected_key = effective_split_key(local_assignment.key, local_member.rank);
        let actual_key = effective_split_key((key >= 0).then_some(key), local_member.rank);
        if local_assignment.color != color || expected_key != actual_key {
            return Err(DistributedGroupError::LocalAssignmentMismatch {
                expected_color: local_assignment.color,
                expected_key,
                actual_color: color,
                actual_key,
            });
        }

        let mut selected = self
            .state
            .members
            .iter()
            .filter_map(|member| {
                assignments
                    .get(member.node_id.as_str())
                    .filter(|assignment| assignment.color == color)
                    .map(|assignment| {
                        (
                            effective_split_key(assignment.key, member.rank),
                            member.rank,
                            member.clone(),
                        )
                    })
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(DistributedGroupError::EmptySplitColor { color });
        }
        selected.sort_by_key(|(effective_key, previous_rank, _)| (*effective_key, *previous_rank));

        let mut subgroup_members = selected
            .into_iter()
            .enumerate()
            .map(|(new_rank, (_, _, member))| {
                DistributedGroupMember::new(
                    member.node_id,
                    new_rank,
                    member.shard_id,
                    member.device_label,
                )
            })
            .collect::<Vec<_>>();
        let local_rank = subgroup_members
            .iter()
            .position(|member| member.node_id == self.state.local_node_id)
            .ok_or(DistributedGroupError::EmptySplitColor { color })?;

        let subgroup_mesh = build_subgroup_mesh(&self.state.mesh, color, &subgroup_members);
        let state = DistributedGroupState {
            group_id: stable_group_id(
                DistributedGroupKind::SplitSubgroup,
                self.state.requested_backend,
                &subgroup_mesh,
                self.state.transport,
                &subgroup_members,
                Some(self.state.group_id.as_str()),
            ),
            kind: DistributedGroupKind::SplitSubgroup,
            requested_backend: self.state.requested_backend,
            mesh: subgroup_mesh,
            transport: self.state.transport,
            local_node_id: self.state.local_node_id.clone(),
            members: std::mem::take(&mut subgroup_members),
            local_rank,
            parent_group_id: Some(self.state.group_id.clone()),
        };
        Ok(Self::from_state(Arc::new(state)))
    }

    /// Splits the current group using one explicit split plan.
    pub fn split_with_plan(
        &self,
        color: i32,
        key: i64,
        split_plan: DistributedSplitPlan,
    ) -> Result<Self, DistributedGroupError> {
        self.with_split_plan(split_plan).split(color, key)
    }
}

/// Returns the current reusable global group for one backend family, when one exists.
#[must_use]
pub fn global_group(backend: DistributedBackend) -> Option<DistributedGroup> {
    lock_global_groups()
        .groups
        .get(&backend)
        .cloned()
        .map(DistributedGroup::from_state)
}

/// Initializes or reuses one public distributed group.
pub fn init(options: DistributedInitOptions) -> Result<DistributedGroup, DistributedInitError> {
    if let Some(bootstrap) = options.bootstrap {
        let state = Arc::new(build_bootstrapped_state(options.backend, bootstrap)?);
        register_global_group(Arc::clone(&state));
        return Ok(DistributedGroup::from_state(state));
    }

    if let Some(group) = global_group(options.backend) {
        return Ok(group);
    }
    if options.strict {
        return Err(DistributedInitError::GlobalGroupUnavailable {
            backend: options.backend,
        });
    }
    Ok(DistributedGroup::singleton(options.backend))
}

/// All-reduce sum above the public distributed-group surface.
pub fn all_sum(
    x: &Array,
    options: DistributedCollectiveOptions,
) -> Result<Array, DistributedCollectiveError> {
    let group = resolve_collective_group(options.group)?;
    if group.is_singleton() {
        return Ok(x.clone());
    }
    let ordered = ordered_rank_inputs(
        DistributedCollectiveOperation::AllSum,
        x,
        &group,
        &options.rank_inputs,
    )?;
    let mut accumulated = vec![0.0; x.shape().element_count()];
    for payload in ordered {
        for (slot, value) in accumulated.iter_mut().zip(payload.as_f32_values()) {
            *slot += value;
        }
    }
    build_output_array(&x.context(), x.shape().clone(), x.dtype(), accumulated)
}

/// First-axis all-gather above the public distributed-group surface.
pub fn all_gather(
    x: &Array,
    options: DistributedCollectiveOptions,
) -> Result<Array, DistributedCollectiveError> {
    let group = resolve_collective_group(options.group)?;
    if group.is_singleton() {
        return Ok(x.clone());
    }
    let ordered = ordered_rank_inputs(
        DistributedCollectiveOperation::AllGather,
        x,
        &group,
        &options.rank_inputs,
    )?;
    let output_shape = gathered_shape(x.shape(), group.size());
    let gathered = ordered
        .into_iter()
        .flat_map(|payload| payload.as_f32_values())
        .collect::<Vec<_>>();
    build_output_array(&x.context(), output_shape, x.dtype(), gathered)
}

/// Sum-only reduce-scatter above the public distributed-group surface.
pub fn reduce_scatter(
    x: &Array,
    options: DistributedCollectiveOptions,
) -> Result<Array, DistributedCollectiveError> {
    let group = resolve_collective_group(options.group)?;
    if group.is_singleton() {
        return Ok(x.clone());
    }
    let ordered = ordered_rank_inputs(
        DistributedCollectiveOperation::ReduceScatter,
        x,
        &group,
        &options.rank_inputs,
    )?;
    let mut reduced = vec![0.0; x.shape().element_count()];
    for payload in ordered {
        for (slot, value) in reduced.iter_mut().zip(payload.as_f32_values()) {
            *slot += value;
        }
    }
    let (output_shape, scattered) =
        reduce_scatter_values(x.shape(), group.size(), group.rank(), reduced)?;
    build_output_array(&x.context(), output_shape, x.dtype(), scattered)
}

/// Sum-only reduce-scatter alias matching the upstream MLX helper name.
pub fn sum_scatter(
    x: &Array,
    options: DistributedCollectiveOptions,
) -> Result<Array, DistributedCollectiveError> {
    reduce_scatter(x, options)
}

/// Sum-only grouped all-reduce over one tree of gradient leaves.
pub fn grouped_all_sum(
    gradients: &Tree<Array>,
    options: DistributedGradientReductionOptions,
) -> Result<Tree<Array>, DistributedGradientReductionError> {
    let group = resolve_collective_group(options.group)?;
    if group.is_singleton() {
        return Ok(gradients.clone());
    }

    let local_spec = gradients.spec();
    let local_leaves = gradients.leaves();
    if local_leaves.is_empty() {
        return Ok(local_spec.unflatten(Vec::new())?);
    }
    let rank_leaves = validated_rank_gradient_trees(
        &group,
        &local_spec,
        local_leaves.as_slice(),
        &options.rank_inputs,
    )?;

    let groups = gradient_leaf_groups(
        local_leaves.as_slice(),
        options.small_tensor_bytes_threshold,
    );
    let mut reduced_leaves = Vec::with_capacity(local_leaves.len());
    for leaf_group in groups {
        let local_payloads = leaf_group
            .leaf_indices
            .iter()
            .map(|index| DistributedReferenceTensor::from_array(&local_leaves[*index]))
            .collect::<Result<Vec<_>, _>>()?;
        let local_concat = concatenate_reference_tensors(local_payloads.as_slice())?;
        let local_concat_array =
            local_concat.to_array(&local_leaves[leaf_group.leaf_indices[0]].context())?;
        let mut rank_inputs = BTreeMap::new();
        for (rank, rank_leaf_tensors) in &rank_leaves {
            let payloads = leaf_group
                .leaf_indices
                .iter()
                .map(|index| rank_leaf_tensors[*index].clone())
                .collect::<Vec<_>>();
            rank_inputs.insert(*rank, concatenate_reference_tensors(payloads.as_slice())?);
        }
        let reduced = all_sum(
            &local_concat_array,
            DistributedCollectiveOptions::new()
                .with_group(group.clone())
                .with_rank_inputs(rank_inputs),
        )?;
        let reduced_reference = DistributedReferenceTensor::from_array(&reduced)?;
        reduced_leaves.extend(split_reduced_group(
            &reduced_reference,
            local_leaves.as_slice(),
            &leaf_group,
        )?);
    }
    Ok(local_spec.unflatten(reduced_leaves)?)
}

/// Alias matching the grouped all-reduce naming family from upstream docs.
pub fn grouped_all_reduce(
    gradients: &Tree<Array>,
    options: DistributedGradientReductionOptions,
) -> Result<Tree<Array>, DistributedGradientReductionError> {
    grouped_all_sum(gradients, options)
}

/// Averages one tree of floating-point gradients across the current distributed group.
pub fn average_gradients(
    gradients: &Tree<Array>,
    options: DistributedGradientReductionOptions,
) -> Result<Tree<Array>, DistributedGradientReductionError> {
    let group = resolve_collective_group(options.group.clone())?;
    if group.is_singleton() {
        return Ok(gradients.clone());
    }
    let local_leaves = gradients.leaves();
    for (leaf_index, leaf) in local_leaves.iter().enumerate() {
        if !matches!(leaf.dtype(), DType::F32 | DType::F16 | DType::BF16) {
            return Err(DistributedGradientReductionError::NonFloatingGradientLeaf {
                leaf_index,
                dtype: leaf.dtype(),
            });
        }
    }
    grouped_all_sum(gradients, options.with_group(group.clone()))?
        .map_leaves(&mut |leaf| scale_gradient_leaf(leaf, 1.0 / group.size() as f32))
}

/// Point-to-point send above the public distributed-group surface.
pub fn send(
    x: &Array,
    destination: usize,
    options: DistributedPointToPointOptions,
) -> Result<Array, DistributedCollectiveError> {
    let group = resolve_collective_group(options.group)?;
    if group.is_singleton() {
        return Err(DistributedCollectiveError::CannotSendSingleton);
    }
    validate_destination(destination, group.size())?;
    validate_point_to_point_payload_ranks(
        DistributedCollectiveOperation::Send,
        group.size(),
        &options.message_payloads,
    )?;
    if let Some(expected_payload) = options.message_payloads.get(&destination) {
        validate_rank_payload_shape_dtype(
            DistributedCollectiveOperation::Send,
            destination,
            x.shape(),
            x.dtype(),
            expected_payload,
        )?;
        let local_payload = DistributedReferenceTensor::from_array(x)?;
        if local_payload != *expected_payload {
            return Err(DistributedCollectiveError::SendPayloadMismatch { destination });
        }
    }
    Ok(x.clone())
}

/// Point-to-point recv above the public distributed-group surface.
pub fn recv(
    context: &ArrayContext,
    shape: Shape,
    dtype: DType,
    source: usize,
    options: DistributedPointToPointOptions,
) -> Result<Array, DistributedCollectiveError> {
    let group = resolve_collective_group(options.group)?;
    if group.is_singleton() {
        return Err(DistributedCollectiveError::CannotRecvSingleton);
    }
    validate_source(source, group.size())?;
    validate_point_to_point_payload_ranks(
        DistributedCollectiveOperation::Recv,
        group.size(),
        &options.message_payloads,
    )?;
    let payload = options.message_payloads.get(&source).ok_or(
        DistributedCollectiveError::MissingReceivePayload {
            source_rank: source,
        },
    )?;
    if payload.shape() != &shape || payload.dtype() != dtype {
        return Err(DistributedCollectiveError::ReceivePayloadMismatch {
            source_rank: source,
            expected_shape: shape,
            actual_shape: payload.shape().clone(),
            expected_dtype: dtype,
            actual_dtype: payload.dtype(),
        });
    }
    payload.to_array(context)
}

/// Point-to-point recv convenience wrapper using the shape and dtype of `like`.
pub fn recv_like(
    like: &Array,
    source: usize,
    options: DistributedPointToPointOptions,
) -> Result<Array, DistributedCollectiveError> {
    recv(
        &like.context(),
        like.shape().clone(),
        like.dtype(),
        source,
        options,
    )
}

/// Parses one hostfile-like string into explicit launch targets.
pub fn parse_hostfile(
    hostfile: &str,
) -> Result<Vec<DistributedHostfileEntry>, DistributedLaunchError> {
    let mut entries = Vec::new();
    let mut seen_node_ids = BTreeSet::new();
    for (line_index, raw_line) in hostfile.lines().enumerate() {
        let line = raw_line
            .split_once('#')
            .map_or(raw_line, |(without_comment, _)| without_comment)
            .trim();
        if line.is_empty() {
            continue;
        }
        let line_number = line_index.saturating_add(1);
        let mut tokens = line.split_whitespace();
        let Some(node_id) = tokens.next() else {
            continue;
        };
        let mut entry = DistributedHostfileEntry::new(node_id);
        for token in tokens {
            if let Some(value) = token.strip_prefix("slots=") {
                let slots = value
                    .parse::<usize>()
                    .ok()
                    .filter(|slots| *slots > 0)
                    .ok_or_else(|| DistributedLaunchError::InvalidHostfileSlots {
                        line: line_number,
                        value: value.to_string(),
                    })?;
                entry = entry.with_slots(slots);
            } else if let Some(value) = token.strip_prefix("addr=") {
                entry = entry.with_advertised_addr(value);
            } else {
                return Err(DistributedLaunchError::UnsupportedHostfileToken {
                    line: line_number,
                    token: token.to_string(),
                });
            }
        }
        if !seen_node_ids.insert(entry.node_id.clone()) {
            return Err(DistributedLaunchError::DuplicateHostfileNodeId {
                node_id: entry.node_id,
            });
        }
        entries.push(entry);
    }
    if entries.is_empty() {
        return Err(DistributedLaunchError::HostfileEmpty);
    }
    Ok(entries)
}

/// Plans one bounded framework-distributed launch against cluster and sandbox truth.
pub fn plan_launch(
    cluster_state: &ClusterState,
    sandbox_profile: &ProviderSandboxProfile,
    config: DistributedLaunchConfig,
) -> Result<DistributedLaunchPlan, DistributedLaunchError> {
    if config.hostfile_entries.is_empty() {
        return Err(DistributedLaunchError::HostfileEmpty);
    }
    validate_launch_config(cluster_state, sandbox_profile, &config)?;

    let cluster_state_digest = cluster_state.stable_digest();
    let topology_digest = cluster_state.topology_digest();
    let world_size = config.hostfile_entries.len();
    let membership_epoch = cluster_state
        .last_applied_event_index()
        .map_or(1, |index| index.as_u64());
    let active_node_ids = config
        .hostfile_entries
        .iter()
        .map(|entry| entry.node_id.clone())
        .collect::<Vec<_>>();
    let axes = if config.axes.is_empty() {
        vec![TrainingDeviceMeshAxis::new(
            "data_parallel",
            TrainingDeviceMeshAxisKind::DataParallel,
            world_size,
        )]
    } else {
        config.axes.clone()
    };
    let elastic_membership = TrainingElasticMembershipContext::new(
        membership_epoch,
        cluster_state_digest.clone(),
        topology_digest.clone(),
        active_node_ids.clone(),
    );
    let device_mesh = TrainingDeviceMeshContext::new(
        format!("{}.mesh", config.launch_id),
        membership_epoch,
        config.effective_backend.clone(),
        ClusterCommunicationClass::TensorCollectiveMesh,
        elastic_membership.clone(),
        active_node_ids,
    )
    .with_axes(axes);
    let backend_capability = validate_requested_backend_mapping(
        DistributedGroupKind::BootstrappedMesh,
        config.requested_backend,
        config.effective_backend.as_str(),
        device_mesh.communication_class,
        config.transport,
        world_size,
    )
    .map_err(DistributedLaunchError::BackendFamilyMapping)?;

    let members = config
        .hostfile_entries
        .iter()
        .enumerate()
        .map(|(rank, entry)| {
            DistributedGroupMember::new(
                entry.node_id.clone(),
                rank,
                rank,
                format!("{}:0", config.effective_backend),
            )
        })
        .collect::<Vec<_>>();

    let representative_state = build_bootstrapped_state(
        config.requested_backend,
        DistributedGroupBootstrap::new(
            device_mesh.clone(),
            config.hostfile_entries[0].node_id.clone(),
            members.clone(),
        )
        .with_transport(config.transport),
    )
    .map_err(DistributedLaunchError::from)?;
    let group_id = representative_state.group_id;

    let selected_nodes = config
        .hostfile_entries
        .iter()
        .map(|entry| {
            ClusterSelectedNode::new(entry.node_id.clone(), config.effective_backend.clone())
        })
        .collect::<Vec<_>>();
    let communication_eligibility = ClusterCommunicationEligibility::new(
        config.effective_backend.clone(),
        ClusterCommunicationClass::TensorCollectiveMesh,
    )
    .with_supported_classes(vec![ClusterCommunicationClass::TensorCollectiveMesh])
    .with_detail(format!(
        "framework distributed launch plan maps requested backend `{}` onto resolved backend `{}` over topology profile `{}`",
        config.requested_backend,
        backend_capability.resolved_backend,
        backend_capability.topology_profile,
    ));
    let single_node = config.hostfile_entries.len() == 1;
    let disposition = if single_node {
        if config.hostfile_entries[0].node_id == config.scheduler_node_id {
            ClusterExecutionDisposition::LocalOnly
        } else {
            ClusterExecutionDisposition::RemoteWholeRequest
        }
    } else {
        ClusterExecutionDisposition::Sharded
    };
    let mut cluster_execution = ClusterExecutionContext::new(
        cluster_state.cluster_id().as_str(),
        cluster_state_digest.clone(),
        topology_digest.clone(),
        config.scheduler_node_id.clone(),
        config.transport,
        disposition,
    )
    .with_communication_eligibility(communication_eligibility)
    .with_selected_nodes(selected_nodes)
    .with_placement_diagnostic(format!(
        "planned from explicit distributed hostfile with {} entries; {}",
        config.hostfile_entries.len(),
        backend_capability.detail
    ));
    if let Some(commit_authority) = cluster_commit_authority_evidence(cluster_state) {
        cluster_execution = cluster_execution.with_commit_authority(commit_authority);
    }

    let compute_product_id = config
        .compute_product_id
        .clone()
        .unwrap_or_else(|| config.execution_class.product_id().to_string());
    let assignments = config
        .hostfile_entries
        .iter()
        .zip(members.iter())
        .enumerate()
        .map(|(rank, (entry, member))| {
            let local_rank = 0;
            let group_bootstrap = DistributedGroupBootstrap::new(
                device_mesh.clone(),
                entry.node_id.clone(),
                members.clone(),
            )
            .with_transport(config.transport);
            let environment = launch_environment(
                config.environment.as_slice(),
                &DistributedLaunchEnvironmentFacts {
                    rank,
                    local_rank,
                    world_size,
                    node_id: entry.node_id.as_str(),
                    cluster_id: cluster_state.cluster_id().as_str(),
                    cluster_state_digest: cluster_state_digest.as_str(),
                    topology_digest: topology_digest.as_str(),
                    device_mesh: &device_mesh,
                    group_id: group_id.as_str(),
                    effective_backend: config.effective_backend.as_str(),
                },
            );
            let sandbox_job = ProviderSandboxJobRequest {
                job_id: format!("{}.rank{rank}", config.launch_id),
                provider_id: config.provider_id.clone(),
                compute_product_id: compute_product_id.clone(),
                execution_class: config.execution_class,
                entrypoint_type: config.entrypoint_type,
                entrypoint: config.entrypoint.clone(),
                payload: config.payload.clone(),
                arguments: config.arguments.clone(),
                workspace_root: config.workspace_root.clone(),
                expected_outputs: config.expected_outputs.clone(),
                timeout_request_s: config.timeout_request_s,
                network_request: config.network_request.clone(),
                filesystem_request: config.filesystem_request.clone(),
                environment: environment.clone(),
                resource_request: config.resource_request.clone(),
                payout_reference: config.payout_reference.clone(),
                verification_posture: config.verification_posture.clone(),
            };
            DistributedLaunchAssignment {
                node_id: entry.node_id.clone(),
                rank,
                local_rank,
                member: member.clone(),
                group_bootstrap,
                environment,
                sandbox_job,
            }
        })
        .collect::<Vec<_>>();

    let plan_digest = stable_launch_plan_digest(
        &config,
        &DistributedLaunchDigestFacts {
            cluster_id: cluster_state.cluster_id().as_str(),
            cluster_state_digest: cluster_state_digest.as_str(),
            topology_digest: topology_digest.as_str(),
            elastic_membership: &elastic_membership,
            device_mesh: &device_mesh,
            group_id: group_id.as_str(),
            members: members.as_slice(),
            cluster_execution: &cluster_execution,
            assignments: assignments.as_slice(),
        },
    );

    Ok(DistributedLaunchPlan {
        launch_id: config.launch_id,
        plan_digest,
        requested_backend: config.requested_backend,
        effective_backend: config.effective_backend,
        backend_capability,
        world_size,
        hostfile_entries: config.hostfile_entries,
        elastic_membership,
        device_mesh,
        group_id,
        members,
        cluster_execution,
        assignments,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GradientLeafGroup {
    leaf_indices: Vec<usize>,
}

fn resolve_collective_group(
    group: Option<DistributedGroup>,
) -> Result<DistributedGroup, DistributedCollectiveError> {
    match group {
        Some(group) => Ok(group),
        None => Ok(init(DistributedInitOptions::new())?),
    }
}

fn tensor_parallel_linear_layout(
    helper: TensorParallelLinearKind,
    group: DistributedGroupSnapshot,
    global_in_features: usize,
    global_out_features: usize,
) -> Result<TensorParallelLinearLayout, TensorParallelLinearError> {
    let axis_size = match helper {
        TensorParallelLinearKind::AllToSharded => global_out_features,
        TensorParallelLinearKind::ShardedToAll => global_in_features,
    };
    let (shard_start, shard_end) = partition_feature_axis(axis_size, group.rank, group.size)
        .ok_or(TensorParallelLinearError::FeatureAxisTooSmall {
            helper,
            axis_size,
            world_size: group.size,
        })?;
    Ok(TensorParallelLinearLayout {
        helper,
        group,
        global_in_features,
        global_out_features,
        shard_axis: match helper {
            TensorParallelLinearKind::AllToSharded => 0,
            TensorParallelLinearKind::ShardedToAll => 1,
        },
        shard_start,
        shard_end,
    })
}

fn partition_feature_axis(
    axis_size: usize,
    rank: usize,
    world_size: usize,
) -> Option<(usize, usize)> {
    if world_size == 0 || rank >= world_size || axis_size < world_size {
        return None;
    }
    let base = axis_size / world_size;
    let remainder = axis_size % world_size;
    let start = (rank * base) + remainder.min(rank);
    let width = base + usize::from(rank < remainder);
    Some((start, start + width))
}

fn slice_linear_rows(
    weight: &[f32],
    out_features: usize,
    in_features: usize,
    row_start: usize,
    row_end: usize,
) -> Vec<f32> {
    let mut local_weight = Vec::with_capacity((row_end - row_start) * in_features);
    for row in row_start..row_end {
        let offset = row * in_features;
        local_weight.extend_from_slice(&weight[offset..offset + in_features]);
    }
    debug_assert_eq!(weight.len(), out_features * in_features);
    local_weight
}

fn slice_linear_columns(
    weight: &[f32],
    out_features: usize,
    in_features: usize,
    column_start: usize,
    column_end: usize,
) -> Vec<f32> {
    let local_in_features = column_end - column_start;
    let mut local_weight = Vec::with_capacity(out_features * local_in_features);
    for row in 0..out_features {
        let offset = row * in_features;
        local_weight.extend_from_slice(&weight[offset + column_start..offset + column_end]);
    }
    local_weight
}

fn slice_last_dim(
    input: &NnTensor,
    shard_start: usize,
    shard_end: usize,
    helper: TensorParallelLinearKind,
    global_width: usize,
) -> Result<NnTensor, TensorParallelLinearError> {
    let dims = input.dims();
    if dims.is_empty() {
        return Err(TensorParallelLinearError::InputShapeMismatch {
            helper,
            expected: format!("rank >= 1 with trailing dimension {global_width}"),
            actual: dims.to_vec(),
        });
    }
    if dims[dims.len() - 1] != global_width {
        return Err(TensorParallelLinearError::InputShapeMismatch {
            helper,
            expected: format!("last dimension {global_width}"),
            actual: dims.to_vec(),
        });
    }
    let values = input.as_f32_slice()?;
    let shard_width = shard_end - shard_start;
    let rows = dims[..dims.len() - 1].iter().product::<usize>().max(1);
    let mut output = Vec::with_capacity(rows * shard_width);
    for row in 0..rows {
        let offset = row * global_width;
        output.extend_from_slice(&values[offset + shard_start..offset + shard_end]);
    }
    let mut output_dims = dims.to_vec();
    let Some(last_dim) = output_dims.last_mut() else {
        return Err(TensorParallelLinearError::InputShapeMismatch {
            helper,
            expected: format!("rank >= 1 with trailing dimension {global_width}"),
            actual: dims.to_vec(),
        });
    };
    *last_dim = shard_width;
    Ok(NnTensor::f32(Shape::new(output_dims), output)?)
}

fn validate_tensor_parallel_group(
    layout: &TensorParallelLinearLayout,
    actual: &DistributedGroupSnapshot,
    helper: TensorParallelLinearKind,
) -> Result<(), TensorParallelLinearError> {
    if layout.group.group_id != actual.group_id
        || layout.group.rank != actual.rank
        || layout.group.size != actual.size
    {
        return Err(TensorParallelLinearError::GroupMismatch {
            helper,
            expected_group_id: layout.group.group_id.clone(),
            expected_rank: layout.group.rank,
            expected_world_size: layout.group.size,
            actual_group_id: actual.group_id.clone(),
            actual_rank: actual.rank,
            actual_world_size: actual.size,
        });
    }
    Ok(())
}

fn validate_rank_modules(
    layout: &TensorParallelLinearLayout,
    rank_modules: &BTreeMap<usize, ShardedToAllLinear>,
) -> Result<(), TensorParallelLinearError> {
    let invalid_ranks = rank_modules
        .keys()
        .copied()
        .filter(|rank| *rank >= layout.group.size)
        .collect::<Vec<_>>();
    if !invalid_ranks.is_empty() {
        return Err(TensorParallelLinearError::InvalidRankModules {
            invalid_ranks,
            group_size: layout.group.size,
        });
    }
    let missing_ranks = (0..layout.group.size)
        .filter(|rank| !rank_modules.contains_key(rank))
        .collect::<Vec<_>>();
    if !missing_ranks.is_empty() {
        return Err(TensorParallelLinearError::MissingRankModules { missing_ranks });
    }
    for rank in 0..layout.group.size {
        let Some(module) = rank_modules.get(&rank) else {
            return Err(TensorParallelLinearError::MissingRankModules {
                missing_ranks: vec![rank],
            });
        };
        if module.layout.helper != TensorParallelLinearKind::ShardedToAll {
            return Err(TensorParallelLinearError::RankModuleMismatch {
                rank,
                detail: String::from("wrapper is not a sharded_to_all tensor-parallel module"),
            });
        }
        if module.layout.group.group_id != layout.group.group_id
            || module.layout.group.size != layout.group.size
        {
            return Err(TensorParallelLinearError::RankModuleMismatch {
                rank,
                detail: format!(
                    "expected group `{}` of size {}, found group `{}` of size {}",
                    layout.group.group_id,
                    layout.group.size,
                    module.layout.group.group_id,
                    module.layout.group.size,
                ),
            });
        }
        if module.layout.group.rank != rank {
            return Err(TensorParallelLinearError::RankModuleMismatch {
                rank,
                detail: format!(
                    "wrapper stores local rank {} instead of {rank}",
                    module.layout.group.rank
                ),
            });
        }
        if module.layout.global_in_features != layout.global_in_features
            || module.layout.global_out_features != layout.global_out_features
        {
            return Err(TensorParallelLinearError::RankModuleMismatch {
                rank,
                detail: format!(
                    "expected global shape [{} -> {}], found [{} -> {}]",
                    layout.global_in_features,
                    layout.global_out_features,
                    module.layout.global_in_features,
                    module.layout.global_out_features,
                ),
            });
        }
        let Some((expected_start, expected_end)) =
            partition_feature_axis(layout.global_in_features, rank, layout.group.size)
        else {
            return Err(TensorParallelLinearError::RankModuleMismatch {
                rank,
                detail: String::from("current world cannot shard the input axis honestly"),
            });
        };
        if module.layout.shard_start != expected_start || module.layout.shard_end != expected_end {
            return Err(TensorParallelLinearError::RankModuleMismatch {
                rank,
                detail: format!(
                    "expected shard [{expected_start}, {expected_end}), found [{}, {})",
                    module.layout.shard_start, module.layout.shard_end
                ),
            });
        }
    }
    Ok(())
}

fn validate_sharded_rank_input(
    rank: usize,
    input: &NnTensor,
    expected_prefix: &[usize],
    expected_last_dim: usize,
) -> Result<(), TensorParallelLinearError> {
    let dims = input.dims();
    if dims.is_empty()
        || dims[..dims.len() - 1] != *expected_prefix
        || dims[dims.len() - 1] != expected_last_dim
    {
        return Err(TensorParallelLinearError::RankInputShapeMismatch {
            rank,
            expected_prefix: expected_prefix.to_vec(),
            expected_last_dim,
            actual: dims.to_vec(),
        });
    }
    Ok(())
}

fn distributed_reference_tensor_from_nn(
    tensor: &NnTensor,
) -> Result<DistributedReferenceTensor, TensorParallelLinearError> {
    Ok(DistributedReferenceTensor::f32(
        tensor.spec.shape().clone(),
        tensor.as_f32_slice()?.to_vec(),
    )?)
}

fn array_from_nn_tensor(
    context: &ArrayContext,
    tensor: &NnTensor,
) -> Result<Array, TensorParallelLinearError> {
    Ok(context.constant_f32(tensor.spec.shape().clone(), tensor.as_f32_slice()?.to_vec())?)
}

fn nn_tensor_from_array(array: &Array) -> Result<NnTensor, TensorParallelLinearError> {
    let host = array.to_host_data()?;
    let values = host
        .as_f32_slice()
        .ok_or(TensorParallelLinearError::OutputDTypeMismatch {
            dtype: array.dtype(),
        })?
        .to_vec();
    Ok(NnTensor::f32(array.shape().clone(), values)?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FsdpGroupExecutionPlan {
    kind: FsdpGroupExecutionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FsdpGroupExecutionKind {
    Replicated,
    FullShard {
        shard_ranges: Vec<TrainingShardRange>,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct FsdpLocalUpdate {
    updated_shard_values: Vec<f32>,
    clipped_gradient_norm_l2: f32,
    effective_learning_rate: f32,
    effective_weight_decay: f32,
    scheduler_kind: Option<TrainingSchedulerKind>,
    update_norm_l2: f32,
    transitions: Vec<OptimizerResidencyTransition>,
}

#[derive(Clone, Debug, PartialEq)]
struct ResolvedFsdpOptimizerStep {
    optimizer: TrainingOptimizerConfig,
    effective_learning_rate: f32,
    effective_weight_decay: f32,
    scheduler_kind: Option<TrainingSchedulerKind>,
}

fn validate_remote_rank_map_keys(
    local_rank: usize,
    world_size: usize,
    ranks: Vec<usize>,
    kind: &'static str,
) -> Result<(), FsdpApplyGradientsError> {
    let invalid_ranks = ranks
        .into_iter()
        .filter(|rank| *rank >= world_size || *rank == local_rank)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if invalid_ranks.is_empty() {
        return Ok(());
    }
    Err(FsdpApplyGradientsError::InvalidRemoteRanks {
        kind,
        invalid_ranks,
        world_size,
    })
}

fn local_group_indices(
    parameter_groups: &[TrainingParameterGroupState],
) -> BTreeMap<String, usize> {
    parameter_groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.group_id.clone(), index))
        .collect()
}

fn remote_group_map(
    rank: usize,
    groups: Vec<TrainingParameterGroupState>,
) -> Result<BTreeMap<String, TrainingParameterGroupState>, FsdpApplyGradientsError> {
    let mut map = BTreeMap::new();
    for group in groups {
        let group_id = group.group_id.clone();
        if map.insert(group_id.clone(), group).is_some() {
            return Err(FsdpApplyGradientsError::DuplicateRemoteGroup { rank, group_id });
        }
    }
    Ok(map)
}

fn validate_remote_group_state_contract(
    group_id: &str,
    local_group: &TrainingParameterGroupState,
    remote_rank_group_maps: &BTreeMap<usize, BTreeMap<String, TrainingParameterGroupState>>,
    _world_size: usize,
) -> Result<(), FsdpApplyGradientsError> {
    for (rank, groups) in remote_rank_group_maps {
        let remote_group =
            groups
                .get(group_id)
                .ok_or_else(|| FsdpApplyGradientsError::MissingRemoteGroup {
                    rank: *rank,
                    group_id: group_id.to_string(),
                })?;
        if remote_group.group_id != local_group.group_id {
            return Err(FsdpApplyGradientsError::RemoteStateMismatch {
                rank: *rank,
                group_id: group_id.to_string(),
                detail: format!(
                    "group id drifted from `{}` to `{}`",
                    local_group.group_id, remote_group.group_id
                ),
            });
        }
        if remote_group.class != local_group.class {
            return Err(FsdpApplyGradientsError::RemoteStateMismatch {
                rank: *rank,
                group_id: group_id.to_string(),
                detail: format!(
                    "class drifted from {:?} to {:?}",
                    local_group.class, remote_group.class
                ),
            });
        }
        if remote_group.parameter.spec != local_group.parameter.spec {
            return Err(FsdpApplyGradientsError::RemoteStateMismatch {
                rank: *rank,
                group_id: group_id.to_string(),
                detail: format!(
                    "parameter spec drifted from {:?} to {:?}",
                    local_group.parameter.spec, remote_group.parameter.spec
                ),
            });
        }
        if remote_group.optimizer != local_group.optimizer {
            return Err(FsdpApplyGradientsError::RemoteStateMismatch {
                rank: *rank,
                group_id: group_id.to_string(),
                detail: format!(
                    "optimizer config drifted from {:?} to {:?}",
                    local_group.optimizer, remote_group.optimizer
                ),
            });
        }
        if remote_group.parameter_semantics != local_group.parameter_semantics {
            return Err(FsdpApplyGradientsError::RemoteStateMismatch {
                rank: *rank,
                group_id: group_id.to_string(),
                detail: format!(
                    "parameter semantics drifted from {:?} to {:?}",
                    local_group.parameter_semantics, remote_group.parameter_semantics
                ),
            });
        }
        if scheduler_binding_config(remote_group.scheduler.as_ref())
            != scheduler_binding_config(local_group.scheduler.as_ref())
        {
            return Err(FsdpApplyGradientsError::RemoteStateMismatch {
                rank: *rank,
                group_id: group_id.to_string(),
                detail: String::from("scheduler config drifted"),
            });
        }
        if remote_group.optimizer_residency_policy != local_group.optimizer_residency_policy {
            return Err(FsdpApplyGradientsError::RemoteStateMismatch {
                rank: *rank,
                group_id: group_id.to_string(),
                detail: format!(
                    "optimizer residency policy drifted from {:?} to {:?}",
                    local_group.optimizer_residency_policy, remote_group.optimizer_residency_policy,
                ),
            });
        }
        if remote_group.applied_steps != local_group.applied_steps {
            return Err(FsdpApplyGradientsError::RemoteStateMismatch {
                rank: *rank,
                group_id: group_id.to_string(),
                detail: format!(
                    "applied step count drifted from {} to {}",
                    local_group.applied_steps, remote_group.applied_steps
                ),
            });
        }
        validate_matching_optimizer_state_shapes(group_id, *rank, local_group, remote_group)?;
    }
    Ok(())
}

fn scheduler_binding_config(
    binding: Option<&TrainingSchedulerBinding>,
) -> Option<&psionic_train::TrainingSchedulerConfig> {
    binding.map(|binding| &binding.config)
}

fn validate_matching_optimizer_state_shapes(
    group_id: &str,
    rank: usize,
    local_group: &TrainingParameterGroupState,
    remote_group: &TrainingParameterGroupState,
) -> Result<(), FsdpApplyGradientsError> {
    match (&local_group.optimizer_state, &remote_group.optimizer_state) {
        (
            TrainingOptimizerState::Sgd {
                momentum_buffer: local,
            },
            TrainingOptimizerState::Sgd {
                momentum_buffer: remote,
            },
        )
        | (
            TrainingOptimizerState::Lars {
                momentum_buffer: local,
            },
            TrainingOptimizerState::Lars {
                momentum_buffer: remote,
            },
        ) => {
            if local.as_ref().map(Vec::len) != remote.as_ref().map(Vec::len) {
                return Err(FsdpApplyGradientsError::RemoteStateMismatch {
                    rank,
                    group_id: group_id.to_string(),
                    detail: format!(
                        "momentum-buffer lengths drifted from {:?} to {:?}",
                        local.as_ref().map(Vec::len),
                        remote.as_ref().map(Vec::len)
                    ),
                });
            }
        }
        (
            TrainingOptimizerState::Adam {
                first_moment: local_first,
                second_moment: local_second,
            },
            TrainingOptimizerState::Adam {
                first_moment: remote_first,
                second_moment: remote_second,
            },
        )
        | (
            TrainingOptimizerState::AdamW {
                first_moment: local_first,
                second_moment: local_second,
            },
            TrainingOptimizerState::AdamW {
                first_moment: remote_first,
                second_moment: remote_second,
            },
        )
        | (
            TrainingOptimizerState::Lamb {
                first_moment: local_first,
                second_moment: local_second,
            },
            TrainingOptimizerState::Lamb {
                first_moment: remote_first,
                second_moment: remote_second,
            },
        ) => {
            if local_first.len() != remote_first.len() || local_second.len() != remote_second.len()
            {
                return Err(FsdpApplyGradientsError::RemoteStateMismatch {
                    rank,
                    group_id: group_id.to_string(),
                    detail: format!(
                        "moment lengths drifted from ({}, {}) to ({}, {})",
                        local_first.len(),
                        local_second.len(),
                        remote_first.len(),
                        remote_second.len(),
                    ),
                });
            }
        }
        _ => {
            return Err(FsdpApplyGradientsError::RemoteStateMismatch {
                rank,
                group_id: group_id.to_string(),
                detail: format!(
                    "optimizer state kind drifted from {:?} to {:?}",
                    local_group.optimizer_state.kind(),
                    remote_group.optimizer_state.kind(),
                ),
            });
        }
    }
    Ok(())
}

fn build_fsdp_group_plan(
    group_contract: &DistributedOptimizerGroupContract,
    tensor_elements: usize,
    world_size: usize,
) -> Result<FsdpGroupExecutionPlan, FsdpApplyGradientsError> {
    match (
        group_contract.parameter_layout.kind,
        group_contract.gradient_layout.kind,
        group_contract.optimizer_state_layout.kind,
    ) {
        (
            TrainingParameterShardKind::Replicated,
            TrainingParameterShardKind::Replicated,
            TrainingOptimizerStateShardKind::Replicated,
        ) => {
            validate_replicated_layout(
                group_contract.group_id.as_str(),
                world_size,
                tensor_elements,
                &group_contract.parameter_layout.placements,
            )?;
            validate_replicated_layout(
                group_contract.group_id.as_str(),
                world_size,
                tensor_elements,
                &group_contract.gradient_layout.placements,
            )?;
            validate_replicated_layout(
                group_contract.group_id.as_str(),
                world_size,
                tensor_elements,
                &group_contract.optimizer_state_layout.placements,
            )?;
            Ok(FsdpGroupExecutionPlan {
                kind: FsdpGroupExecutionKind::Replicated,
            })
        }
        (
            TrainingParameterShardKind::FullShard,
            TrainingParameterShardKind::FullShard,
            TrainingOptimizerStateShardKind::ZeroStage3,
        ) => {
            let parameter_ranges = contiguous_equal_shard_ranges(
                group_contract.group_id.as_str(),
                world_size,
                tensor_elements,
                &group_contract.parameter_layout.placements,
            )?;
            let gradient_ranges = contiguous_equal_shard_ranges(
                group_contract.group_id.as_str(),
                world_size,
                tensor_elements,
                &group_contract.gradient_layout.placements,
            )?;
            let optimizer_ranges = contiguous_equal_shard_ranges(
                group_contract.group_id.as_str(),
                world_size,
                tensor_elements,
                &group_contract.optimizer_state_layout.placements,
            )?;
            if parameter_ranges != gradient_ranges || parameter_ranges != optimizer_ranges {
                return Err(FsdpApplyGradientsError::UnevenFullShardLayout {
                    group_id: group_contract.group_id.clone(),
                });
            }
            Ok(FsdpGroupExecutionPlan {
                kind: FsdpGroupExecutionKind::FullShard {
                    shard_ranges: parameter_ranges,
                },
            })
        }
        (parameter_kind, gradient_kind, optimizer_state_kind) => {
            Err(FsdpApplyGradientsError::UnsupportedGroupLayout {
                group_id: group_contract.group_id.clone(),
                parameter_kind,
                gradient_kind,
                optimizer_state_kind,
            })
        }
    }
}

fn validate_replicated_layout(
    group_id: &str,
    world_size: usize,
    tensor_elements: usize,
    placements: &[TrainingShardPlacement],
) -> Result<(), FsdpApplyGradientsError> {
    if placements.len() != world_size {
        return Err(FsdpApplyGradientsError::UnsupportedGroupLayout {
            group_id: group_id.to_string(),
            parameter_kind: TrainingParameterShardKind::Replicated,
            gradient_kind: TrainingParameterShardKind::Replicated,
            optimizer_state_kind: TrainingOptimizerStateShardKind::Replicated,
        });
    }
    let mut ordered = placements.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|placement| placement.shard_id);
    let valid = ordered.iter().enumerate().all(|(rank, placement)| {
        placement.shard_id == rank
            && placement.range.offset_elements == 0
            && placement.range.element_count == tensor_elements
    });
    if valid {
        return Ok(());
    }
    Err(FsdpApplyGradientsError::UnsupportedGroupLayout {
        group_id: group_id.to_string(),
        parameter_kind: TrainingParameterShardKind::Replicated,
        gradient_kind: TrainingParameterShardKind::Replicated,
        optimizer_state_kind: TrainingOptimizerStateShardKind::Replicated,
    })
}

fn contiguous_equal_shard_ranges(
    group_id: &str,
    world_size: usize,
    tensor_elements: usize,
    placements: &[TrainingShardPlacement],
) -> Result<Vec<TrainingShardRange>, FsdpApplyGradientsError> {
    if world_size == 0
        || placements.len() != world_size
        || tensor_elements == 0
        || !tensor_elements.is_multiple_of(world_size)
    {
        return Err(FsdpApplyGradientsError::UnevenFullShardLayout {
            group_id: group_id.to_string(),
        });
    }
    let shard_width = tensor_elements / world_size;
    let mut ordered = placements.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|placement| placement.shard_id);
    let mut shard_ranges = Vec::with_capacity(world_size);
    for (rank, placement) in ordered.into_iter().enumerate() {
        if placement.shard_id != rank
            || placement.range.offset_elements != rank * shard_width
            || placement.range.element_count != shard_width
        {
            return Err(FsdpApplyGradientsError::UnevenFullShardLayout {
                group_id: group_id.to_string(),
            });
        }
        shard_ranges.push(placement.range.clone());
    }
    Ok(shard_ranges)
}

fn validate_training_buffer_compatibility(
    batch_id: &str,
    group_id: &str,
    expected: &psionic_core::TensorSpec,
    actual: &psionic_core::TensorSpec,
) -> Result<(), TrainingCoreError> {
    if expected == actual {
        return Ok(());
    }
    Err(TrainingCoreError::GradientTensorMismatch {
        batch_id: batch_id.to_string(),
        group_id: group_id.to_string(),
        expected: expected.clone(),
        actual: actual.clone(),
    })
}

fn training_buffer_values<'a>(
    group_id: &str,
    buffer: &'a TrainingTensorBuffer,
) -> Result<&'a [f32], TrainingCoreError> {
    match &buffer.data {
        TensorData::F32(values) => {
            let expected_len = buffer.spec.storage_size();
            if values.len() != expected_len {
                return Err(TrainingCoreError::TensorLengthMismatch {
                    group_id: group_id.to_string(),
                    expected_len,
                    actual_len: values.len(),
                });
            }
            Ok(values.as_slice())
        }
        TensorData::QuantizedBlocks(_) => Err(TrainingCoreError::UnsupportedTensorDType {
            group_id: group_id.to_string(),
            dtype: buffer.spec.dtype(),
        }),
    }
}

fn assign_training_buffer_values(
    group_id: &str,
    buffer: &mut TrainingTensorBuffer,
    values: &[f32],
) -> Result<(), TrainingCoreError> {
    let expected_len = buffer.spec.storage_size();
    if values.len() != expected_len {
        return Err(TrainingCoreError::TensorLengthMismatch {
            group_id: group_id.to_string(),
            expected_len,
            actual_len: values.len(),
        });
    }
    match &mut buffer.data {
        TensorData::F32(existing) => {
            *existing = values.to_vec();
            Ok(())
        }
        TensorData::QuantizedBlocks(_) => Err(TrainingCoreError::UnsupportedTensorDType {
            group_id: group_id.to_string(),
            dtype: buffer.spec.dtype(),
        }),
    }
}

fn array_values(array: &Array) -> Result<Vec<f32>, ArrayError> {
    let host = array.to_host_data()?;
    let values = host
        .as_f32_slice()
        .ok_or_else(|| ArrayError::HostInteropRefusal {
            tensor: array.tensor_id(),
            dtype: array.dtype(),
            detail: String::from("expected floating host storage for FSDP reference emulation"),
        })?;
    Ok(values.to_vec())
}

fn l2_norm(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn apply_optional_global_clip(values: &[f32], scale: Option<f32>) -> Vec<f32> {
    match scale {
        Some(scale) => values.iter().map(|value| value * scale).collect(),
        None => values.to_vec(),
    }
}

fn clip_gradient_values(gradients: &[f32], clip_norm: Option<f32>) -> (Vec<f32>, f32) {
    let gradient_norm_l2 = l2_norm(gradients);
    let Some(clip_norm) = clip_norm else {
        return (gradients.to_vec(), gradient_norm_l2);
    };
    if gradient_norm_l2 <= clip_norm || gradient_norm_l2 == 0.0 {
        return (gradients.to_vec(), gradient_norm_l2);
    }
    let scale = clip_norm / gradient_norm_l2;
    (
        gradients.iter().map(|value| value * scale).collect(),
        clip_norm,
    )
}

fn maybe_transition_fsdp_group(
    group: &mut TrainingParameterGroupState,
    target: OptimizerStateResidency,
    global_step: u64,
    reason: OptimizerResidencyTransitionReason,
    transitions: &mut Vec<OptimizerResidencyTransition>,
) {
    if group.optimizer_residency == target {
        return;
    }
    transitions.push(OptimizerResidencyTransition {
        group_id: group.group_id.clone(),
        global_step,
        from: group.optimizer_residency,
        to: target,
        reason,
    });
    group.optimizer_residency = target;
}

fn resolve_fsdp_group_optimizer_step(
    group: &mut TrainingParameterGroupState,
    step_number: u64,
) -> Result<ResolvedFsdpOptimizerStep, TrainingOptimizerError> {
    let mut optimizer = group.optimizer.clone();
    optimizer.learning_rate *= group.parameter_semantics.learning_rate_scale;
    optimizer.weight_decay *= group.parameter_semantics.weight_decay_scale;
    let scheduler_kind = group
        .scheduler
        .as_ref()
        .map(|binding| binding.config.kind());
    if let Some(binding) = &mut group.scheduler {
        optimizer.learning_rate =
            scheduled_learning_rate(binding, optimizer.learning_rate, step_number)?;
    }
    Ok(ResolvedFsdpOptimizerStep {
        effective_learning_rate: optimizer.learning_rate,
        effective_weight_decay: optimizer.weight_decay,
        optimizer,
        scheduler_kind,
    })
}

fn apply_fsdp_local_update(
    group: &mut TrainingParameterGroupState,
    shard_range: &TrainingShardRange,
    gradient_values: &[f32],
) -> Result<FsdpLocalUpdate, FsdpApplyGradientsError> {
    if shard_range.element_count != gradient_values.len() {
        return Err(TrainingOptimizerError::GradientLengthMismatch {
            optimizer: group.optimizer.kind,
            parameter_len: shard_range.element_count,
            gradient_len: gradient_values.len(),
        }
        .into());
    }
    let full_parameter_values =
        training_buffer_values(group.group_id.as_str(), &group.parameter)?.to_vec();
    let shard_end = shard_range
        .offset_elements
        .saturating_add(shard_range.element_count);
    if shard_end > full_parameter_values.len() {
        return Err(TrainingOptimizerError::GradientLengthMismatch {
            optimizer: group.optimizer.kind,
            parameter_len: full_parameter_values.len(),
            gradient_len: shard_end,
        }
        .into());
    }

    let global_step = group.applied_steps.saturating_add(1);
    let mut transitions = Vec::new();
    maybe_transition_fsdp_group(
        group,
        group.optimizer_residency_policy.step_residency,
        global_step,
        OptimizerResidencyTransitionReason::PrefetchForStep,
        &mut transitions,
    );

    let resolved_optimizer = resolve_fsdp_group_optimizer_step(group, global_step)?;
    let (clipped_gradient_values, clipped_gradient_norm_l2) =
        clip_gradient_values(gradient_values, group.optimizer.gradient_clip_norm);
    let mut updated_parameter_values = full_parameter_values;
    let mut shard_parameter_values =
        updated_parameter_values[shard_range.offset_elements..shard_end].to_vec();
    let mut shard_optimizer_state =
        slice_training_optimizer_state(&group.optimizer_state, shard_range)?;
    let optimizer_report = apply_training_optimizer_step(
        &mut shard_parameter_values,
        clipped_gradient_values.as_slice(),
        &resolved_optimizer.optimizer,
        &mut shard_optimizer_state,
        global_step,
    )?;
    updated_parameter_values[shard_range.offset_elements..shard_end]
        .copy_from_slice(shard_parameter_values.as_slice());
    assign_training_buffer_values(
        group.group_id.as_str(),
        &mut group.parameter,
        updated_parameter_values.as_slice(),
    )?;
    assign_training_optimizer_state_shard(
        &mut group.optimizer_state,
        shard_range,
        &shard_optimizer_state,
    )?;
    group.applied_steps = global_step;
    maybe_transition_fsdp_group(
        group,
        group.optimizer_residency_policy.idle_residency,
        global_step,
        OptimizerResidencyTransitionReason::OffloadAfterStep,
        &mut transitions,
    );
    Ok(FsdpLocalUpdate {
        updated_shard_values: shard_parameter_values,
        clipped_gradient_norm_l2,
        effective_learning_rate: optimizer_report.effective_learning_rate,
        effective_weight_decay: optimizer_report.effective_weight_decay,
        scheduler_kind: resolved_optimizer.scheduler_kind,
        update_norm_l2: optimizer_report.update_norm_l2,
        transitions,
    })
}

fn slice_training_optimizer_state(
    state: &TrainingOptimizerState,
    shard_range: &TrainingShardRange,
) -> Result<TrainingOptimizerState, TrainingOptimizerError> {
    match state {
        TrainingOptimizerState::Sgd { momentum_buffer } => Ok(TrainingOptimizerState::Sgd {
            momentum_buffer: slice_optional_optimizer_buffer(
                state.kind(),
                momentum_buffer.as_ref(),
                shard_range,
            )?,
        }),
        TrainingOptimizerState::Adam {
            first_moment,
            second_moment,
        } => Ok(TrainingOptimizerState::Adam {
            first_moment: slice_optimizer_buffer(state.kind(), first_moment, shard_range)?,
            second_moment: slice_optimizer_buffer(state.kind(), second_moment, shard_range)?,
        }),
        TrainingOptimizerState::AdamW {
            first_moment,
            second_moment,
        } => Ok(TrainingOptimizerState::AdamW {
            first_moment: slice_optimizer_buffer(state.kind(), first_moment, shard_range)?,
            second_moment: slice_optimizer_buffer(state.kind(), second_moment, shard_range)?,
        }),
        TrainingOptimizerState::Lars { momentum_buffer } => Ok(TrainingOptimizerState::Lars {
            momentum_buffer: slice_optional_optimizer_buffer(
                state.kind(),
                momentum_buffer.as_ref(),
                shard_range,
            )?,
        }),
        TrainingOptimizerState::Lamb {
            first_moment,
            second_moment,
        } => Ok(TrainingOptimizerState::Lamb {
            first_moment: slice_optimizer_buffer(state.kind(), first_moment, shard_range)?,
            second_moment: slice_optimizer_buffer(state.kind(), second_moment, shard_range)?,
        }),
    }
}

fn assign_training_optimizer_state_shard(
    state: &mut TrainingOptimizerState,
    shard_range: &TrainingShardRange,
    shard_state: &TrainingOptimizerState,
) -> Result<(), TrainingOptimizerError> {
    let optimizer = state.kind();
    match (state, shard_state) {
        (
            TrainingOptimizerState::Sgd {
                momentum_buffer: target,
            },
            TrainingOptimizerState::Sgd {
                momentum_buffer: source,
            },
        )
        | (
            TrainingOptimizerState::Lars {
                momentum_buffer: target,
            },
            TrainingOptimizerState::Lars {
                momentum_buffer: source,
            },
        ) => assign_optional_optimizer_buffer(optimizer, target, source.as_ref(), shard_range),
        (
            TrainingOptimizerState::Adam {
                first_moment: target_first,
                second_moment: target_second,
            },
            TrainingOptimizerState::Adam {
                first_moment: source_first,
                second_moment: source_second,
            },
        )
        | (
            TrainingOptimizerState::AdamW {
                first_moment: target_first,
                second_moment: target_second,
            },
            TrainingOptimizerState::AdamW {
                first_moment: source_first,
                second_moment: source_second,
            },
        )
        | (
            TrainingOptimizerState::Lamb {
                first_moment: target_first,
                second_moment: target_second,
            },
            TrainingOptimizerState::Lamb {
                first_moment: source_first,
                second_moment: source_second,
            },
        ) => {
            assign_optimizer_buffer(optimizer, target_first, source_first, shard_range)?;
            assign_optimizer_buffer(optimizer, target_second, source_second, shard_range)
        }
        (_, _) => Err(TrainingOptimizerError::StateKindMismatch {
            optimizer,
            state_kind: shard_state.kind(),
        }),
    }
}

fn slice_optimizer_buffer(
    optimizer: psionic_train::TrainingOptimizerKind,
    values: &[f32],
    shard_range: &TrainingShardRange,
) -> Result<Vec<f32>, TrainingOptimizerError> {
    let shard_end = shard_range
        .offset_elements
        .saturating_add(shard_range.element_count);
    if shard_end > values.len() {
        return Err(TrainingOptimizerError::StateLengthMismatch {
            optimizer,
            expected_len: shard_end,
            actual_len: values.len(),
        });
    }
    Ok(values[shard_range.offset_elements..shard_end].to_vec())
}

fn slice_optional_optimizer_buffer(
    optimizer: psionic_train::TrainingOptimizerKind,
    values: Option<&Vec<f32>>,
    shard_range: &TrainingShardRange,
) -> Result<Option<Vec<f32>>, TrainingOptimizerError> {
    values
        .map(|values| slice_optimizer_buffer(optimizer, values.as_slice(), shard_range))
        .transpose()
}

fn assign_optimizer_buffer(
    optimizer: psionic_train::TrainingOptimizerKind,
    target: &mut [f32],
    source: &[f32],
    shard_range: &TrainingShardRange,
) -> Result<(), TrainingOptimizerError> {
    if source.len() != shard_range.element_count {
        return Err(TrainingOptimizerError::StateLengthMismatch {
            optimizer,
            expected_len: shard_range.element_count,
            actual_len: source.len(),
        });
    }
    let shard_end = shard_range
        .offset_elements
        .saturating_add(shard_range.element_count);
    if shard_end > target.len() {
        return Err(TrainingOptimizerError::StateLengthMismatch {
            optimizer,
            expected_len: shard_end,
            actual_len: target.len(),
        });
    }
    target[shard_range.offset_elements..shard_end].copy_from_slice(source);
    Ok(())
}

fn assign_optional_optimizer_buffer(
    optimizer: psionic_train::TrainingOptimizerKind,
    target: &mut Option<Vec<f32>>,
    source: Option<&Vec<f32>>,
    shard_range: &TrainingShardRange,
) -> Result<(), TrainingOptimizerError> {
    match (target.as_mut(), source) {
        (Some(target), Some(source)) => assign_optimizer_buffer(
            optimizer,
            target.as_mut_slice(),
            source.as_slice(),
            shard_range,
        ),
        (None, None) => Ok(()),
        (Some(_), None) | (None, Some(_)) => Err(TrainingOptimizerError::StateLengthMismatch {
            optimizer,
            expected_len: shard_range.element_count,
            actual_len: source.map_or(0, Vec::len),
        }),
    }
}

fn stable_fsdp_apply_gradients_receipt_digest(receipt: &FsdpApplyGradientsReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_distributed_fsdp_apply_gradients|");
    hasher.update(receipt.distributed_group_id.as_bytes());
    hasher.update(b"|contract|");
    hasher.update(receipt.contract_digest.as_bytes());
    hasher.update(b"|rank|");
    hasher.update(receipt.local_rank.to_le_bytes());
    hasher.update(b"|world|");
    hasher.update(receipt.world_size.to_le_bytes());
    hasher.update(b"|batch|");
    hasher.update(receipt.batch_id.as_bytes());
    hasher.update(b"|clip|");
    hasher.update(
        receipt
            .clip_global_norm
            .map(f32::to_bits)
            .unwrap_or_default()
            .to_le_bytes(),
    );
    hasher.update(b"|global_norm|");
    hasher.update(receipt.global_gradient_norm_l2.to_bits().to_le_bytes());
    hasher.update(b"|global_scale|");
    hasher.update(
        receipt
            .global_clipping_scale
            .map(f32::to_bits)
            .unwrap_or_default()
            .to_le_bytes(),
    );
    for group in &receipt.groups {
        hasher.update(b"|group|");
        hasher.update(group.group_id.as_bytes());
        hasher.update(b"|parameter_kind|");
        hasher.update(format!("{:?}", group.parameter_shard_kind).as_bytes());
        hasher.update(b"|optimizer_state_kind|");
        hasher.update(format!("{:?}", group.optimizer_state_shard_kind).as_bytes());
        hasher.update(b"|offset|");
        hasher.update(group.local_shard_range.offset_elements.to_le_bytes());
        hasher.update(b"|count|");
        hasher.update(group.local_shard_range.element_count.to_le_bytes());
        hasher.update(b"|reduced_norm|");
        hasher.update(group.reduced_full_gradient_norm_l2.to_bits().to_le_bytes());
        hasher.update(b"|local_norm|");
        hasher.update(group.local_shard_gradient_norm_l2.to_bits().to_le_bytes());
        hasher.update(b"|lr|");
        hasher.update(group.effective_learning_rate.to_bits().to_le_bytes());
        hasher.update(b"|wd|");
        hasher.update(group.effective_weight_decay.to_bits().to_le_bytes());
        hasher.update(b"|scheduler|");
        hasher.update(
            group
                .scheduler_kind
                .map(|kind| format!("{kind:?}"))
                .unwrap_or_default()
                .as_bytes(),
        );
        hasher.update(b"|update_norm|");
        hasher.update(group.local_update_norm_l2.to_bits().to_le_bytes());
        hasher.update(b"|parameter_norm|");
        hasher.update(
            group
                .gathered_parameter_norm_l2_after
                .to_bits()
                .to_le_bytes(),
        );
        for transition in &group.residency_transitions {
            hasher.update(b"|transition|");
            hasher.update(transition.group_id.as_bytes());
            hasher.update(b"|step|");
            hasher.update(transition.global_step.to_le_bytes());
            hasher.update(b"|from|");
            hasher.update(format!("{:?}", transition.from).as_bytes());
            hasher.update(b"|to|");
            hasher.update(format!("{:?}", transition.to).as_bytes());
            hasher.update(b"|reason|");
            hasher.update(format!("{:?}", transition.reason).as_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

fn validate_launch_config(
    cluster_state: &ClusterState,
    sandbox_profile: &ProviderSandboxProfile,
    config: &DistributedLaunchConfig,
) -> Result<(), DistributedLaunchError> {
    if config.execution_class != sandbox_profile.execution_class {
        return Err(DistributedLaunchError::SandboxExecutionClassMismatch {
            requested: config.execution_class,
            profile: sandbox_profile.execution_class,
        });
    }
    if !sandbox_profile.runtime_ready {
        return Err(DistributedLaunchError::SandboxRuntimeNotReady {
            profile_id: sandbox_profile.profile_id.clone(),
            execution_class: config.execution_class,
        });
    }
    if config.timeout_request_s == 0 {
        return Err(DistributedLaunchError::TimeoutRequestZero);
    }
    if config.timeout_request_s > sandbox_profile.timeout_limit_s {
        return Err(DistributedLaunchError::TimeoutExceedsSandboxProfile {
            requested_timeout_s: config.timeout_request_s,
            profile_limit_s: sandbox_profile.timeout_limit_s,
        });
    }
    if config
        .resource_request
        .cpu_limit
        .is_some_and(|cpu_limit| cpu_limit > sandbox_profile.cpu_limit)
    {
        return Err(
            DistributedLaunchError::ResourceRequestExceedsSandboxProfile {
                resource: "cpu_limit",
                requested: u64::from(config.resource_request.cpu_limit.unwrap_or_default()),
                profile_limit: u64::from(sandbox_profile.cpu_limit),
            },
        );
    }
    if config
        .resource_request
        .memory_limit_mb
        .is_some_and(|memory_limit_mb| memory_limit_mb > sandbox_profile.memory_limit_mb)
    {
        return Err(
            DistributedLaunchError::ResourceRequestExceedsSandboxProfile {
                resource: "memory_limit_mb",
                requested: config.resource_request.memory_limit_mb.unwrap_or_default(),
                profile_limit: sandbox_profile.memory_limit_mb,
            },
        );
    }
    if config
        .resource_request
        .disk_limit_mb
        .is_some_and(|disk_limit_mb| disk_limit_mb > sandbox_profile.disk_limit_mb)
    {
        return Err(
            DistributedLaunchError::ResourceRequestExceedsSandboxProfile {
                resource: "disk_limit_mb",
                requested: config.resource_request.disk_limit_mb.unwrap_or_default(),
                profile_limit: sandbox_profile.disk_limit_mb,
            },
        );
    }
    if config.network_request.trim() != sandbox_profile.network_mode.trim() {
        return Err(DistributedLaunchError::NetworkRequestMismatch {
            requested: config.network_request.clone(),
            profile: sandbox_profile.network_mode.clone(),
        });
    }
    if config.filesystem_request.trim() != sandbox_profile.filesystem_mode.trim() {
        return Err(DistributedLaunchError::FilesystemRequestMismatch {
            requested: config.filesystem_request.clone(),
            profile: sandbox_profile.filesystem_mode.clone(),
        });
    }
    if sandbox_profile.secrets_mode.trim() == "none" {
        return Err(
            DistributedLaunchError::SandboxEnvironmentInjectionForbidden {
                profile_id: sandbox_profile.profile_id.clone(),
            },
        );
    }
    if let Some(compute_product_id) = &config.compute_product_id {
        let expected_product_id = config.execution_class.product_id();
        if compute_product_id != expected_product_id {
            return Err(DistributedLaunchError::ComputeProductIdMismatch {
                compute_product_id: compute_product_id.clone(),
                expected_product_id: expected_product_id.to_string(),
            });
        }
    }
    match config.entrypoint_type {
        ProviderSandboxEntrypointType::InlinePayload => {
            if config.payload.is_none() {
                return Err(DistributedLaunchError::InlinePayloadMissing);
            }
        }
        ProviderSandboxEntrypointType::WorkspaceFile => {
            if !is_relative_workspace_path(config.entrypoint.as_str()) {
                return Err(DistributedLaunchError::InvalidWorkspaceEntrypoint {
                    entrypoint: config.entrypoint.clone(),
                });
            }
        }
        ProviderSandboxEntrypointType::Command => {
            if config.execution_class != ProviderSandboxExecutionClass::ContainerExec {
                return Err(DistributedLaunchError::CommandEntrypointUnsupported {
                    execution_class: config.execution_class,
                });
            }
        }
    }
    for expected_output in &config.expected_outputs {
        if !is_relative_workspace_path(expected_output) {
            return Err(DistributedLaunchError::InvalidExpectedOutput {
                path: expected_output.clone(),
            });
        }
    }
    if config.hostfile_entries.is_empty() {
        return Err(DistributedLaunchError::HostfileEmpty);
    }
    let axis_product = if config.axes.is_empty() {
        config.hostfile_entries.len()
    } else {
        config
            .axes
            .iter()
            .map(|axis| axis.extent)
            .product::<usize>()
    };
    if axis_product != config.hostfile_entries.len() {
        return Err(DistributedLaunchError::AxisProductMismatch {
            axis_product,
            world_size: config.hostfile_entries.len(),
        });
    }
    if !cluster_state
        .memberships()
        .values()
        .any(|record| record.identity.node_id.as_str() == config.scheduler_node_id)
    {
        return Err(DistributedLaunchError::SchedulerNodeMissing {
            node_id: config.scheduler_node_id.clone(),
        });
    }
    for environment in &config.environment {
        if RESERVED_DISTRIBUTED_ENV_KEYS.contains(&environment.key.as_str()) {
            return Err(DistributedLaunchError::ReservedEnvironmentOverride {
                key: environment.key.clone(),
            });
        }
    }

    let mut seen_node_ids = BTreeSet::new();
    for entry in &config.hostfile_entries {
        if !seen_node_ids.insert(entry.node_id.clone()) {
            return Err(DistributedLaunchError::DuplicateHostfileNodeId {
                node_id: entry.node_id.clone(),
            });
        }
        if entry.slots != 1 {
            return Err(DistributedLaunchError::MultiSlotHostfileEntryUnsupported {
                node_id: entry.node_id.clone(),
                slots: entry.slots,
            });
        }
        let membership = cluster_membership_record(cluster_state, entry.node_id.as_str())?;
        if membership.status != ClusterMembershipStatus::Ready {
            return Err(DistributedLaunchError::ClusterNodeNotReady {
                node_id: entry.node_id.clone(),
                status: membership.status,
            });
        }
        if let Some(hostfile_addr) = &entry.advertised_addr {
            let cluster_addr = membership
                .advertised_addr
                .map(|addr| addr.to_string())
                .unwrap_or_else(|| String::from("<none>"));
            if *hostfile_addr != cluster_addr {
                return Err(DistributedLaunchError::HostfileAddressMismatch {
                    node_id: entry.node_id.clone(),
                    hostfile_addr: hostfile_addr.clone(),
                    cluster_addr,
                });
            }
        }
        let telemetry = cluster_node_telemetry(cluster_state, entry.node_id.as_str())?;
        let status = telemetry
            .backend_readiness
            .get(config.effective_backend.as_str())
            .copied()
            .unwrap_or(ClusterBackendReadinessStatus::Unknown);
        if status != ClusterBackendReadinessStatus::Ready {
            return Err(DistributedLaunchError::BackendNotReady {
                node_id: entry.node_id.clone(),
                backend: config.effective_backend.clone(),
                status,
            });
        }
    }
    Ok(())
}

fn cluster_membership_record<'a>(
    cluster_state: &'a ClusterState,
    node_id: &str,
) -> Result<&'a ClusterMembershipRecord, DistributedLaunchError> {
    cluster_state
        .memberships()
        .values()
        .find(|record| record.identity.node_id.as_str() == node_id)
        .ok_or_else(|| DistributedLaunchError::ClusterNodeMissing {
            node_id: node_id.to_string(),
        })
}

fn cluster_node_telemetry<'a>(
    cluster_state: &'a ClusterState,
    node_id: &str,
) -> Result<&'a ClusterNodeTelemetry, DistributedLaunchError> {
    cluster_state
        .telemetry()
        .iter()
        .find(|(telemetry_node_id, _)| telemetry_node_id.as_str() == node_id)
        .map(|(_, telemetry)| telemetry)
        .ok_or_else(|| DistributedLaunchError::ClusterTelemetryMissing {
            node_id: node_id.to_string(),
        })
}

fn cluster_commit_authority_evidence(
    cluster_state: &ClusterState,
) -> Option<ClusterCommitAuthorityEvidence> {
    cluster_state.commit_authority().map(|authority| {
        ClusterCommitAuthorityEvidence::new(
            authority.leader_id.as_str(),
            authority.term.as_u64(),
            authority.committed_event_index.as_u64(),
            authority.fence_token,
            authority.authority_digest,
        )
    })
}

fn launch_environment(
    base_environment: &[ProviderSandboxEnvironmentVar],
    facts: &DistributedLaunchEnvironmentFacts<'_>,
) -> Vec<ProviderSandboxEnvironmentVar> {
    let mut environment = base_environment.to_vec();
    let distributed = [
        sandbox_environment_var("PSIONIC_DISTRIBUTED_RANK", facts.rank.to_string()),
        sandbox_environment_var(
            "PSIONIC_DISTRIBUTED_LOCAL_RANK",
            facts.local_rank.to_string(),
        ),
        sandbox_environment_var(
            "PSIONIC_DISTRIBUTED_WORLD_SIZE",
            facts.world_size.to_string(),
        ),
        sandbox_environment_var("PSIONIC_DISTRIBUTED_NODE_ID", facts.node_id.to_string()),
        sandbox_environment_var(
            "PSIONIC_DISTRIBUTED_CLUSTER_ID",
            facts.cluster_id.to_string(),
        ),
        sandbox_environment_var(
            "PSIONIC_DISTRIBUTED_CLUSTER_STATE_DIGEST",
            facts.cluster_state_digest.to_string(),
        ),
        sandbox_environment_var(
            "PSIONIC_DISTRIBUTED_TOPOLOGY_DIGEST",
            facts.topology_digest.to_string(),
        ),
        sandbox_environment_var(
            "PSIONIC_DISTRIBUTED_MESH_ID",
            facts.device_mesh.mesh_id.clone(),
        ),
        sandbox_environment_var(
            "PSIONIC_DISTRIBUTED_MESH_REVISION",
            facts.device_mesh.mesh_revision.to_string(),
        ),
        sandbox_environment_var(
            "PSIONIC_DISTRIBUTED_EFFECTIVE_BACKEND",
            facts.effective_backend.to_string(),
        ),
        sandbox_environment_var("PSIONIC_DISTRIBUTED_GROUP_ID", facts.group_id.to_string()),
    ];
    environment.extend(distributed);
    environment
}

fn sandbox_environment_var(
    key: impl Into<String>,
    value: impl Into<String>,
) -> ProviderSandboxEnvironmentVar {
    ProviderSandboxEnvironmentVar {
        key: key.into(),
        value: value.into(),
    }
}

fn is_relative_workspace_path(path: &str) -> bool {
    let candidate = Path::new(path);
    !path.trim().is_empty()
        && !candidate.is_absolute()
        && !candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir))
}

fn stable_launch_plan_digest(
    config: &DistributedLaunchConfig,
    facts: &DistributedLaunchDigestFacts<'_>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_distributed_launch_plan|");
    hasher.update(config.launch_id.as_bytes());
    hasher.update(b"|scheduler|");
    hasher.update(config.scheduler_node_id.as_bytes());
    hasher.update(b"|backend|");
    hasher.update(config.requested_backend.as_str().as_bytes());
    hasher.update(b"|effective_backend|");
    hasher.update(config.effective_backend.as_bytes());
    hasher.update(b"|cluster_id|");
    hasher.update(facts.cluster_id.as_bytes());
    hasher.update(b"|cluster_state|");
    hasher.update(facts.cluster_state_digest.as_bytes());
    hasher.update(b"|topology|");
    hasher.update(facts.topology_digest.as_bytes());
    hasher.update(b"|transport|");
    hasher.update(format!("{:?}", config.transport).as_bytes());
    hasher.update(b"|execution_class|");
    hasher.update(format!("{:?}", config.execution_class).as_bytes());
    hasher.update(b"|entrypoint_type|");
    hasher.update(format!("{:?}", config.entrypoint_type).as_bytes());
    hasher.update(b"|entrypoint|");
    hasher.update(config.entrypoint.as_bytes());
    hasher.update(b"|payload|");
    hasher.update(config.payload.as_deref().unwrap_or_default().as_bytes());
    hasher.update(b"|workspace_root|");
    hasher.update(
        config
            .workspace_root
            .as_os_str()
            .to_string_lossy()
            .as_bytes(),
    );
    hasher.update(b"|timeout|");
    hasher.update(config.timeout_request_s.to_le_bytes());
    hasher.update(b"|network_request|");
    hasher.update(config.network_request.as_bytes());
    hasher.update(b"|filesystem_request|");
    hasher.update(config.filesystem_request.as_bytes());
    hasher.update(b"|provider_id|");
    hasher.update(config.provider_id.as_bytes());
    hasher.update(b"|compute_product|");
    hasher.update(
        config
            .compute_product_id
            .as_deref()
            .unwrap_or(config.execution_class.product_id())
            .as_bytes(),
    );
    hasher.update(b"|payout_reference|");
    hasher.update(
        config
            .payout_reference
            .as_deref()
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(b"|verification_posture|");
    hasher.update(
        config
            .verification_posture
            .as_deref()
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(b"|mesh|");
    hasher.update(facts.device_mesh.mesh_id.as_bytes());
    hasher.update(b"|mesh_revision|");
    hasher.update(facts.device_mesh.mesh_revision.to_le_bytes());
    hasher.update(b"|group_id|");
    hasher.update(facts.group_id.as_bytes());
    hasher.update(b"|elastic_membership_epoch|");
    hasher.update(facts.elastic_membership.membership_epoch.to_le_bytes());
    hasher.update(b"|elastic_cluster_state|");
    hasher.update(facts.elastic_membership.cluster_state_digest.as_bytes());
    hasher.update(b"|elastic_topology|");
    hasher.update(facts.elastic_membership.topology_digest.as_bytes());
    for node_id in &facts.elastic_membership.active_node_ids {
        hasher.update(b"|elastic_node|");
        hasher.update(node_id.as_bytes());
    }
    for axis in &facts.device_mesh.axes {
        hasher.update(b"|axis|");
        hasher.update(axis.axis_id.as_bytes());
        hasher.update(b"|kind|");
        hasher.update(format!("{:?}", axis.kind).as_bytes());
        hasher.update(b"|extent|");
        hasher.update(axis.extent.to_le_bytes());
    }
    for argument in &config.arguments {
        hasher.update(b"|arg|");
        hasher.update(argument.as_bytes());
    }
    for path in &config.expected_outputs {
        hasher.update(b"|output|");
        hasher.update(path.as_bytes());
    }
    for environment in &config.environment {
        hasher.update(b"|base_env|");
        hasher.update(environment.key.as_bytes());
        hasher.update(b"|");
        hasher.update(environment.value.as_bytes());
    }
    if let Some(cpu_limit) = config.resource_request.cpu_limit {
        hasher.update(b"|cpu_limit|");
        hasher.update(cpu_limit.to_le_bytes());
    }
    if let Some(memory_limit_mb) = config.resource_request.memory_limit_mb {
        hasher.update(b"|memory_limit_mb|");
        hasher.update(memory_limit_mb.to_le_bytes());
    }
    if let Some(disk_limit_mb) = config.resource_request.disk_limit_mb {
        hasher.update(b"|disk_limit_mb|");
        hasher.update(disk_limit_mb.to_le_bytes());
    }
    for entry in &config.hostfile_entries {
        hasher.update(b"|host|");
        hasher.update(entry.node_id.as_bytes());
        hasher.update(b"|slots|");
        hasher.update(entry.slots.to_le_bytes());
        if let Some(advertised_addr) = &entry.advertised_addr {
            hasher.update(b"|addr|");
            hasher.update(advertised_addr.as_bytes());
        }
    }
    for member in facts.members {
        hasher.update(b"|member|");
        hasher.update(member.node_id.as_bytes());
        hasher.update(b"|rank|");
        hasher.update(member.rank.to_le_bytes());
        hasher.update(b"|shard|");
        hasher.update(member.shard_id.to_le_bytes());
        hasher.update(b"|device|");
        hasher.update(member.device_label.as_bytes());
    }
    hasher.update(b"|cluster_execution_scheduler|");
    hasher.update(facts.cluster_execution.scheduler_node_id.as_bytes());
    hasher.update(b"|cluster_execution_disposition|");
    hasher.update(format!("{:?}", facts.cluster_execution.disposition).as_bytes());
    if let Some(commit_authority) = &facts.cluster_execution.commit_authority {
        hasher.update(b"|commit_authority|");
        hasher.update(commit_authority.coordinator_node_id.as_bytes());
        hasher.update(b"|term|");
        hasher.update(commit_authority.term.to_le_bytes());
        hasher.update(b"|committed_index|");
        hasher.update(commit_authority.committed_event_index.to_le_bytes());
        hasher.update(b"|fence|");
        hasher.update(commit_authority.fence_token.as_bytes());
        hasher.update(b"|authority_digest|");
        hasher.update(commit_authority.authority_digest.as_bytes());
    }
    for node in &facts.cluster_execution.selected_nodes {
        hasher.update(b"|selected_node|");
        hasher.update(node.node_id.as_bytes());
        hasher.update(b"|");
        hasher.update(node.runtime_backend.as_bytes());
    }
    for diagnostic in &facts.cluster_execution.placement_diagnostics {
        hasher.update(b"|placement|");
        hasher.update(diagnostic.as_bytes());
    }
    for assignment in facts.assignments {
        hasher.update(b"|assignment|");
        hasher.update(assignment.node_id.as_bytes());
        hasher.update(b"|rank|");
        hasher.update(assignment.rank.to_le_bytes());
        hasher.update(b"|local_rank|");
        hasher.update(assignment.local_rank.to_le_bytes());
        hasher.update(b"|job_id|");
        hasher.update(assignment.sandbox_job.job_id.as_bytes());
        hasher.update(b"|sandbox_provider|");
        hasher.update(assignment.sandbox_job.provider_id.as_bytes());
        hasher.update(b"|sandbox_product|");
        hasher.update(assignment.sandbox_job.compute_product_id.as_bytes());
        hasher.update(b"|sandbox_timeout|");
        hasher.update(assignment.sandbox_job.timeout_request_s.to_le_bytes());
        hasher.update(b"|sandbox_entrypoint|");
        hasher.update(assignment.sandbox_job.entrypoint.as_bytes());
        for environment in &assignment.environment {
            hasher.update(b"|assignment_env|");
            hasher.update(environment.key.as_bytes());
            hasher.update(b"|");
            hasher.update(environment.value.as_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

fn validated_rank_gradient_trees(
    group: &DistributedGroup,
    local_spec: &TreeSpec,
    local_leaves: &[Array],
    rank_inputs: &BTreeMap<usize, Tree<DistributedReferenceTensor>>,
) -> Result<BTreeMap<usize, Vec<DistributedReferenceTensor>>, DistributedGradientReductionError> {
    let invalid_ranks = rank_inputs
        .keys()
        .copied()
        .filter(|rank| *rank >= group.size())
        .collect::<Vec<_>>();
    if !invalid_ranks.is_empty() {
        return Err(DistributedCollectiveError::InvalidRankInputs {
            operation: DistributedCollectiveOperation::AllSum,
            invalid_ranks,
            group_size: group.size(),
        }
        .into());
    }
    let missing_ranks = (0..group.size())
        .filter(|rank| !rank_inputs.contains_key(rank))
        .collect::<Vec<_>>();
    if !missing_ranks.is_empty() {
        return Err(DistributedCollectiveError::MissingRankInputs {
            operation: DistributedCollectiveOperation::AllSum,
            missing_ranks,
        }
        .into());
    }

    let mut validated = BTreeMap::new();
    for (rank, tree) in rank_inputs {
        let actual = tree.spec();
        if actual != *local_spec {
            return Err(
                DistributedGradientReductionError::RankInputTreeStructureMismatch {
                    rank: *rank,
                    expected: local_spec.clone(),
                    actual,
                },
            );
        }
        let leaves = tree.leaves();
        for (local_leaf, remote_leaf) in local_leaves.iter().zip(leaves.iter()) {
            validate_rank_payload_shape_dtype(
                DistributedCollectiveOperation::AllSum,
                *rank,
                local_leaf.shape(),
                local_leaf.dtype(),
                remote_leaf,
            )?;
            if *rank == group.rank()
                && *remote_leaf != DistributedReferenceTensor::from_array(local_leaf)?
            {
                return Err(DistributedCollectiveError::LocalRankInputMismatch {
                    operation: DistributedCollectiveOperation::AllSum,
                    rank: group.rank(),
                }
                .into());
            }
        }
        validated.insert(*rank, leaves);
    }
    Ok(validated)
}

fn gradient_leaf_groups(
    local_leaves: &[Array],
    small_tensor_bytes_threshold: usize,
) -> Vec<GradientLeafGroup> {
    let mut groups = Vec::new();
    let mut current_indices = Vec::<usize>::new();
    let mut current_dtype = None::<DType>;
    let mut current_bytes = 0usize;

    for (index, leaf) in local_leaves.iter().enumerate() {
        let leaf_bytes = leaf
            .shape()
            .element_count()
            .saturating_mul(leaf.dtype().element_size_bytes());
        let eligible =
            small_tensor_bytes_threshold > 0 && leaf_bytes <= small_tensor_bytes_threshold;
        let can_extend = eligible
            && !current_indices.is_empty()
            && current_dtype == Some(leaf.dtype())
            && current_bytes.saturating_add(leaf_bytes) <= small_tensor_bytes_threshold;
        if can_extend {
            current_indices.push(index);
            current_bytes = current_bytes.saturating_add(leaf_bytes);
            continue;
        }
        if !current_indices.is_empty() {
            groups.push(GradientLeafGroup {
                leaf_indices: std::mem::take(&mut current_indices),
            });
        }
        if eligible {
            current_indices.push(index);
            current_dtype = Some(leaf.dtype());
            current_bytes = leaf_bytes;
        } else {
            groups.push(GradientLeafGroup {
                leaf_indices: vec![index],
            });
            current_dtype = None;
            current_bytes = 0;
        }
    }
    if !current_indices.is_empty() {
        groups.push(GradientLeafGroup {
            leaf_indices: current_indices,
        });
    }
    groups
}

fn concatenate_reference_tensors(
    tensors: &[DistributedReferenceTensor],
) -> Result<DistributedReferenceTensor, DistributedGradientReductionError> {
    let Some(first) = tensors.first() else {
        return Err(DistributedCollectiveError::ReferenceElementCountMismatch {
            shape: Shape::new(vec![0]),
            expected_elements: 1,
            actual_elements: 0,
        }
        .into());
    };
    let dtype = first.dtype();
    let element_count = tensors
        .iter()
        .map(|tensor| tensor.shape().element_count())
        .sum::<usize>();
    let shape = Shape::new(vec![element_count]);
    match dtype {
        DType::F32 | DType::F16 | DType::BF16 => {
            let values = tensors
                .iter()
                .flat_map(DistributedReferenceTensor::as_f32_values)
                .collect::<Vec<_>>();
            Ok(DistributedReferenceTensor::float(shape, dtype, values)?)
        }
        DType::I8 => {
            let mut values = Vec::with_capacity(element_count);
            for tensor in tensors {
                let Some(local) = tensor.as_i8_values() else {
                    return Err(DistributedCollectiveError::RankInputMismatch {
                        operation: DistributedCollectiveOperation::AllSum,
                        rank: 0,
                        expected_shape: first.shape().clone(),
                        actual_shape: tensor.shape().clone(),
                        expected_dtype: dtype,
                        actual_dtype: tensor.dtype(),
                    }
                    .into());
                };
                values.extend(local);
            }
            Ok(DistributedReferenceTensor::i8(shape, values)?)
        }
    }
}

fn split_reduced_group(
    reduced_group: &DistributedReferenceTensor,
    local_leaves: &[Array],
    group: &GradientLeafGroup,
) -> Result<Vec<Array>, DistributedGradientReductionError> {
    let mut rebuilt = Vec::with_capacity(group.leaf_indices.len());
    match reduced_group.dtype() {
        DType::F32 | DType::F16 | DType::BF16 => {
            let values = reduced_group.as_f32_values();
            let mut cursor = 0usize;
            for leaf_index in &group.leaf_indices {
                let leaf = &local_leaves[*leaf_index];
                let element_count = leaf.shape().element_count();
                let next_cursor = cursor.saturating_add(element_count);
                let payload = values[cursor..next_cursor].to_vec();
                cursor = next_cursor;
                rebuilt.push(
                    DistributedReferenceTensor::float(leaf.shape().clone(), leaf.dtype(), payload)?
                        .to_array(&leaf.context())?,
                );
            }
        }
        DType::I8 => {
            let values = reduced_group.as_i8_values().unwrap_or_default();
            let mut cursor = 0usize;
            for leaf_index in &group.leaf_indices {
                let leaf = &local_leaves[*leaf_index];
                let element_count = leaf.shape().element_count();
                let next_cursor = cursor.saturating_add(element_count);
                let payload = values[cursor..next_cursor].to_vec();
                cursor = next_cursor;
                rebuilt.push(
                    DistributedReferenceTensor::i8(leaf.shape().clone(), payload)?
                        .to_array(&leaf.context())?,
                );
            }
        }
    }
    Ok(rebuilt)
}

fn scale_gradient_leaf(
    leaf: &Array,
    factor: f32,
) -> Result<Array, DistributedGradientReductionError> {
    let context = leaf.context();
    let scalar = context
        .scalar_f32(factor)
        .map_err(DistributedCollectiveError::from)?
        .cast(leaf.dtype())
        .map_err(DistributedCollectiveError::from)?;
    let scale = scalar
        .broadcast_to(leaf.shape().clone())
        .map_err(DistributedCollectiveError::from)?;
    leaf.mul(&scale)
        .map_err(DistributedCollectiveError::from)
        .map_err(Into::into)
}

fn validate_reference_element_count(
    shape: &Shape,
    actual_elements: usize,
) -> Result<(), DistributedCollectiveError> {
    let expected_elements = shape.element_count();
    if actual_elements != expected_elements {
        return Err(DistributedCollectiveError::ReferenceElementCountMismatch {
            shape: shape.clone(),
            expected_elements,
            actual_elements,
        });
    }
    Ok(())
}

fn ordered_rank_inputs(
    operation: DistributedCollectiveOperation,
    x: &Array,
    group: &DistributedGroup,
    rank_inputs: &BTreeMap<usize, DistributedReferenceTensor>,
) -> Result<Vec<DistributedReferenceTensor>, DistributedCollectiveError> {
    let invalid_ranks = rank_inputs
        .keys()
        .copied()
        .filter(|rank| *rank >= group.size())
        .collect::<Vec<_>>();
    if !invalid_ranks.is_empty() {
        return Err(DistributedCollectiveError::InvalidRankInputs {
            operation,
            invalid_ranks,
            group_size: group.size(),
        });
    }

    let missing_ranks = (0..group.size())
        .filter(|rank| !rank_inputs.contains_key(rank))
        .collect::<Vec<_>>();
    if !missing_ranks.is_empty() {
        return Err(DistributedCollectiveError::MissingRankInputs {
            operation,
            missing_ranks,
        });
    }

    let local_payload = DistributedReferenceTensor::from_array(x)?;
    let mut ordered = Vec::with_capacity(group.size());
    for rank in 0..group.size() {
        let Some(payload) = rank_inputs.get(&rank).cloned() else {
            return Err(DistributedCollectiveError::MissingRankInputs {
                operation,
                missing_ranks: vec![rank],
            });
        };
        validate_rank_payload_shape_dtype(operation, rank, x.shape(), x.dtype(), &payload)?;
        if rank == group.rank() && payload != local_payload {
            return Err(DistributedCollectiveError::LocalRankInputMismatch { operation, rank });
        }
        ordered.push(payload);
    }
    Ok(ordered)
}

fn validate_rank_payload_shape_dtype(
    operation: DistributedCollectiveOperation,
    rank: usize,
    expected_shape: &Shape,
    expected_dtype: DType,
    payload: &DistributedReferenceTensor,
) -> Result<(), DistributedCollectiveError> {
    if payload.shape() != expected_shape || payload.dtype() != expected_dtype {
        return Err(DistributedCollectiveError::RankInputMismatch {
            operation,
            rank,
            expected_shape: expected_shape.clone(),
            actual_shape: payload.shape().clone(),
            expected_dtype,
            actual_dtype: payload.dtype(),
        });
    }
    Ok(())
}

fn build_output_array(
    context: &ArrayContext,
    shape: Shape,
    dtype: DType,
    values: Vec<f32>,
) -> Result<Array, DistributedCollectiveError> {
    DistributedReferenceTensor::float(shape, dtype, values)?.to_array(context)
}

fn gathered_shape(input_shape: &Shape, group_size: usize) -> Shape {
    if input_shape.rank() == 0 {
        return Shape::new(vec![group_size]);
    }
    let mut dims = input_shape.dims().to_vec();
    dims[0] *= group_size;
    Shape::new(dims)
}

fn reduce_scatter_values(
    input_shape: &Shape,
    group_size: usize,
    rank: usize,
    reduced: Vec<f32>,
) -> Result<(Shape, Vec<f32>), DistributedCollectiveError> {
    if input_shape.rank() == 0 {
        return Err(DistributedCollectiveError::ReduceScatterScalarInput);
    }
    let first_axis = input_shape.dims()[0];
    if !first_axis.is_multiple_of(group_size) {
        return Err(DistributedCollectiveError::ReduceScatterNonDivisibleAxis0 {
            first_axis,
            group_size,
        });
    }
    let chunk_axis = first_axis / group_size;
    let trailing = input_shape.dims()[1..].iter().product::<usize>().max(1);
    let chunk_elements = chunk_axis * trailing;
    let start = rank * chunk_elements;
    let end = start + chunk_elements;
    let mut dims = input_shape.dims().to_vec();
    dims[0] = chunk_axis;
    Ok((Shape::new(dims), reduced[start..end].to_vec()))
}

fn validate_destination(
    destination: usize,
    group_size: usize,
) -> Result<(), DistributedCollectiveError> {
    if destination >= group_size {
        return Err(DistributedCollectiveError::InvalidDestination {
            destination,
            group_size,
        });
    }
    Ok(())
}

fn validate_source(source: usize, group_size: usize) -> Result<(), DistributedCollectiveError> {
    if source >= group_size {
        return Err(DistributedCollectiveError::InvalidSource {
            source_rank: source,
            group_size,
        });
    }
    Ok(())
}

fn validate_point_to_point_payload_ranks(
    operation: DistributedCollectiveOperation,
    group_size: usize,
    payloads: &BTreeMap<usize, DistributedReferenceTensor>,
) -> Result<(), DistributedCollectiveError> {
    let invalid_ranks = payloads
        .keys()
        .copied()
        .filter(|rank| *rank >= group_size)
        .collect::<Vec<_>>();
    if !invalid_ranks.is_empty() {
        return Err(DistributedCollectiveError::InvalidRankInputs {
            operation,
            invalid_ranks,
            group_size,
        });
    }
    Ok(())
}

fn build_bootstrapped_state(
    requested_backend: DistributedBackend,
    bootstrap: DistributedGroupBootstrap,
) -> Result<DistributedGroupState, DistributedInitError> {
    let (mesh, transport, members, local_rank, local_node_id) = validate_bootstrap(bootstrap)?;
    validate_requested_backend_mapping(
        DistributedGroupKind::BootstrappedMesh,
        requested_backend,
        mesh.effective_backend.as_str(),
        mesh.communication_class,
        transport,
        members.len(),
    )?;
    Ok(DistributedGroupState {
        group_id: stable_group_id(
            DistributedGroupKind::BootstrappedMesh,
            requested_backend,
            &mesh,
            transport,
            &members,
            None,
        ),
        kind: DistributedGroupKind::BootstrappedMesh,
        requested_backend,
        mesh,
        transport,
        local_node_id,
        members,
        local_rank,
        parent_group_id: None,
    })
}

fn validate_bootstrap(
    bootstrap: DistributedGroupBootstrap,
) -> Result<
    (
        TrainingDeviceMeshContext,
        ClusterTransportClass,
        Vec<DistributedGroupMember>,
        usize,
        String,
    ),
    DistributedInitError,
> {
    if bootstrap.members.is_empty() {
        return Err(DistributedInitError::BootstrapMembersEmpty);
    }

    let mut seen_node_ids = BTreeSet::new();
    let mut seen_ranks = BTreeSet::new();
    for member in &bootstrap.members {
        if !seen_node_ids.insert(member.node_id.clone()) {
            return Err(DistributedInitError::DuplicateMemberNodeId {
                node_id: member.node_id.clone(),
            });
        }
        if !seen_ranks.insert(member.rank) {
            return Err(DistributedInitError::DuplicateMemberRank { rank: member.rank });
        }
    }

    let expected_ranks = (0..bootstrap.members.len()).collect::<Vec<_>>();
    let actual_ranks = seen_ranks.into_iter().collect::<Vec<_>>();
    if actual_ranks != expected_ranks {
        return Err(DistributedInitError::NonContiguousRanks {
            actual: actual_ranks,
            size: bootstrap.members.len(),
        });
    }

    let mesh_members = sorted_distinct_strings(bootstrap.mesh.member_node_ids.clone());
    let group_members = sorted_distinct_strings(
        bootstrap
            .members
            .iter()
            .map(|member| member.node_id.clone())
            .collect(),
    );
    if mesh_members != group_members {
        return Err(DistributedInitError::MeshMemberMismatch {
            mesh_members,
            group_members,
        });
    }

    let mut members = bootstrap.members;
    members.sort_by_key(|member| member.rank);
    let local_rank = members
        .iter()
        .position(|member| member.node_id == bootstrap.local_node_id)
        .ok_or_else(|| DistributedInitError::LocalNodeMissing {
            local_node_id: bootstrap.local_node_id.clone(),
        })?;
    Ok((
        bootstrap.mesh,
        bootstrap.transport,
        members,
        local_rank,
        bootstrap.local_node_id,
    ))
}

fn split_assignment_map<'a>(
    plan: &'a DistributedSplitPlan,
    members: &[DistributedGroupMember],
) -> Result<BTreeMap<&'a str, &'a DistributedSplitAssignment>, DistributedGroupError> {
    let member_node_ids = members
        .iter()
        .map(|member| member.node_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut assignments = BTreeMap::new();
    for assignment in &plan.assignments {
        if !member_node_ids.contains(assignment.node_id.as_str()) {
            return Err(DistributedGroupError::UnknownSplitNode {
                node_id: assignment.node_id.clone(),
            });
        }
        if assignments
            .insert(assignment.node_id.as_str(), assignment)
            .is_some()
        {
            return Err(DistributedGroupError::DuplicateSplitAssignment {
                node_id: assignment.node_id.clone(),
            });
        }
    }

    let missing_node_ids = members
        .iter()
        .filter(|member| !assignments.contains_key(member.node_id.as_str()))
        .map(|member| member.node_id.clone())
        .collect::<Vec<_>>();
    if !missing_node_ids.is_empty() {
        return Err(DistributedGroupError::MissingSplitAssignments { missing_node_ids });
    }
    Ok(assignments)
}

fn build_subgroup_mesh(
    parent_mesh: &TrainingDeviceMeshContext,
    color: i32,
    subgroup_members: &[DistributedGroupMember],
) -> TrainingDeviceMeshContext {
    let member_node_ids = subgroup_members
        .iter()
        .map(|member| member.node_id.clone())
        .collect::<Vec<_>>();
    let axes = if subgroup_members.len() == parent_mesh.member_node_ids.len() {
        parent_mesh.axes.clone()
    } else {
        Vec::new()
    };
    TrainingDeviceMeshContext::new(
        format!("{}.split.{}", parent_mesh.mesh_id, color),
        parent_mesh.mesh_revision,
        parent_mesh.effective_backend.clone(),
        parent_mesh.communication_class,
        parent_mesh.elastic_membership.clone(),
        member_node_ids,
    )
    .with_axes(axes)
}

fn effective_split_key(key: Option<i64>, fallback_rank: usize) -> i64 {
    key.unwrap_or(i64::try_from(fallback_rank).unwrap_or(i64::MAX))
}

fn stable_group_id(
    kind: DistributedGroupKind,
    requested_backend: DistributedBackend,
    mesh: &TrainingDeviceMeshContext,
    transport: ClusterTransportClass,
    members: &[DistributedGroupMember],
    parent_group_id: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_distributed_group|");
    hasher.update(match kind {
        DistributedGroupKind::SingletonFallback => b"singleton".as_slice(),
        DistributedGroupKind::BootstrappedMesh => b"bootstrapped".as_slice(),
        DistributedGroupKind::SplitSubgroup => b"split".as_slice(),
    });
    hasher.update(b"|backend|");
    hasher.update(requested_backend.as_str().as_bytes());
    hasher.update(b"|mesh_id|");
    hasher.update(mesh.mesh_id.as_bytes());
    hasher.update(b"|mesh_revision|");
    hasher.update(mesh.mesh_revision.to_le_bytes());
    hasher.update(b"|effective_backend|");
    hasher.update(mesh.effective_backend.as_bytes());
    hasher.update(b"|communication_class|");
    hasher.update(match mesh.communication_class {
        ClusterCommunicationClass::RemoteDispatch => b"remote_dispatch".as_slice(),
        ClusterCommunicationClass::ReplicaRouting => b"replica_routing".as_slice(),
        ClusterCommunicationClass::PipelineStageHandoff => b"pipeline_stage_handoff".as_slice(),
        ClusterCommunicationClass::LayerShardHandoff => b"layer_shard_handoff".as_slice(),
        ClusterCommunicationClass::TensorCollectiveMesh => b"tensor_collective_mesh".as_slice(),
    });
    hasher.update(b"|transport|");
    hasher.update(cluster_transport_label(transport));
    if let Some(parent_group_id) = parent_group_id {
        hasher.update(b"|parent|");
        hasher.update(parent_group_id.as_bytes());
    }
    for member in members {
        hasher.update(b"|member|");
        hasher.update(member.node_id.as_bytes());
        hasher.update(b"|rank|");
        hasher.update(member.rank.to_le_bytes());
        hasher.update(b"|shard|");
        hasher.update(member.shard_id.to_le_bytes());
        hasher.update(b"|device|");
        hasher.update(member.device_label.as_bytes());
    }
    hex::encode(hasher.finalize())
}

const fn default_distributed_bootstrap_transport() -> ClusterTransportClass {
    ClusterTransportClass::TrustedLanStream
}

const fn cluster_transport_label(transport: ClusterTransportClass) -> &'static [u8] {
    match transport {
        ClusterTransportClass::LocalOnly => b"local_only",
        ClusterTransportClass::Loopback => b"loopback",
        ClusterTransportClass::TrustedLanDatagram => b"trusted_lan_datagram",
        ClusterTransportClass::TrustedLanStream => b"trusted_lan_stream",
        ClusterTransportClass::WiderNetworkStream => b"wider_network_stream",
        ClusterTransportClass::Mixed => b"mixed",
    }
}

const fn collective_topology_profile(
    transport: ClusterTransportClass,
    world_size: usize,
) -> DistributedCollectiveTopologyProfile {
    if world_size <= 1 {
        return DistributedCollectiveTopologyProfile::SingletonLocal;
    }
    match transport {
        ClusterTransportClass::LocalOnly => DistributedCollectiveTopologyProfile::SingletonLocal,
        ClusterTransportClass::Loopback => DistributedCollectiveTopologyProfile::LoopbackMesh,
        ClusterTransportClass::TrustedLanDatagram => {
            DistributedCollectiveTopologyProfile::TrustedLanDatagramMesh
        }
        ClusterTransportClass::TrustedLanStream => {
            DistributedCollectiveTopologyProfile::TrustedLanStreamMesh
        }
        ClusterTransportClass::WiderNetworkStream => {
            DistributedCollectiveTopologyProfile::WiderNetworkStreamMesh
        }
        ClusterTransportClass::Mixed => DistributedCollectiveTopologyProfile::MixedMesh,
    }
}

fn mapped_backend_capability(
    requested_backend: DistributedBackend,
    resolved_backend: DistributedBackend,
    effective_backend: &str,
    communication_class: ClusterCommunicationClass,
    transport: ClusterTransportClass,
    topology_profile: DistributedCollectiveTopologyProfile,
    detail: impl Into<String>,
) -> DistributedBackendCapability {
    DistributedBackendCapability {
        requested_backend,
        resolved_backend,
        effective_backend: effective_backend.to_string(),
        communication_class,
        transport,
        topology_profile,
        backend_transport_available: false,
        detail: detail.into(),
    }
}

fn singleton_fallback_backend_capability(
    requested_backend: DistributedBackend,
    effective_backend: &str,
    communication_class: ClusterCommunicationClass,
    transport: ClusterTransportClass,
) -> DistributedBackendCapability {
    mapped_backend_capability(
        requested_backend,
        DistributedBackend::Any,
        effective_backend,
        communication_class,
        transport,
        DistributedCollectiveTopologyProfile::SingletonLocal,
        "Strict=false init fell back to one singleton local group, so no named distributed backend family is active on the public surface.",
    )
}

fn resolve_named_backend_capability(
    requested_backend: DistributedBackend,
    effective_backend: &str,
    communication_class: ClusterCommunicationClass,
    transport: ClusterTransportClass,
    world_size: usize,
) -> Result<DistributedBackendCapability, DistributedBackendMappingError> {
    let topology_profile = collective_topology_profile(transport, world_size);
    match requested_backend {
        DistributedBackend::Any => {
            unreachable!("named backend resolver requires one concrete family")
        }
        DistributedBackend::Ring => {
            if world_size <= 1 {
                return Ok(mapped_backend_capability(
                    requested_backend,
                    DistributedBackend::Ring,
                    effective_backend,
                    communication_class,
                    transport,
                    topology_profile,
                    "MLX ring semantics collapse honestly to singleton local execution when no peer rank is active.",
                ));
            }
            if communication_class != ClusterCommunicationClass::TensorCollectiveMesh {
                return Err(DistributedBackendMappingError::CommunicationClassMismatch {
                    backend: requested_backend,
                    actual_class: communication_class,
                });
            }
            if !matches!(
                topology_profile,
                DistributedCollectiveTopologyProfile::LoopbackMesh
                    | DistributedCollectiveTopologyProfile::TrustedLanStreamMesh
                    | DistributedCollectiveTopologyProfile::WiderNetworkStreamMesh
                    | DistributedCollectiveTopologyProfile::MixedMesh
            ) {
                return Err(DistributedBackendMappingError::TopologyProfileMismatch {
                    backend: requested_backend,
                    topology_profile,
                    transport,
                    detail: String::from(
                        "MLX ring uses TCP-socket style peer transport, so the public Psionic mapping only admits stream-capable or loopback tensor-collective meshes.",
                    ),
                });
            }
            Ok(mapped_backend_capability(
                requested_backend,
                DistributedBackend::Ring,
                effective_backend,
                communication_class,
                transport,
                topology_profile,
                "MLX ring maps onto a Psionic tensor-collective mesh with stream-capable peer transport; collectives remain reference-emulated on the current public surface.",
            ))
        }
        DistributedBackend::Mpi => {
            if world_size <= 1 {
                return Ok(mapped_backend_capability(
                    requested_backend,
                    DistributedBackend::Mpi,
                    effective_backend,
                    communication_class,
                    transport,
                    topology_profile,
                    "MLX MPI semantics collapse honestly to singleton local execution when no peer rank is active.",
                ));
            }
            if communication_class != ClusterCommunicationClass::TensorCollectiveMesh {
                return Err(DistributedBackendMappingError::CommunicationClassMismatch {
                    backend: requested_backend,
                    actual_class: communication_class,
                });
            }
            if matches!(transport, ClusterTransportClass::LocalOnly) {
                return Err(DistributedBackendMappingError::TopologyProfileMismatch {
                    backend: requested_backend,
                    topology_profile,
                    transport,
                    detail: String::from(
                        "MLX MPI requires a peer-capable process mesh; the current Psionic topology is marked local_only.",
                    ),
                });
            }
            Ok(mapped_backend_capability(
                requested_backend,
                DistributedBackend::Mpi,
                effective_backend,
                communication_class,
                transport,
                topology_profile,
                "MLX MPI maps onto a Psionic tensor-collective mesh with peer-capable loopback or network transport; collectives remain reference-emulated on the current public surface.",
            ))
        }
        DistributedBackend::Nccl => {
            if !effective_backend.eq_ignore_ascii_case("cuda") {
                return Err(DistributedBackendMappingError::EffectiveBackendMismatch {
                    backend: requested_backend,
                    required_backend: String::from("cuda"),
                    effective_backend: effective_backend.to_string(),
                });
            }
            if world_size <= 1 {
                return Ok(mapped_backend_capability(
                    requested_backend,
                    DistributedBackend::Nccl,
                    effective_backend,
                    communication_class,
                    transport,
                    topology_profile,
                    "MLX NCCL maps onto a CUDA-backed singleton local group, but no peer transport is active until a multi-rank mesh is bootstrapped.",
                ));
            }
            if communication_class != ClusterCommunicationClass::TensorCollectiveMesh {
                return Err(DistributedBackendMappingError::CommunicationClassMismatch {
                    backend: requested_backend,
                    actual_class: communication_class,
                });
            }
            if !matches!(
                topology_profile,
                DistributedCollectiveTopologyProfile::LoopbackMesh
                    | DistributedCollectiveTopologyProfile::TrustedLanStreamMesh
                    | DistributedCollectiveTopologyProfile::WiderNetworkStreamMesh
                    | DistributedCollectiveTopologyProfile::MixedMesh
            ) {
                return Err(DistributedBackendMappingError::TopologyProfileMismatch {
                    backend: requested_backend,
                    topology_profile,
                    transport,
                    detail: String::from(
                        "MLX NCCL is mapped only onto CUDA tensor-collective meshes with loopback or stream-capable peer transport on the current Psionic surface.",
                    ),
                });
            }
            Ok(mapped_backend_capability(
                requested_backend,
                DistributedBackend::Nccl,
                effective_backend,
                communication_class,
                transport,
                topology_profile,
                "MLX NCCL maps onto a CUDA tensor-collective mesh with loopback or stream-capable peer transport; collectives remain reference-emulated on the current public surface.",
            ))
        }
        DistributedBackend::Jaccl => {
            Err(DistributedBackendMappingError::TopologyProfileUnavailable {
                backend: requested_backend,
                detail: String::from(
                    "MLX JACCL expects a low-latency RDMA-over-Thunderbolt style topology profile, and Psionic does not expose that transport family publicly yet.",
                ),
            })
        }
    }
}

fn resolve_any_backend_capability(
    effective_backend: &str,
    communication_class: ClusterCommunicationClass,
    transport: ClusterTransportClass,
    world_size: usize,
) -> DistributedBackendCapability {
    let topology_profile = collective_topology_profile(transport, world_size);
    if world_size <= 1 {
        return singleton_fallback_backend_capability(
            DistributedBackend::Any,
            effective_backend,
            communication_class,
            transport,
        );
    }

    let candidate_backends = if effective_backend.eq_ignore_ascii_case("cuda") {
        vec![
            DistributedBackend::Nccl,
            DistributedBackend::Ring,
            DistributedBackend::Mpi,
        ]
    } else {
        vec![DistributedBackend::Ring, DistributedBackend::Mpi]
    };
    for candidate in candidate_backends {
        if let Ok(mut capability) = resolve_named_backend_capability(
            candidate,
            effective_backend,
            communication_class,
            transport,
            world_size,
        ) {
            capability.requested_backend = DistributedBackend::Any;
            return capability;
        }
    }

    if communication_class != ClusterCommunicationClass::TensorCollectiveMesh {
        return mapped_backend_capability(
            DistributedBackend::Any,
            DistributedBackend::Any,
            effective_backend,
            communication_class,
            transport,
            topology_profile,
            format!(
                "Current Psionic topology uses communication class `{:?}` instead of `tensor_collective_mesh`, so no named MLX distributed collective backend is active; the generic public group surface remains available.",
                communication_class
            ),
        );
    }

    mapped_backend_capability(
        DistributedBackend::Any,
        DistributedBackend::Any,
        effective_backend,
        communication_class,
        transport,
        topology_profile,
        format!(
            "Current Psionic topology profile `{}` does not map cleanly onto the public `ring`, `mpi`, `nccl`, or `jaccl` families, so the generic public group surface remains active without a named MLX backend family.",
            topology_profile
        ),
    )
}

fn validate_requested_backend_mapping(
    kind: DistributedGroupKind,
    requested_backend: DistributedBackend,
    effective_backend: &str,
    communication_class: ClusterCommunicationClass,
    transport: ClusterTransportClass,
    world_size: usize,
) -> Result<DistributedBackendCapability, DistributedBackendMappingError> {
    if kind == DistributedGroupKind::SingletonFallback {
        return Ok(singleton_fallback_backend_capability(
            requested_backend,
            effective_backend,
            communication_class,
            transport,
        ));
    }
    match requested_backend {
        DistributedBackend::Any => Ok(resolve_any_backend_capability(
            effective_backend,
            communication_class,
            transport,
            world_size,
        )),
        concrete => resolve_named_backend_capability(
            concrete,
            effective_backend,
            communication_class,
            transport,
            world_size,
        ),
    }
}

fn resolve_distributed_backend_capability(
    kind: DistributedGroupKind,
    requested_backend: DistributedBackend,
    effective_backend: &str,
    communication_class: ClusterCommunicationClass,
    transport: ClusterTransportClass,
    world_size: usize,
) -> DistributedBackendCapability {
    validate_requested_backend_mapping(
        kind,
        requested_backend,
        effective_backend,
        communication_class,
        transport,
        world_size,
    )
    .unwrap_or_else(|error| {
        mapped_backend_capability(
            requested_backend,
            DistributedBackend::Any,
            effective_backend,
            communication_class,
            transport,
            collective_topology_profile(transport, world_size),
            error.to_string(),
        )
    })
}

fn sorted_distinct_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn global_groups() -> &'static Mutex<GlobalDistributedGroups> {
    static GLOBAL_GROUPS: OnceLock<Mutex<GlobalDistributedGroups>> = OnceLock::new();
    GLOBAL_GROUPS.get_or_init(|| Mutex::new(GlobalDistributedGroups::default()))
}

fn register_global_group(state: Arc<DistributedGroupState>) {
    let capability = resolve_distributed_backend_capability(
        state.kind,
        state.requested_backend,
        state.mesh.effective_backend.as_str(),
        state.mesh.communication_class,
        state.transport,
        state.members.len(),
    );
    let mut groups = lock_global_groups();
    groups
        .groups
        .insert(state.requested_backend, Arc::clone(&state));
    groups
        .groups
        .insert(DistributedBackend::Any, Arc::clone(&state));
    if capability.resolved_backend != DistributedBackend::Any {
        groups.groups.insert(capability.resolved_backend, state);
    }
}

fn lock_global_groups() -> MutexGuard<'static, GlobalDistributedGroups> {
    match global_groups().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
fn clear_global_groups() {
    lock_global_groups().groups.clear();
}

#[cfg(test)]
fn distributed_test_lock() -> &'static Mutex<()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
fn lock_distributed_test() -> MutexGuard<'static, ()> {
    match distributed_test_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, net::SocketAddr};

    use psionic_array::ArrayContext;
    use psionic_cluster::{
        AdmissionToken, ClusterEventIndex, ClusterId, ClusterLeadershipRecord, ClusterNamespace,
        ClusterSnapshot, ClusterTerm, NodeEpoch, NodeId, NodeRole,
    };
    use psionic_core::{DType, Device, Shape, TensorSpec};
    use psionic_sandbox::ProviderSandboxRuntimeKind;

    fn sample_membership() -> TrainingElasticMembershipContext {
        TrainingElasticMembershipContext::new(
            7,
            "cluster_state_v7",
            "topology_v7",
            vec![
                String::from("node-a"),
                String::from("node-b"),
                String::from("node-c"),
                String::from("node-d"),
            ],
        )
    }

    fn sample_mesh() -> TrainingDeviceMeshContext {
        TrainingDeviceMeshContext::new(
            "mesh.tensor.v1",
            7,
            "cuda",
            ClusterCommunicationClass::TensorCollectiveMesh,
            sample_membership(),
            vec![
                String::from("node-a"),
                String::from("node-b"),
                String::from("node-c"),
                String::from("node-d"),
            ],
        )
    }

    fn sample_bootstrap(local_node_id: &str) -> DistributedGroupBootstrap {
        sample_bootstrap_with_topology(
            local_node_id,
            "cuda",
            ClusterCommunicationClass::TensorCollectiveMesh,
            ClusterTransportClass::TrustedLanStream,
        )
    }

    fn sample_bootstrap_with_topology(
        local_node_id: &str,
        effective_backend: &str,
        communication_class: ClusterCommunicationClass,
        transport: ClusterTransportClass,
    ) -> DistributedGroupBootstrap {
        let mut mesh = sample_mesh();
        mesh.effective_backend = effective_backend.to_string();
        mesh.communication_class = communication_class;
        DistributedGroupBootstrap::new(
            mesh,
            local_node_id,
            vec![
                DistributedGroupMember::new("node-a", 0, 0, format!("{effective_backend}:0")),
                DistributedGroupMember::new("node-b", 1, 1, format!("{effective_backend}:1")),
                DistributedGroupMember::new("node-c", 2, 2, format!("{effective_backend}:2")),
                DistributedGroupMember::new("node-d", 3, 3, format!("{effective_backend}:3")),
            ],
        )
        .with_transport(transport)
    }

    fn sample_two_rank_membership() -> TrainingElasticMembershipContext {
        TrainingElasticMembershipContext::new(
            3,
            "cluster_state_v3",
            "topology_v3",
            vec![String::from("node-a"), String::from("node-b")],
        )
    }

    fn sample_two_rank_mesh() -> TrainingDeviceMeshContext {
        TrainingDeviceMeshContext::new(
            "mesh.tensor.v2",
            3,
            "cuda",
            ClusterCommunicationClass::TensorCollectiveMesh,
            sample_two_rank_membership(),
            vec![String::from("node-a"), String::from("node-b")],
        )
    }

    fn sample_two_rank_bootstrap(local_node_id: &str) -> DistributedGroupBootstrap {
        DistributedGroupBootstrap::new(
            sample_two_rank_mesh(),
            local_node_id,
            vec![
                DistributedGroupMember::new("node-a", 0, 0, "cuda:0"),
                DistributedGroupMember::new("node-b", 1, 1, "cuda:1"),
            ],
        )
        .with_transport(ClusterTransportClass::TrustedLanStream)
    }

    fn sample_cluster_id() -> ClusterId {
        ClusterId::new(
            &ClusterNamespace::new("psionic-lab"),
            &AdmissionToken::new("launch-secret"),
        )
    }

    fn sample_cluster_membership(
        cluster_id: &ClusterId,
        node_id: &str,
        port: u16,
        role: NodeRole,
        status: ClusterMembershipStatus,
    ) -> ClusterMembershipRecord {
        ClusterMembershipRecord::new(
            psionic_cluster::ClusterNodeIdentity {
                cluster_id: cluster_id.clone(),
                node_id: NodeId::new(node_id),
                node_epoch: NodeEpoch::initial(),
                role,
                auth_public_key: format!("{node_id}-public-key"),
                attestation: None,
            },
            Some(SocketAddr::from(([127, 0, 0, 1], port))),
            status,
        )
    }

    fn sample_cluster_state() -> ClusterState {
        let cluster_id = sample_cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.last_applied_event_index = Some(ClusterEventIndex::initial());
        snapshot.memberships.insert(
            NodeId::new("scheduler-a"),
            sample_cluster_membership(
                &cluster_id,
                "scheduler-a",
                4000,
                NodeRole::CoordinatorOnly,
                ClusterMembershipStatus::Ready,
            ),
        );
        snapshot.memberships.insert(
            NodeId::new("worker-a"),
            sample_cluster_membership(
                &cluster_id,
                "worker-a",
                4101,
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
        );
        snapshot.memberships.insert(
            NodeId::new("worker-b"),
            sample_cluster_membership(
                &cluster_id,
                "worker-b",
                4102,
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
        );
        snapshot.telemetry.insert(
            NodeId::new("worker-a"),
            ClusterNodeTelemetry::new(NodeId::new("worker-a"))
                .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready),
        );
        snapshot.telemetry.insert(
            NodeId::new("worker-b"),
            ClusterNodeTelemetry::new(NodeId::new("worker-b"))
                .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready),
        );
        snapshot.leadership = Some(ClusterLeadershipRecord::new(
            ClusterTerm::initial(),
            NodeId::new("scheduler-a"),
            ClusterEventIndex::initial(),
        ));
        ClusterState::from_snapshot(snapshot)
    }

    fn sample_sandbox_profile() -> ProviderSandboxProfile {
        ProviderSandboxProfile {
            profile_id: String::from("sandbox.posix.v1"),
            profile_digest: String::from("sandbox-profile-digest"),
            execution_class: ProviderSandboxExecutionClass::PosixExec,
            runtime_family: String::from("posix"),
            runtime_version: String::from("1.0"),
            sandbox_engine: String::from("process"),
            os_family: String::from("linux"),
            arch: String::from("x86_64"),
            cpu_limit: 8,
            memory_limit_mb: 16_384,
            disk_limit_mb: 32_768,
            timeout_limit_s: 600,
            network_mode: String::from("disabled"),
            filesystem_mode: String::from("workspace_only"),
            workspace_mode: String::from("workspace_only"),
            artifact_output_mode: String::from("workspace_copy"),
            secrets_mode: String::from("env_allowed"),
            allowed_binaries: vec![String::from("bash")],
            toolchain_inventory: vec![String::from("python3")],
            container_image: None,
            runtime_image_digest: None,
            accelerator_policy: Some(String::from("gpu-optional")),
            runtime_kind: ProviderSandboxRuntimeKind::Posix,
            runtime_ready: true,
            runtime_binary_path: Some(String::from("/usr/bin/env")),
            capability_summary: String::from("Posix sandbox ready"),
        }
    }

    fn sample_launch_config(
        hostfile_entries: Vec<DistributedHostfileEntry>,
    ) -> DistributedLaunchConfig {
        DistributedLaunchConfig::new(
            "launch.train.v1",
            "scheduler-a",
            "cuda",
            "bin/train.sh",
            "/tmp/psionic-distributed-launch",
        )
        .with_hostfile_entries(hostfile_entries)
        .with_provider_id("provider-alpha")
        .with_expected_outputs(vec![String::from("artifacts/weights.bin")])
    }

    fn training_spec(width: usize) -> TensorSpec {
        TensorSpec::new(Shape::new(vec![width]), DType::F32, Device::cpu())
    }

    fn training_buffer(group_id: &str, values: Vec<f32>) -> TrainingTensorBuffer {
        TrainingTensorBuffer::from_f32(group_id, training_spec(values.len()), values)
            .expect("training buffer")
    }

    fn training_group(
        group_id: &str,
        class: psionic_train::TrainingParameterClass,
        values: Vec<f32>,
        optimizer: psionic_train::TrainingOptimizerConfig,
    ) -> TrainingParameterGroupState {
        TrainingParameterGroupState::new(
            group_id,
            class,
            training_buffer(group_id, values),
            optimizer,
            psionic_train::TrainingOptimizerResidencyPolicy::device_step_offload_idle(),
        )
        .expect("training group")
    }

    fn two_rank_full_shard_placements(width: usize) -> Vec<TrainingShardPlacement> {
        assert!(
            width.is_multiple_of(2),
            "test full-shard width must divide evenly"
        );
        let shard_width = width / 2;
        vec![
            TrainingShardPlacement::new(
                0,
                "tensor",
                "node-a",
                "cuda:0",
                0,
                TrainingShardRange::new(0, shard_width),
            ),
            TrainingShardPlacement::new(
                1,
                "tensor",
                "node-b",
                "cuda:1",
                0,
                TrainingShardRange::new(shard_width, shard_width),
            ),
        ]
    }

    fn two_rank_replicated_placements(width: usize) -> Vec<TrainingShardPlacement> {
        vec![
            TrainingShardPlacement::new(
                0,
                "replica",
                "node-a",
                "cuda:0",
                0,
                TrainingShardRange::new(0, width),
            ),
            TrainingShardPlacement::new(
                1,
                "replica",
                "node-b",
                "cuda:1",
                0,
                TrainingShardRange::new(0, width),
            ),
        ]
    }

    fn fsdp_sync_plan() -> psionic_collectives::CollectiveSyncExecutionPlan {
        psionic_collectives::CollectiveSyncExecutionPlan {
            cadence_receipt: psionic_collectives::CollectiveSyncCadenceReceipt::new(
                1,
                1,
                psionic_collectives::CollectiveSyncCadenceClass::EveryStepGlobal,
                1,
                2,
                None,
                psionic_runtime::TrainingCollectiveQuantization::None,
                psionic_runtime::TrainingCollectiveQuantization::None,
                false,
                Vec::new(),
                None,
            ),
            stages: Vec::new(),
        }
    }

    fn fsdp_contract(
        groups: Vec<DistributedOptimizerGroupContract>,
    ) -> DistributedOptimizerContract {
        DistributedOptimizerContract::new(
            "fsdp.contract.v1",
            TrainingDistributedOptimizerKind::ZeroStage3,
            psionic_train::TrainingPrecisionPolicy {
                parameter_precision: psionic_train::TrainingPrecisionMode::Fp32,
                gradient_precision: psionic_train::TrainingPrecisionMode::Fp32,
                optimizer_state_precision: psionic_train::TrainingPrecisionMode::Fp32,
                master_weight_precision: psionic_train::TrainingPrecisionMode::Fp32,
                reduction_precision: psionic_train::TrainingPrecisionMode::Fp32,
                communication_quantization: psionic_runtime::TrainingCollectiveQuantization::None,
                stochastic_rounding: false,
                loss_scale: None,
            },
            psionic_train::TrainingGradientAccumulationPolicy::new(
                1,
                psionic_train::TrainingGradientAccumulationReduction::Sum,
                psionic_runtime::TrainingCollectiveKind::AllReduce,
            ),
            psionic_train::TrainingActivationCheckpointPolicy::Disabled {
                activation_peak_bytes: 0,
            },
            psionic_train::DistributedTrainingMemoryBudget::new(1_000_000, 1_000_000, 0),
            fsdp_sync_plan(),
            groups,
        )
        .expect("distributed optimizer contract")
    }

    fn full_shard_matrix_contract(
        group_id: &str,
        width: usize,
    ) -> DistributedOptimizerGroupContract {
        DistributedOptimizerGroupContract::new(
            group_id,
            psionic_train::TrainingParameterClass::Matrix,
            psionic_train::TrainingParameterShardLayout::new(
                TrainingParameterShardKind::FullShard,
                two_rank_full_shard_placements(width),
            ),
            psionic_train::TrainingParameterShardLayout::new(
                TrainingParameterShardKind::FullShard,
                two_rank_full_shard_placements(width),
            ),
            psionic_train::TrainingOptimizerStateShardLayout::new(
                TrainingOptimizerStateShardKind::ZeroStage3,
                psionic_train::TrainingOptimizerShardResidency::DeviceResident,
                two_rank_full_shard_placements(width),
            ),
            OptimizerStateResidency::Offloaded,
        )
    }

    fn replicated_bias_contract(group_id: &str, width: usize) -> DistributedOptimizerGroupContract {
        DistributedOptimizerGroupContract::new(
            group_id,
            psionic_train::TrainingParameterClass::Bias,
            psionic_train::TrainingParameterShardLayout::new(
                TrainingParameterShardKind::Replicated,
                two_rank_replicated_placements(width),
            ),
            psionic_train::TrainingParameterShardLayout::new(
                TrainingParameterShardKind::Replicated,
                two_rank_replicated_placements(width),
            ),
            psionic_train::TrainingOptimizerStateShardLayout::new(
                TrainingOptimizerStateShardKind::Replicated,
                psionic_train::TrainingOptimizerShardResidency::DeviceResident,
                two_rank_replicated_placements(width),
            ),
            OptimizerStateResidency::Offloaded,
        )
    }

    fn training_batch(batch_id: &str, gradients: &[(&str, Vec<f32>)]) -> TrainingGradientBatch {
        TrainingGradientBatch::new(
            batch_id,
            0.0,
            1,
            gradients
                .iter()
                .map(|(group_id, values)| {
                    (
                        (*group_id).to_string(),
                        training_buffer(group_id, values.clone()),
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn init_returns_singleton_fallback_when_non_strict_group_is_unavailable() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let group = init(DistributedInitOptions::new())
            .expect("non-strict init should return one singleton fallback");

        assert!(group.is_singleton());
        assert_eq!(group.rank(), 0);
        assert_eq!(group.size(), 1);
        assert_eq!(group.kind(), DistributedGroupKind::SingletonFallback);
        assert_eq!(group.requested_backend(), DistributedBackend::Any);
        assert_eq!(group.snapshot().members[0].node_id, "singleton");
    }

    #[test]
    fn init_strict_refuses_without_bootstrap_or_existing_global_group() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let error = init(DistributedInitOptions::new().with_strict(true))
            .expect_err("strict init should refuse without a reusable group");

        assert_eq!(
            error,
            DistributedInitError::GlobalGroupUnavailable {
                backend: DistributedBackend::Any
            }
        );
    }

    #[test]
    fn init_bootstraps_one_group_and_reuses_it_as_the_global_group() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let bootstrapped =
            init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-c")))
                .expect("bootstrapped mesh should initialize one public group");

        assert_eq!(bootstrapped.rank(), 2);
        assert_eq!(bootstrapped.size(), 4);
        assert_eq!(bootstrapped.kind(), DistributedGroupKind::BootstrappedMesh);
        assert_eq!(bootstrapped.effective_backend(), "cuda");
        assert_eq!(
            global_group(DistributedBackend::Any)
                .expect("bootstrapped group should be reusable")
                .group_id(),
            bootstrapped.group_id()
        );

        let reused = init(DistributedInitOptions::new())
            .expect("init should reuse the bootstrapped global group");
        assert_eq!(reused.group_id(), bootstrapped.group_id());
        assert_eq!(reused.rank(), 2);
        assert_eq!(reused.size(), 4);
    }

    #[test]
    fn named_backend_families_map_onto_current_topology_profiles() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let ring = init(
            DistributedInitOptions::new()
                .with_backend(DistributedBackend::Ring)
                .with_bootstrap(sample_bootstrap("node-a")),
        )
        .expect("ring should map onto the default stream-capable tensor mesh");
        assert_eq!(
            ring.backend_capability().resolved_backend,
            DistributedBackend::Ring
        );
        assert_eq!(
            ring.backend_capability().topology_profile,
            DistributedCollectiveTopologyProfile::TrustedLanStreamMesh
        );
        assert_eq!(
            global_group(DistributedBackend::Any)
                .expect("named backend init should alias the reusable any group")
                .group_id(),
            ring.group_id()
        );

        clear_global_groups();
        let mpi = init(
            DistributedInitOptions::new()
                .with_backend(DistributedBackend::Mpi)
                .with_bootstrap(sample_bootstrap_with_topology(
                    "node-b",
                    "cpu",
                    ClusterCommunicationClass::TensorCollectiveMesh,
                    ClusterTransportClass::TrustedLanDatagram,
                )),
        )
        .expect("mpi should map onto the peer-capable datagram tensor mesh");
        assert_eq!(
            mpi.backend_capability().resolved_backend,
            DistributedBackend::Mpi
        );
        assert_eq!(mpi.effective_backend(), "cpu");
        assert_eq!(
            mpi.backend_capability().topology_profile,
            DistributedCollectiveTopologyProfile::TrustedLanDatagramMesh
        );

        clear_global_groups();
        let nccl = init(
            DistributedInitOptions::new()
                .with_backend(DistributedBackend::Nccl)
                .with_bootstrap(sample_bootstrap("node-c")),
        )
        .expect("nccl should map onto the default CUDA tensor mesh");
        assert_eq!(
            nccl.backend_capability().resolved_backend,
            DistributedBackend::Nccl
        );
        assert_eq!(nccl.effective_backend(), "cuda");

        clear_global_groups();
        let any = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-d")))
            .expect("backend=any should resolve the first truthful family for the mesh");
        assert_eq!(
            any.backend_capability().requested_backend,
            DistributedBackend::Any
        );
        assert_eq!(
            any.backend_capability().resolved_backend,
            DistributedBackend::Nccl
        );
    }

    #[test]
    fn named_backend_families_refuse_incompatible_topologies() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let ring_error = init(
            DistributedInitOptions::new()
                .with_backend(DistributedBackend::Ring)
                .with_bootstrap(sample_bootstrap_with_topology(
                    "node-a",
                    "cuda",
                    ClusterCommunicationClass::TensorCollectiveMesh,
                    ClusterTransportClass::TrustedLanDatagram,
                )),
        )
        .expect_err("ring should refuse datagram-only transport on the public surface");
        assert!(matches!(
            ring_error,
            DistributedInitError::BackendFamilyMapping(
                DistributedBackendMappingError::TopologyProfileMismatch {
                    backend: DistributedBackend::Ring,
                    topology_profile: DistributedCollectiveTopologyProfile::TrustedLanDatagramMesh,
                    ..
                }
            )
        ));

        let nccl_error = init(
            DistributedInitOptions::new()
                .with_backend(DistributedBackend::Nccl)
                .with_bootstrap(sample_bootstrap_with_topology(
                    "node-a",
                    "cpu",
                    ClusterCommunicationClass::TensorCollectiveMesh,
                    ClusterTransportClass::TrustedLanStream,
                )),
        )
        .expect_err("nccl should refuse non-cuda effective backends");
        assert!(matches!(
            nccl_error,
            DistributedInitError::BackendFamilyMapping(
                DistributedBackendMappingError::EffectiveBackendMismatch {
                    backend: DistributedBackend::Nccl,
                    ..
                }
            )
        ));

        let jaccl_error = init(
            DistributedInitOptions::new()
                .with_backend(DistributedBackend::Jaccl)
                .with_bootstrap(sample_bootstrap("node-a")),
        )
        .expect_err("jaccl should refuse until Psionic exposes a real RDMA topology profile");
        assert!(matches!(
            jaccl_error,
            DistributedInitError::BackendFamilyMapping(
                DistributedBackendMappingError::TopologyProfileUnavailable {
                    backend: DistributedBackend::Jaccl,
                    ..
                }
            )
        ));
    }

    #[test]
    fn split_uses_one_explicit_plan_and_reassigns_rank_by_key() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-c")))
            .expect("bootstrapped mesh should initialize one public group");
        let plan = DistributedSplitPlan::new(vec![
            DistributedSplitAssignment::new("node-a", 0).with_key(20),
            DistributedSplitAssignment::new("node-b", 1).with_key(10),
            DistributedSplitAssignment::new("node-c", 0).with_key(5),
            DistributedSplitAssignment::new("node-d", 1).with_key(15),
        ]);

        let subgroup = group
            .split_with_plan(0, 5, plan)
            .expect("explicit split plan should realize the color-matched subgroup");

        assert_eq!(subgroup.kind(), DistributedGroupKind::SplitSubgroup);
        assert_eq!(subgroup.size(), 2);
        assert_eq!(subgroup.rank(), 0);
        assert_eq!(
            subgroup
                .members()
                .iter()
                .map(|member| (member.node_id.as_str(), member.rank))
                .collect::<Vec<_>>(),
            vec![("node-c", 0), ("node-a", 1)]
        );
        assert_eq!(
            subgroup.snapshot().parent_group_id,
            Some(group.group_id().to_string())
        );
    }

    #[test]
    fn split_refuses_missing_plan_singleton_and_local_assignment_mismatch() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let singleton = init(DistributedInitOptions::new())
            .expect("non-strict init should return the singleton fallback");
        assert_eq!(
            singleton.split(0, -1).expect_err("singleton cannot split"),
            DistributedGroupError::CannotSplitSingleton
        );

        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-b")))
            .expect("bootstrapped mesh should initialize one public group");
        assert_eq!(
            group
                .split(1, -1)
                .expect_err("split should require one explicit plan"),
            DistributedGroupError::SplitPlanUnavailable
        );

        let mismatch = group
            .split_with_plan(
                0,
                -1,
                DistributedSplitPlan::new(vec![
                    DistributedSplitAssignment::new("node-a", 0),
                    DistributedSplitAssignment::new("node-b", 1),
                    DistributedSplitAssignment::new("node-c", 0),
                    DistributedSplitAssignment::new("node-d", 1),
                ]),
            )
            .expect_err("local caller must agree with the explicit split plan");
        assert_eq!(
            mismatch,
            DistributedGroupError::LocalAssignmentMismatch {
                expected_color: 1,
                expected_key: 1,
                actual_color: 0,
                actual_key: 1,
            }
        );
    }

    #[test]
    fn collective_support_reports_singleton_and_reference_bounds() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let singleton = init(DistributedInitOptions::new())
            .expect("non-strict init should return the singleton fallback");
        let singleton_support = singleton.collective_support();
        assert_eq!(
            singleton_support.backend_capability.resolved_backend,
            DistributedBackend::Any
        );
        assert_eq!(
            singleton_support.all_sum,
            DistributedCollectiveSupportStatus::SingletonPassthrough
        );
        assert_eq!(
            singleton_support.send,
            DistributedCollectiveSupportStatus::TypedRefusal
        );
        assert!(!singleton_support.backend_transport_available);

        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-b")))
            .expect("bootstrapped mesh should initialize one public group");
        let support = group.collective_support();
        assert_eq!(
            support.backend_capability.resolved_backend,
            DistributedBackend::Nccl
        );
        assert_eq!(
            support.backend_capability.topology_profile,
            DistributedCollectiveTopologyProfile::TrustedLanStreamMesh
        );
        assert_eq!(
            support.all_gather,
            DistributedCollectiveSupportStatus::ReferenceEmulation
        );
        assert_eq!(
            support.send,
            DistributedCollectiveSupportStatus::ValidationOnly
        );
        assert_eq!(
            support.recv,
            DistributedCollectiveSupportStatus::ReferenceEmulation
        );
        assert!(!support.backend_transport_available);
    }

    #[test]
    fn all_sum_respects_singleton_passthrough_and_multi_rank_reference_inputs() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let context = ArrayContext::cpu();
        let singleton = context
            .constant_f32(Shape::new(vec![2]), vec![3.0, 4.0])
            .expect("singleton input");
        let singleton_sum = all_sum(&singleton, DistributedCollectiveOptions::new())
            .expect("singleton passthrough");
        assert_eq!(
            singleton_sum
                .to_host_data()
                .expect("singleton host")
                .as_f32_slice(),
            Some(&[3.0, 4.0][..])
        );

        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-c")))
            .expect("bootstrapped mesh should initialize one public group");
        let local = context
            .constant_f32(Shape::new(vec![2]), vec![3.0, 4.0])
            .expect("local input");
        let rank_inputs = BTreeMap::from([
            (
                0,
                DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![1.0, 1.0])
                    .expect("rank 0 input"),
            ),
            (
                1,
                DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![2.0, 2.0])
                    .expect("rank 1 input"),
            ),
            (
                2,
                DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![3.0, 4.0])
                    .expect("rank 2 input"),
            ),
            (
                3,
                DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![4.0, 5.0])
                    .expect("rank 3 input"),
            ),
        ]);

        let reduced = all_sum(
            &local,
            DistributedCollectiveOptions::new()
                .with_group(group)
                .with_rank_inputs(rank_inputs),
        )
        .expect("all_sum should emulate the four-rank reduction");
        assert_eq!(
            reduced.to_host_data().expect("reduced host").as_f32_slice(),
            Some(&[10.0, 12.0][..])
        );
    }

    #[test]
    fn all_sum_refuses_missing_or_mismatched_rank_inputs() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let context = ArrayContext::cpu();
        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-b")))
            .expect("bootstrapped mesh should initialize one public group");
        let local = context
            .constant_f32(Shape::new(vec![2]), vec![3.0, 4.0])
            .expect("local input");

        let missing = all_sum(
            &local,
            DistributedCollectiveOptions::new()
                .with_group(group.clone())
                .with_rank_inputs(BTreeMap::from([(
                    1,
                    DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![3.0, 4.0])
                        .expect("local payload"),
                )])),
        )
        .expect_err("missing remote ranks should refuse");
        assert_eq!(
            missing,
            DistributedCollectiveError::MissingRankInputs {
                operation: DistributedCollectiveOperation::AllSum,
                missing_ranks: vec![0, 2, 3],
            }
        );

        let mismatch = all_sum(
            &local,
            DistributedCollectiveOptions::new()
                .with_group(group)
                .with_rank_inputs(BTreeMap::from([
                    (
                        0,
                        DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![1.0, 1.0])
                            .expect("rank 0"),
                    ),
                    (
                        1,
                        DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![9.0, 9.0])
                            .expect("local mismatch"),
                    ),
                    (
                        2,
                        DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![2.0, 2.0])
                            .expect("rank 2"),
                    ),
                    (
                        3,
                        DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![4.0, 4.0])
                            .expect("rank 3"),
                    ),
                ])),
        )
        .expect_err("local rank payload mismatch should refuse");
        assert_eq!(
            mismatch,
            DistributedCollectiveError::LocalRankInputMismatch {
                operation: DistributedCollectiveOperation::AllSum,
                rank: 1,
            }
        );
    }

    #[test]
    fn all_gather_handles_vector_and_scalar_payloads() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let context = ArrayContext::cpu();
        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-b")))
            .expect("bootstrapped mesh should initialize one public group");
        let local = context
            .constant_f32(Shape::new(vec![2]), vec![3.0, 4.0])
            .expect("local input");
        let gathered = all_gather(
            &local,
            DistributedCollectiveOptions::new()
                .with_group(group.clone())
                .with_rank_inputs(BTreeMap::from([
                    (
                        0,
                        DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![1.0, 2.0])
                            .expect("rank 0"),
                    ),
                    (
                        1,
                        DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![3.0, 4.0])
                            .expect("rank 1"),
                    ),
                    (
                        2,
                        DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![5.0, 6.0])
                            .expect("rank 2"),
                    ),
                    (
                        3,
                        DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![7.0, 8.0])
                            .expect("rank 3"),
                    ),
                ])),
        )
        .expect("all_gather should concatenate first-axis payloads");
        assert_eq!(gathered.shape(), &Shape::new(vec![8]));
        assert_eq!(
            gathered
                .to_host_data()
                .expect("gathered host")
                .as_f32_slice(),
            Some(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0][..])
        );

        let scalar = context.scalar_f32(2.0).expect("scalar input");
        let scalar_gathered = all_gather(
            &scalar,
            DistributedCollectiveOptions::new()
                .with_group(group)
                .with_rank_inputs(BTreeMap::from([
                    (
                        0,
                        DistributedReferenceTensor::f32(Shape::scalar(), vec![1.0])
                            .expect("rank 0"),
                    ),
                    (
                        1,
                        DistributedReferenceTensor::f32(Shape::scalar(), vec![2.0])
                            .expect("rank 1"),
                    ),
                    (
                        2,
                        DistributedReferenceTensor::f32(Shape::scalar(), vec![3.0])
                            .expect("rank 2"),
                    ),
                    (
                        3,
                        DistributedReferenceTensor::f32(Shape::scalar(), vec![4.0])
                            .expect("rank 3"),
                    ),
                ])),
        )
        .expect("scalar gather should widen into one first axis");
        assert_eq!(scalar_gathered.shape(), &Shape::new(vec![4]));
        assert_eq!(
            scalar_gathered
                .to_host_data()
                .expect("scalar gathered host")
                .as_f32_slice(),
            Some(&[1.0, 2.0, 3.0, 4.0][..])
        );
    }

    #[test]
    fn reduce_scatter_sums_then_slices_local_chunk() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let context = ArrayContext::cpu();
        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-b")))
            .expect("bootstrapped mesh should initialize one public group");
        let local = context
            .constant_f32(Shape::new(vec![4]), vec![2.0, 20.0, 200.0, 2000.0])
            .expect("local input");
        let scattered = reduce_scatter(
            &local,
            DistributedCollectiveOptions::new()
                .with_group(group)
                .with_rank_inputs(BTreeMap::from([
                    (
                        0,
                        DistributedReferenceTensor::f32(
                            Shape::new(vec![4]),
                            vec![1.0, 10.0, 100.0, 1000.0],
                        )
                        .expect("rank 0"),
                    ),
                    (
                        1,
                        DistributedReferenceTensor::f32(
                            Shape::new(vec![4]),
                            vec![2.0, 20.0, 200.0, 2000.0],
                        )
                        .expect("rank 1"),
                    ),
                    (
                        2,
                        DistributedReferenceTensor::f32(
                            Shape::new(vec![4]),
                            vec![3.0, 30.0, 300.0, 3000.0],
                        )
                        .expect("rank 2"),
                    ),
                    (
                        3,
                        DistributedReferenceTensor::f32(
                            Shape::new(vec![4]),
                            vec![4.0, 40.0, 400.0, 4000.0],
                        )
                        .expect("rank 3"),
                    ),
                ])),
        )
        .expect("reduce_scatter should sum then return the local chunk");
        assert_eq!(scattered.shape(), &Shape::new(vec![1]));
        assert_eq!(
            scattered
                .to_host_data()
                .expect("scattered host")
                .as_f32_slice(),
            Some(&[100.0][..])
        );
    }

    #[test]
    fn reduce_scatter_refuses_scalar_or_non_divisible_axis_zero() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let context = ArrayContext::cpu();
        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-a")))
            .expect("bootstrapped mesh should initialize one public group");
        let scalar = context.scalar_f32(1.0).expect("scalar input");
        let scalar_error = reduce_scatter(
            &scalar,
            DistributedCollectiveOptions::new()
                .with_group(group.clone())
                .with_rank_inputs(BTreeMap::from([
                    (
                        0,
                        DistributedReferenceTensor::f32(Shape::scalar(), vec![1.0])
                            .expect("rank 0"),
                    ),
                    (
                        1,
                        DistributedReferenceTensor::f32(Shape::scalar(), vec![2.0])
                            .expect("rank 1"),
                    ),
                    (
                        2,
                        DistributedReferenceTensor::f32(Shape::scalar(), vec![3.0])
                            .expect("rank 2"),
                    ),
                    (
                        3,
                        DistributedReferenceTensor::f32(Shape::scalar(), vec![4.0])
                            .expect("rank 3"),
                    ),
                ])),
        )
        .expect_err("scalar reduce_scatter should refuse");
        assert_eq!(
            scalar_error,
            DistributedCollectiveError::ReduceScatterScalarInput
        );

        let vector = context
            .constant_f32(Shape::new(vec![5]), vec![1.0, 2.0, 3.0, 4.0, 5.0])
            .expect("vector input");
        let divisibility_error = reduce_scatter(
            &vector,
            DistributedCollectiveOptions::new()
                .with_group(group)
                .with_rank_inputs(BTreeMap::from([
                    (
                        0,
                        DistributedReferenceTensor::f32(
                            Shape::new(vec![5]),
                            vec![1.0, 2.0, 3.0, 4.0, 5.0],
                        )
                        .expect("rank 0"),
                    ),
                    (
                        1,
                        DistributedReferenceTensor::f32(
                            Shape::new(vec![5]),
                            vec![1.0, 2.0, 3.0, 4.0, 5.0],
                        )
                        .expect("rank 1"),
                    ),
                    (
                        2,
                        DistributedReferenceTensor::f32(
                            Shape::new(vec![5]),
                            vec![1.0, 2.0, 3.0, 4.0, 5.0],
                        )
                        .expect("rank 2"),
                    ),
                    (
                        3,
                        DistributedReferenceTensor::f32(
                            Shape::new(vec![5]),
                            vec![1.0, 2.0, 3.0, 4.0, 5.0],
                        )
                        .expect("rank 3"),
                    ),
                ])),
        )
        .expect_err("axis 0 must divide evenly by group size");
        assert_eq!(
            divisibility_error,
            DistributedCollectiveError::ReduceScatterNonDivisibleAxis0 {
                first_axis: 5,
                group_size: 4,
            }
        );
    }

    #[test]
    fn send_and_recv_cover_validation_and_reference_payload_paths() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let context = ArrayContext::cpu();
        let singleton = context.scalar_f32(1.0).expect("singleton input");
        assert_eq!(
            send(&singleton, 0, DistributedPointToPointOptions::new())
                .expect_err("singleton send should refuse"),
            DistributedCollectiveError::CannotSendSingleton
        );
        assert_eq!(
            recv(
                &context,
                Shape::scalar(),
                DType::F32,
                0,
                DistributedPointToPointOptions::new(),
            )
            .expect_err("singleton recv should refuse"),
            DistributedCollectiveError::CannotRecvSingleton
        );

        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-a")))
            .expect("bootstrapped mesh should initialize one public group");
        let local = context
            .constant_f32(Shape::new(vec![2]), vec![5.0, 6.0])
            .expect("local input");
        let sent = send(
            &local,
            3,
            DistributedPointToPointOptions::new()
                .with_group(group.clone())
                .with_message_payload(
                    3,
                    DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![5.0, 6.0])
                        .expect("expected outbound"),
                ),
        )
        .expect("send should validate destination and return the local array");
        assert_eq!(
            sent.to_host_data().expect("sent host").as_f32_slice(),
            Some(&[5.0, 6.0][..])
        );

        let invalid_destination = send(
            &local,
            4,
            DistributedPointToPointOptions::new().with_group(group.clone()),
        )
        .expect_err("out-of-range destination should refuse");
        assert_eq!(
            invalid_destination,
            DistributedCollectiveError::InvalidDestination {
                destination: 4,
                group_size: 4,
            }
        );

        let send_mismatch = send(
            &local,
            2,
            DistributedPointToPointOptions::new()
                .with_group(group.clone())
                .with_message_payload(
                    2,
                    DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![9.0, 9.0])
                        .expect("mismatched outbound"),
                ),
        )
        .expect_err("explicit outbound mismatch should refuse");
        assert_eq!(
            send_mismatch,
            DistributedCollectiveError::SendPayloadMismatch { destination: 2 }
        );

        let received = recv(
            &context,
            Shape::new(vec![2]),
            DType::F32,
            2,
            DistributedPointToPointOptions::new()
                .with_group(group.clone())
                .with_message_payload(
                    2,
                    DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![7.0, 8.0])
                        .expect("inbound payload"),
                ),
        )
        .expect("recv should materialize the explicit inbound payload");
        assert_eq!(
            received
                .to_host_data()
                .expect("received host")
                .as_f32_slice(),
            Some(&[7.0, 8.0][..])
        );

        let missing_recv = recv(
            &context,
            Shape::new(vec![2]),
            DType::F32,
            1,
            DistributedPointToPointOptions::new().with_group(group.clone()),
        )
        .expect_err("recv requires an inbound payload on the bounded surface");
        assert_eq!(
            missing_recv,
            DistributedCollectiveError::MissingReceivePayload { source_rank: 1 }
        );

        let mismatched_recv = recv(
            &context,
            Shape::new(vec![2]),
            DType::F32,
            1,
            DistributedPointToPointOptions::new()
                .with_group(group)
                .with_message_payload(
                    1,
                    DistributedReferenceTensor::i8(Shape::new(vec![2]), vec![1, 2])
                        .expect("i8 payload"),
                ),
        )
        .expect_err("recv payload shape/dtype mismatches should refuse");
        assert_eq!(
            mismatched_recv,
            DistributedCollectiveError::ReceivePayloadMismatch {
                source_rank: 1,
                expected_shape: Shape::new(vec![2]),
                actual_shape: Shape::new(vec![2]),
                expected_dtype: DType::F32,
                actual_dtype: DType::I8,
            }
        );
    }

    #[test]
    fn parse_hostfile_accepts_comments_slots_and_addresses() {
        let parsed = parse_hostfile(
            "\n# comment\nworker-a slots=1 addr=127.0.0.1:4101\nworker-b addr=127.0.0.1:4102\n",
        )
        .expect("hostfile should parse valid entries");

        assert_eq!(
            parsed,
            vec![
                DistributedHostfileEntry::new("worker-a")
                    .with_slots(1)
                    .with_advertised_addr("127.0.0.1:4101"),
                DistributedHostfileEntry::new("worker-b").with_advertised_addr("127.0.0.1:4102"),
            ]
        );
    }

    #[test]
    fn parse_hostfile_refuses_duplicate_nodes_and_unknown_tokens() {
        let duplicate = parse_hostfile("worker-a\nworker-a")
            .expect_err("duplicate hostfile nodes should refuse");
        assert_eq!(
            duplicate,
            DistributedLaunchError::DuplicateHostfileNodeId {
                node_id: String::from("worker-a"),
            }
        );

        let unsupported = parse_hostfile("worker-a gpu=1")
            .expect_err("unsupported hostfile directives should refuse");
        assert_eq!(
            unsupported,
            DistributedLaunchError::UnsupportedHostfileToken {
                line: 1,
                token: String::from("gpu=1"),
            }
        );
    }

    #[test]
    fn plan_launch_emits_cluster_evidence_rank_assignments_and_reserved_environment() {
        let cluster_state = sample_cluster_state();
        let sandbox_profile = sample_sandbox_profile();
        let config = sample_launch_config(vec![
            DistributedHostfileEntry::new("worker-a").with_advertised_addr("127.0.0.1:4101"),
            DistributedHostfileEntry::new("worker-b").with_advertised_addr("127.0.0.1:4102"),
        ]);

        let plan = plan_launch(&cluster_state, &sandbox_profile, config)
            .expect("launch plan should be derived from cluster and sandbox truth");

        assert_eq!(plan.world_size, 2);
        assert_eq!(plan.assignments.len(), 2);
        assert_eq!(
            plan.members
                .iter()
                .map(|member| (member.node_id.as_str(), member.rank))
                .collect::<Vec<_>>(),
            vec![("worker-a", 0), ("worker-b", 1)]
        );
        assert_eq!(plan.cluster_execution.scheduler_node_id, "scheduler-a");
        assert_eq!(plan.cluster_execution.selected_nodes.len(), 2);
        assert_eq!(
            plan.cluster_execution.commit_authority,
            cluster_commit_authority_evidence(&cluster_state)
        );
        assert_eq!(
            plan.assignments[0]
                .environment
                .iter()
                .find(|env| env.key == "PSIONIC_DISTRIBUTED_GROUP_ID")
                .map(|env| env.value.as_str()),
            Some(plan.group_id.as_str())
        );
        assert_eq!(
            plan.assignments[1]
                .environment
                .iter()
                .find(|env| env.key == "PSIONIC_DISTRIBUTED_RANK")
                .map(|env| env.value.as_str()),
            Some("1")
        );
        assert_eq!(
            plan.assignments[0].sandbox_job.compute_product_id,
            ProviderSandboxExecutionClass::PosixExec.product_id()
        );
        assert_eq!(
            plan.backend_capability.requested_backend,
            DistributedBackend::Any
        );
        assert_eq!(
            plan.backend_capability.resolved_backend,
            DistributedBackend::Nccl
        );
        assert_eq!(
            plan.backend_capability.topology_profile,
            DistributedCollectiveTopologyProfile::TrustedLanStreamMesh
        );
    }

    #[test]
    fn plan_launch_maps_named_backend_families_and_refuses_missing_profiles() {
        let cluster_state = sample_cluster_state();
        let sandbox_profile = sample_sandbox_profile();
        let hostfile = vec![
            DistributedHostfileEntry::new("worker-a").with_advertised_addr("127.0.0.1:4101"),
            DistributedHostfileEntry::new("worker-b").with_advertised_addr("127.0.0.1:4102"),
        ];

        let nccl_plan = plan_launch(
            &cluster_state,
            &sandbox_profile,
            sample_launch_config(hostfile.clone()).with_requested_backend(DistributedBackend::Nccl),
        )
        .expect("nccl launch should map onto the default CUDA tensor mesh");
        assert_eq!(
            nccl_plan.backend_capability.resolved_backend,
            DistributedBackend::Nccl
        );

        let mut mpi_config =
            sample_launch_config(hostfile.clone()).with_requested_backend(DistributedBackend::Mpi);
        mpi_config.transport = ClusterTransportClass::TrustedLanDatagram;
        let mpi_plan = plan_launch(&cluster_state, &sandbox_profile, mpi_config)
            .expect("mpi launch should map onto the datagram tensor mesh");
        assert_eq!(
            mpi_plan.backend_capability.resolved_backend,
            DistributedBackend::Mpi
        );
        assert_eq!(
            mpi_plan.backend_capability.topology_profile,
            DistributedCollectiveTopologyProfile::TrustedLanDatagramMesh
        );

        let jaccl_error = plan_launch(
            &cluster_state,
            &sandbox_profile,
            sample_launch_config(hostfile).with_requested_backend(DistributedBackend::Jaccl),
        )
        .expect_err("jaccl launch should refuse without an RDMA topology profile");
        assert!(matches!(
            jaccl_error,
            DistributedLaunchError::BackendFamilyMapping(
                DistributedBackendMappingError::TopologyProfileUnavailable {
                    backend: DistributedBackend::Jaccl,
                    ..
                }
            )
        ));
    }

    #[test]
    fn plan_launch_digest_is_stable_for_equal_inputs_and_changes_when_config_changes() {
        let cluster_state = sample_cluster_state();
        let sandbox_profile = sample_sandbox_profile();
        let config = sample_launch_config(vec![
            DistributedHostfileEntry::new("worker-a"),
            DistributedHostfileEntry::new("worker-b"),
        ]);

        let first = plan_launch(&cluster_state, &sandbox_profile, config.clone())
            .expect("first launch plan should succeed");
        let second = plan_launch(&cluster_state, &sandbox_profile, config.clone())
            .expect("second launch plan should succeed");
        assert_eq!(first.plan_digest, second.plan_digest);

        let changed = plan_launch(
            &cluster_state,
            &sandbox_profile,
            config.with_provider_id("provider-beta"),
        )
        .expect("changed config should still succeed");
        assert_ne!(first.plan_digest, changed.plan_digest);
    }

    #[test]
    fn plan_launch_refuses_multi_slot_and_cluster_truth_mismatches() {
        let cluster_state = sample_cluster_state();
        let sandbox_profile = sample_sandbox_profile();

        let multi_slot = plan_launch(
            &cluster_state,
            &sandbox_profile,
            sample_launch_config(vec![
                DistributedHostfileEntry::new("worker-a").with_slots(2),
            ]),
        )
        .expect_err("multi-slot entries should refuse on the bounded launch surface");
        assert_eq!(
            multi_slot,
            DistributedLaunchError::MultiSlotHostfileEntryUnsupported {
                node_id: String::from("worker-a"),
                slots: 2,
            }
        );

        let address_mismatch = plan_launch(
            &cluster_state,
            &sandbox_profile,
            sample_launch_config(vec![
                DistributedHostfileEntry::new("worker-a").with_advertised_addr("127.0.0.1:9999"),
            ]),
        )
        .expect_err("hostfile addresses must match cluster truth");
        assert_eq!(
            address_mismatch,
            DistributedLaunchError::HostfileAddressMismatch {
                node_id: String::from("worker-a"),
                hostfile_addr: String::from("127.0.0.1:9999"),
                cluster_addr: String::from("127.0.0.1:4101"),
            }
        );
    }

    #[test]
    fn plan_launch_refuses_configs_that_would_fail_the_sandbox_contract() {
        let cluster_state = sample_cluster_state();
        let hostfile = vec![DistributedHostfileEntry::new("worker-a")];

        let command_mismatch = plan_launch(
            &cluster_state,
            &sample_sandbox_profile(),
            sample_launch_config(hostfile.clone())
                .with_entrypoint_type(ProviderSandboxEntrypointType::Command),
        )
        .expect_err("non-container command entrypoints should refuse");
        assert_eq!(
            command_mismatch,
            DistributedLaunchError::CommandEntrypointUnsupported {
                execution_class: ProviderSandboxExecutionClass::PosixExec,
            }
        );

        let compute_product_mismatch = plan_launch(
            &cluster_state,
            &sample_sandbox_profile(),
            sample_launch_config(hostfile.clone())
                .with_compute_product_id("sandbox.container.exec"),
        )
        .expect_err("mismatched compute-product ids should refuse");
        assert_eq!(
            compute_product_mismatch,
            DistributedLaunchError::ComputeProductIdMismatch {
                compute_product_id: String::from("sandbox.container.exec"),
                expected_product_id: String::from("sandbox.posix.exec"),
            }
        );

        let env_forbidden = plan_launch(
            &cluster_state,
            &ProviderSandboxProfile {
                secrets_mode: String::from("none"),
                ..sample_sandbox_profile()
            },
            sample_launch_config(hostfile.clone()),
        )
        .expect_err("profiles that forbid environment injection should refuse");
        assert_eq!(
            env_forbidden,
            DistributedLaunchError::SandboxEnvironmentInjectionForbidden {
                profile_id: String::from("sandbox.posix.v1"),
            }
        );

        let resource_limit = plan_launch(
            &cluster_state,
            &sample_sandbox_profile(),
            sample_launch_config(hostfile).with_resource_request(ProviderSandboxResourceRequest {
                cpu_limit: Some(16),
                memory_limit_mb: None,
                disk_limit_mb: None,
            }),
        )
        .expect_err("resource requests above the sandbox profile should refuse");
        assert_eq!(
            resource_limit,
            DistributedLaunchError::ResourceRequestExceedsSandboxProfile {
                resource: "cpu_limit",
                requested: 16,
                profile_limit: 8,
            }
        );
    }

    #[test]
    fn grouped_all_sum_preserves_tree_structure_and_reduces_grouped_leaves() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let context = ArrayContext::cpu();
        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-b")))
            .expect("bootstrapped mesh should initialize one public group");
        let gradients = Tree::Tuple(vec![
            Tree::Leaf(
                context
                    .constant_f32(Shape::new(vec![2]), vec![1.0, 2.0])
                    .expect("leaf a"),
            ),
            Tree::Dict(BTreeMap::from([
                (
                    String::from("bias"),
                    Tree::Leaf(context.scalar_f32(3.0).expect("leaf b")),
                ),
                (
                    String::from("proj"),
                    Tree::Leaf(
                        context
                            .constant_f32(Shape::new(vec![1]), vec![4.0])
                            .expect("leaf c"),
                    ),
                ),
            ])),
        ]);
        let reduced = grouped_all_sum(
            &gradients,
            DistributedGradientReductionOptions::new()
                .with_group(group)
                .with_small_tensor_bytes_threshold(32)
                .with_rank_input_tree(
                    0,
                    Tree::Tuple(vec![
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![0.5, 1.5])
                                .expect("rank 0 leaf a"),
                        ),
                        Tree::Dict(BTreeMap::from([
                            (
                                String::from("bias"),
                                Tree::Leaf(
                                    DistributedReferenceTensor::f32(Shape::scalar(), vec![1.0])
                                        .expect("rank 0 leaf b"),
                                ),
                            ),
                            (
                                String::from("proj"),
                                Tree::Leaf(
                                    DistributedReferenceTensor::f32(Shape::new(vec![1]), vec![2.0])
                                        .expect("rank 0 leaf c"),
                                ),
                            ),
                        ])),
                    ]),
                )
                .with_rank_input_tree(
                    1,
                    Tree::Tuple(vec![
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![1.0, 2.0])
                                .expect("rank 1 leaf a"),
                        ),
                        Tree::Dict(BTreeMap::from([
                            (
                                String::from("bias"),
                                Tree::Leaf(
                                    DistributedReferenceTensor::f32(Shape::scalar(), vec![3.0])
                                        .expect("rank 1 leaf b"),
                                ),
                            ),
                            (
                                String::from("proj"),
                                Tree::Leaf(
                                    DistributedReferenceTensor::f32(Shape::new(vec![1]), vec![4.0])
                                        .expect("rank 1 leaf c"),
                                ),
                            ),
                        ])),
                    ]),
                )
                .with_rank_input_tree(
                    2,
                    Tree::Tuple(vec![
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![1.0, 1.0])
                                .expect("rank 2 leaf a"),
                        ),
                        Tree::Dict(BTreeMap::from([
                            (
                                String::from("bias"),
                                Tree::Leaf(
                                    DistributedReferenceTensor::f32(Shape::scalar(), vec![2.0])
                                        .expect("rank 2 leaf b"),
                                ),
                            ),
                            (
                                String::from("proj"),
                                Tree::Leaf(
                                    DistributedReferenceTensor::f32(Shape::new(vec![1]), vec![1.0])
                                        .expect("rank 2 leaf c"),
                                ),
                            ),
                        ])),
                    ]),
                )
                .with_rank_input_tree(
                    3,
                    Tree::Tuple(vec![
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![0.5, 0.5])
                                .expect("rank 3 leaf a"),
                        ),
                        Tree::Dict(BTreeMap::from([
                            (
                                String::from("bias"),
                                Tree::Leaf(
                                    DistributedReferenceTensor::f32(Shape::scalar(), vec![3.0])
                                        .expect("rank 3 leaf b"),
                                ),
                            ),
                            (
                                String::from("proj"),
                                Tree::Leaf(
                                    DistributedReferenceTensor::f32(Shape::new(vec![1]), vec![3.0])
                                        .expect("rank 3 leaf c"),
                                ),
                            ),
                        ])),
                    ]),
                ),
        )
        .expect("grouped tree reduction should succeed");

        let flattened = reduced.flatten();
        assert_eq!(flattened.spec, gradients.spec());
        let reduced_values = flattened
            .leaves
            .into_iter()
            .map(|leaf| {
                leaf.to_host_data()
                    .expect("reduced host export")
                    .as_f32_slice()
                    .expect("floating payload")
                    .to_vec()
            })
            .collect::<Vec<_>>();
        assert_eq!(reduced_values, vec![vec![3.0, 5.0], vec![9.0], vec![10.0]]);
    }

    #[test]
    fn average_gradients_divides_grouped_reduction_by_world_size() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let context = ArrayContext::cpu();
        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-a")))
            .expect("bootstrapped mesh should initialize one public group");
        let gradients = Tree::Tuple(vec![
            Tree::Leaf(
                context
                    .constant_f32(Shape::new(vec![2]), vec![4.0, 8.0])
                    .expect("leaf a"),
            ),
            Tree::Leaf(context.scalar_f32(12.0).expect("leaf b")),
        ]);

        let averaged = average_gradients(
            &gradients,
            DistributedGradientReductionOptions::new()
                .with_group(group)
                .with_small_tensor_bytes_threshold(32)
                .with_rank_input_tree(
                    0,
                    Tree::Tuple(vec![
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![4.0, 8.0])
                                .expect("rank 0 leaf a"),
                        ),
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::scalar(), vec![12.0])
                                .expect("rank 0 leaf b"),
                        ),
                    ]),
                )
                .with_rank_input_tree(
                    1,
                    Tree::Tuple(vec![
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![2.0, 6.0])
                                .expect("rank 1 leaf a"),
                        ),
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::scalar(), vec![10.0])
                                .expect("rank 1 leaf b"),
                        ),
                    ]),
                )
                .with_rank_input_tree(
                    2,
                    Tree::Tuple(vec![
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![6.0, 10.0])
                                .expect("rank 2 leaf a"),
                        ),
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::scalar(), vec![14.0])
                                .expect("rank 2 leaf b"),
                        ),
                    ]),
                )
                .with_rank_input_tree(
                    3,
                    Tree::Tuple(vec![
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::new(vec![2]), vec![8.0, 12.0])
                                .expect("rank 3 leaf a"),
                        ),
                        Tree::Leaf(
                            DistributedReferenceTensor::f32(Shape::scalar(), vec![16.0])
                                .expect("rank 3 leaf b"),
                        ),
                    ]),
                ),
        )
        .expect("average_gradients should succeed on floating tree leaves");

        let flattened = averaged.flatten();
        let averaged_values = flattened
            .leaves
            .into_iter()
            .map(|leaf| {
                leaf.to_host_data()
                    .expect("averaged host export")
                    .as_f32_slice()
                    .expect("floating payload")
                    .to_vec()
            })
            .collect::<Vec<_>>();
        assert_eq!(averaged_values, vec![vec![5.0, 9.0], vec![13.0]]);
    }

    #[test]
    fn gradient_reduction_refuses_tree_structure_mismatch_and_non_floating_average() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let context = ArrayContext::cpu();
        let group = init(DistributedInitOptions::new().with_bootstrap(sample_bootstrap("node-c")))
            .expect("bootstrapped mesh should initialize one public group");
        let gradients = Tree::Tuple(vec![Tree::Leaf(
            context
                .constant_f32(Shape::new(vec![1]), vec![1.0])
                .expect("leaf"),
        )]);

        let structure_error = grouped_all_sum(
            &gradients,
            DistributedGradientReductionOptions::new()
                .with_group(group.clone())
                .with_rank_input_tree(
                    0,
                    Tree::List(vec![Tree::Leaf(
                        DistributedReferenceTensor::f32(Shape::new(vec![1]), vec![1.0])
                            .expect("rank 0 leaf"),
                    )]),
                )
                .with_rank_input_tree(
                    1,
                    Tree::Tuple(vec![Tree::Leaf(
                        DistributedReferenceTensor::f32(Shape::new(vec![1]), vec![1.0])
                            .expect("rank 1 leaf"),
                    )]),
                )
                .with_rank_input_tree(
                    2,
                    Tree::Tuple(vec![Tree::Leaf(
                        DistributedReferenceTensor::f32(Shape::new(vec![1]), vec![1.0])
                            .expect("rank 2 leaf"),
                    )]),
                )
                .with_rank_input_tree(
                    3,
                    Tree::Tuple(vec![Tree::Leaf(
                        DistributedReferenceTensor::f32(Shape::new(vec![1]), vec![1.0])
                            .expect("rank 3 leaf"),
                    )]),
                ),
        )
        .expect_err("rank trees must preserve local structure");
        assert_eq!(
            structure_error,
            DistributedGradientReductionError::RankInputTreeStructureMismatch {
                rank: 0,
                expected: TreeSpec::Tuple(vec![TreeSpec::Leaf]),
                actual: TreeSpec::List(vec![TreeSpec::Leaf]),
            }
        );

        let int_gradient = Tree::Leaf(
            context
                .scalar_f32(1.0)
                .expect("int source")
                .cast(DType::I8)
                .expect("int gradient"),
        );
        let non_floating = average_gradients(
            &int_gradient,
            DistributedGradientReductionOptions::new().with_group(group),
        )
        .expect_err("average_gradients should refuse non-floating leaves");
        assert_eq!(
            non_floating,
            DistributedGradientReductionError::NonFloatingGradientLeaf {
                leaf_index: 0,
                dtype: DType::I8,
            }
        );
    }

    #[test]
    fn all_to_sharded_linear_slices_output_rows_into_local_wrappers() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let rank0_group =
            init(DistributedInitOptions::new().with_bootstrap(sample_two_rank_bootstrap("node-a")))
                .expect("rank 0 bootstrap");
        let rank1_group =
            init(DistributedInitOptions::new().with_bootstrap(sample_two_rank_bootstrap("node-b")))
                .expect("rank 1 bootstrap");
        let linear = Linear::from_f32_parts(
            "tp_all_to_sharded",
            2,
            5,
            vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 2.0],
            Some(vec![0.0, 1.0, 2.0, 3.0, 4.0]),
        )
        .expect("full linear");
        let input = NnTensor::f32(Shape::new(vec![1, 2]), vec![1.0, 2.0]).expect("input");
        let full = linear.forward(&input).expect("full output");

        let rank0 = AllToShardedLinear::from_linear(&linear, &rank0_group).expect("rank 0 shard");
        let rank1 = AllToShardedLinear::from_linear(&linear, &rank1_group).expect("rank 1 shard");

        assert_eq!(rank0.layout().shard_start, 0);
        assert_eq!(rank0.layout().shard_end, 3);
        assert_eq!(rank1.layout().shard_start, 3);
        assert_eq!(rank1.layout().shard_end, 5);
        assert_eq!(
            rank0.local_linear().bias_f32().expect("rank 0 bias"),
            Some(&[0.0, 1.0, 2.0][..])
        );
        assert_eq!(
            rank1.local_linear().bias_f32().expect("rank 1 bias"),
            Some(&[3.0, 4.0][..])
        );

        let mut sharded = rank0
            .forward(&input)
            .expect("rank 0 forward")
            .as_f32_slice()
            .expect("rank 0 host")
            .to_vec();
        sharded.extend_from_slice(
            rank1
                .forward(&input)
                .expect("rank 1 forward")
                .as_f32_slice()
                .expect("rank 1 host"),
        );
        assert_eq!(sharded, full.as_f32_slice().expect("full host"));
    }

    #[test]
    fn sharded_to_all_linear_reconstructs_full_output_from_rank_wrappers() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let rank0_group =
            init(DistributedInitOptions::new().with_bootstrap(sample_two_rank_bootstrap("node-a")))
                .expect("rank 0 bootstrap");
        let rank1_group =
            init(DistributedInitOptions::new().with_bootstrap(sample_two_rank_bootstrap("node-b")))
                .expect("rank 1 bootstrap");
        let linear = Linear::from_f32_parts(
            "tp_sharded_to_all",
            5,
            2,
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 4.0, 3.0, 2.0, 1.0],
            Some(vec![10.0, 20.0]),
        )
        .expect("full linear");
        let full_input = NnTensor::f32(Shape::new(vec![1, 5]), vec![1.0, 1.0, 1.0, 1.0, 1.0])
            .expect("full input");
        let full_output = linear.forward(&full_input).expect("full output");

        let rank0 = ShardedToAllLinear::from_linear(&linear, &rank0_group).expect("rank 0 wrapper");
        let rank1 = ShardedToAllLinear::from_linear(&linear, &rank1_group).expect("rank 1 wrapper");
        let rank0_input = rank0.shard_input(&full_input).expect("rank 0 shard input");
        let rank1_input = rank1.shard_input(&full_input).expect("rank 1 shard input");

        assert_eq!(rank0.layout().local_in_features(), 3);
        assert_eq!(rank1.layout().local_in_features(), 2);
        assert!(rank0.local_linear().uses_bias());
        assert!(!rank1.local_linear().uses_bias());

        let mut rank_modules = BTreeMap::new();
        rank_modules.insert(0, rank0.clone());
        rank_modules.insert(1, rank1.clone());

        let mut rank_inputs = BTreeMap::new();
        rank_inputs.insert(0, rank0_input.clone());
        rank_inputs.insert(1, rank1_input.clone());

        let reconstructed = rank0
            .forward_with_options(
                &rank0_input,
                TensorParallelShardedToAllOptions::new()
                    .with_group(rank0_group)
                    .with_rank_modules(rank_modules)
                    .with_rank_inputs(rank_inputs),
            )
            .expect("reference-emulated reconstruction");
        assert_eq!(
            reconstructed.as_f32_slice().expect("reconstructed host"),
            full_output.as_f32_slice().expect("full host"),
        );
    }

    #[test]
    fn tensor_parallel_wrappers_refuse_missing_rank_state_and_tiny_axes() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let rank0_group =
            init(DistributedInitOptions::new().with_bootstrap(sample_two_rank_bootstrap("node-a")))
                .expect("rank 0 bootstrap");
        let rank1_group =
            init(DistributedInitOptions::new().with_bootstrap(sample_two_rank_bootstrap("node-b")))
                .expect("rank 1 bootstrap");
        let linear = Linear::from_f32_parts(
            "tp_sharded_to_all_refusal",
            5,
            2,
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 4.0, 3.0, 2.0, 1.0],
            Some(vec![10.0, 20.0]),
        )
        .expect("full linear");
        let rank0 = ShardedToAllLinear::from_linear(&linear, &rank0_group).expect("rank 0 wrapper");
        let rank1 = ShardedToAllLinear::from_linear(&linear, &rank1_group).expect("rank 1 wrapper");
        let full_input = NnTensor::f32(Shape::new(vec![1, 5]), vec![1.0, 1.0, 1.0, 1.0, 1.0])
            .expect("full input");
        let rank0_input = rank0.shard_input(&full_input).expect("rank 0 shard input");
        let rank1_input = rank1.shard_input(&full_input).expect("rank 1 shard input");

        let direct_error = rank0
            .forward(&rank0_input)
            .expect_err("multi-rank forward should require explicit remote state");
        assert_eq!(
            direct_error,
            TensorParallelLinearError::MultiRankForwardRequiresExplicitInputs {
                group_id: rank0.layout().group.group_id.clone(),
                world_size: 2,
            }
        );

        let mut partial_inputs = BTreeMap::new();
        partial_inputs.insert(0, rank0_input.clone());
        partial_inputs.insert(1, rank1_input);
        let missing_module_error = rank0
            .forward_with_options(
                &rank0_input,
                TensorParallelShardedToAllOptions::new()
                    .with_group(rank0_group)
                    .with_rank_module(0, rank0.clone())
                    .with_rank_inputs(partial_inputs),
            )
            .expect_err("missing remote wrapper should refuse");
        assert_eq!(
            missing_module_error,
            TensorParallelLinearError::MissingRankModules {
                missing_ranks: vec![1]
            }
        );

        let tiny =
            Linear::from_f32_parts("tiny_tp", 1, 2, vec![1.0, 2.0], None).expect("tiny linear");
        let tiny_error = ShardedToAllLinear::from_linear(&tiny, &rank1_group)
            .expect_err("world size larger than input axis should refuse");
        assert_eq!(
            tiny_error,
            TensorParallelLinearError::FeatureAxisTooSmall {
                helper: TensorParallelLinearKind::ShardedToAll,
                axis_size: 1,
                world_size: 2,
            }
        );
    }

    #[test]
    fn fsdp_apply_gradients_updates_mixed_full_shard_and_replicated_groups() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let group =
            init(DistributedInitOptions::new().with_bootstrap(sample_two_rank_bootstrap("node-b")))
                .expect("rank 1 bootstrap");
        let mut local_groups = vec![
            training_group(
                "matrix",
                psionic_train::TrainingParameterClass::Matrix,
                vec![10.0, 20.0, 30.0, 40.0],
                psionic_train::TrainingOptimizerConfig::sgd(0.1),
            ),
            training_group(
                "bias",
                psionic_train::TrainingParameterClass::Bias,
                vec![1.0, 2.0],
                psionic_train::TrainingOptimizerConfig::sgd(0.1),
            ),
        ];
        let remote_groups = local_groups.clone();
        let contract = fsdp_contract(vec![
            full_shard_matrix_contract("matrix", 4),
            replicated_bias_contract("bias", 2),
        ]);
        let local_batch = training_batch(
            "batch.local",
            &[
                ("matrix", vec![1.0, 2.0, 3.0, 4.0]),
                ("bias", vec![0.5, 1.5]),
            ],
        );
        let remote_batch = training_batch(
            "batch.remote",
            &[
                ("matrix", vec![5.0, 6.0, 7.0, 8.0]),
                ("bias", vec![1.5, 2.5]),
            ],
        );

        let receipt = fsdp_apply_gradients(
            local_groups.as_mut_slice(),
            &local_batch,
            &contract,
            FsdpApplyGradientsOptions::new()
                .with_group(group)
                .with_remote_rank_group_states(0, remote_groups)
                .with_remote_rank_batch(0, remote_batch),
        )
        .expect("mixed FSDP update should succeed");

        assert_eq!(receipt.local_rank, 1);
        assert_eq!(receipt.world_size, 2);
        assert_eq!(receipt.batch_id, "batch.local");
        assert!(receipt.global_clipping_scale.is_none());
        assert!((receipt.global_gradient_norm_l2 - 364.0_f32.sqrt()).abs() < 1e-6);

        assert_eq!(
            training_buffer_values("matrix", &local_groups[0].parameter).expect("matrix values"),
            &[9.4, 19.2, 29.0, 38.8],
        );
        assert_eq!(
            training_buffer_values("bias", &local_groups[1].parameter).expect("bias values"),
            &[0.8, 1.6],
        );
        assert_eq!(local_groups[0].applied_steps, 1);
        assert_eq!(local_groups[1].applied_steps, 1);

        let matrix_receipt = receipt
            .groups
            .iter()
            .find(|group| group.group_id == "matrix")
            .expect("matrix receipt");
        assert_eq!(
            matrix_receipt.local_shard_range,
            TrainingShardRange::new(2, 2)
        );
        assert_eq!(
            matrix_receipt.parameter_shard_kind,
            TrainingParameterShardKind::FullShard
        );
        assert_eq!(
            matrix_receipt.optimizer_state_shard_kind,
            TrainingOptimizerStateShardKind::ZeroStage3
        );
        assert_eq!(matrix_receipt.residency_transitions.len(), 2);

        let bias_receipt = receipt
            .groups
            .iter()
            .find(|group| group.group_id == "bias")
            .expect("bias receipt");
        assert_eq!(
            bias_receipt.local_shard_range,
            TrainingShardRange::new(0, 2)
        );
        assert_eq!(
            bias_receipt.parameter_shard_kind,
            TrainingParameterShardKind::Replicated
        );
        assert_eq!(
            bias_receipt.optimizer_state_shard_kind,
            TrainingOptimizerStateShardKind::Replicated
        );
    }

    #[test]
    fn fsdp_apply_gradients_applies_global_norm_clip_before_local_update() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let group =
            init(DistributedInitOptions::new().with_bootstrap(sample_two_rank_bootstrap("node-a")))
                .expect("rank 0 bootstrap");
        let mut local_groups = vec![training_group(
            "matrix",
            psionic_train::TrainingParameterClass::Matrix,
            vec![10.0, 20.0, 30.0, 40.0],
            psionic_train::TrainingOptimizerConfig::sgd(0.1),
        )];
        let remote_groups = local_groups.clone();
        let contract = fsdp_contract(vec![full_shard_matrix_contract("matrix", 4)]);
        let local_batch =
            training_batch("batch.local.clip", &[("matrix", vec![3.0, 4.0, 0.0, 0.0])]);
        let remote_batch =
            training_batch("batch.remote.clip", &[("matrix", vec![0.0, 0.0, 0.0, 0.0])]);

        let receipt = fsdp_apply_gradients(
            local_groups.as_mut_slice(),
            &local_batch,
            &contract,
            FsdpApplyGradientsOptions::new()
                .with_group(group)
                .with_remote_rank_group_states(1, remote_groups)
                .with_remote_rank_batch(1, remote_batch)
                .with_clip_global_norm(2.5),
        )
        .expect("global-norm clipping should succeed");

        assert!((receipt.global_gradient_norm_l2 - 5.0).abs() < 1e-6);
        assert_eq!(receipt.global_clipping_scale, Some(0.5));
        assert_eq!(
            training_buffer_values("matrix", &local_groups[0].parameter).expect("matrix values"),
            &[9.85, 19.8, 30.0, 40.0],
        );
        assert!((receipt.groups[0].local_shard_gradient_norm_l2 - 2.5).abs() < 1e-6);
    }

    #[test]
    fn fsdp_apply_gradients_refuses_invalid_remote_maps_and_unsupported_layouts() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let group =
            init(DistributedInitOptions::new().with_bootstrap(sample_two_rank_bootstrap("node-a")))
                .expect("rank 0 bootstrap");
        let mut local_groups = vec![training_group(
            "matrix",
            psionic_train::TrainingParameterClass::Matrix,
            vec![10.0, 20.0, 30.0, 40.0],
            psionic_train::TrainingOptimizerConfig::sgd(0.1),
        )];
        let contract = fsdp_contract(vec![full_shard_matrix_contract("matrix", 4)]);
        let local_batch = training_batch(
            "batch.local.refusal",
            &[("matrix", vec![1.0, 1.0, 1.0, 1.0])],
        );
        let remote_groups = local_groups.clone();
        let remote_batch = training_batch(
            "batch.remote.refusal",
            &[("matrix", vec![1.0, 1.0, 1.0, 1.0])],
        );

        let invalid_rank_error = fsdp_apply_gradients(
            local_groups.as_mut_slice(),
            &local_batch,
            &contract,
            FsdpApplyGradientsOptions::new()
                .with_group(group.clone())
                .with_remote_rank_group_states(0, remote_groups.clone())
                .with_remote_rank_batch(1, remote_batch.clone()),
        )
        .expect_err("local rank should not appear in the remote-state map");
        assert!(matches!(
            invalid_rank_error,
            FsdpApplyGradientsError::InvalidRemoteRanks {
                kind: "state",
                invalid_ranks,
                world_size: 2,
            } if invalid_ranks == vec![0]
        ));

        let bad_contract = fsdp_contract(vec![DistributedOptimizerGroupContract::new(
            "matrix",
            psionic_train::TrainingParameterClass::Matrix,
            psionic_train::TrainingParameterShardLayout::new(
                TrainingParameterShardKind::FullShard,
                two_rank_full_shard_placements(4),
            ),
            psionic_train::TrainingParameterShardLayout::new(
                TrainingParameterShardKind::Replicated,
                two_rank_replicated_placements(4),
            ),
            psionic_train::TrainingOptimizerStateShardLayout::new(
                TrainingOptimizerStateShardKind::ZeroStage3,
                psionic_train::TrainingOptimizerShardResidency::DeviceResident,
                two_rank_full_shard_placements(4),
            ),
            OptimizerStateResidency::Offloaded,
        )]);
        let unsupported_layout_error = fsdp_apply_gradients(
            local_groups.as_mut_slice(),
            &local_batch,
            &bad_contract,
            FsdpApplyGradientsOptions::new()
                .with_group(group)
                .with_remote_rank_group_states(1, remote_groups)
                .with_remote_rank_batch(1, remote_batch),
        )
        .expect_err("unsupported layout combinations should refuse");
        assert!(matches!(
            unsupported_layout_error,
            FsdpApplyGradientsError::UnsupportedGroupLayout {
                group_id,
                parameter_kind: TrainingParameterShardKind::FullShard,
                gradient_kind: TrainingParameterShardKind::Replicated,
                optimizer_state_kind: TrainingOptimizerStateShardKind::ZeroStage3,
            } if group_id == "matrix"
        ));
    }
}
