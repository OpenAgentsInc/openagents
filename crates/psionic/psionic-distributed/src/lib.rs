//! Public framework-distributed semantics above Psionic mesh truth.
//!
//! This crate intentionally lands the first bounded framework-distributed
//! slices: explicit group initialization from current mesh facts, honest
//! singleton fallback, global-group reuse, plan-backed subgroup split
//! semantics, and a reference-first collective helper layer above that group
//! surface. Backend-family transport mapping and broader helper families still
//! land later.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};

use psionic_array::{Array, ArrayContext, ArrayError};
use psionic_core::{DType, Shape};
use psionic_runtime::{
    ClusterCommunicationClass, TrainingDeviceMeshContext, TrainingElasticMembershipContext,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "public framework-distributed groups and bounded collective helpers above Psionic runtime mesh truth";

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
            local_node_id: local_node_id.into(),
            members,
        }
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
    /// Named backend-family mapping work remains open.
    #[error(
        "public distributed group init cannot bootstrap backend `{backend}` yet; backend-family mapping remains later work"
    )]
    BackendFamilyMappingPending {
        /// Requested backend family.
        backend: DistributedBackend,
    },
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

#[derive(Clone, Debug)]
struct DistributedGroupState {
    group_id: String,
    kind: DistributedGroupKind,
    requested_backend: DistributedBackend,
    mesh: TrainingDeviceMeshContext,
    local_node_id: String,
    members: Vec<DistributedGroupMember>,
    local_rank: usize,
    parent_group_id: Option<String>,
}

#[derive(Default)]
struct GlobalDistributedGroups {
    groups: BTreeMap<DistributedBackend, Arc<DistributedGroupState>>,
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
                &members,
                None,
            ),
            kind: DistributedGroupKind::SingletonFallback,
            requested_backend,
            mesh,
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
        DistributedGroupSnapshot {
            group_id: self.state.group_id.clone(),
            kind: self.state.kind,
            requested_backend: self.state.requested_backend,
            effective_backend: self.state.mesh.effective_backend.clone(),
            communication_class: self.state.mesh.communication_class,
            mesh_id: self.state.mesh.mesh_id.clone(),
            mesh_revision: self.state.mesh.mesh_revision,
            local_node_id: self.state.local_node_id.clone(),
            rank: self.state.local_rank,
            size: self.state.members.len(),
            members: self.state.members.clone(),
            parent_group_id: self.state.parent_group_id.clone(),
        }
    }

    /// Returns the current capability snapshot for public collective helpers on this group.
    #[must_use]
    pub fn collective_support(&self) -> DistributedCollectiveSupport {
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
                    "Singleton passthrough for all_sum, all_gather, and reduce_scatter is real today; send and recv still refuse because there is no peer process in the group.",
                ),
            )
        } else {
            (
                DistributedCollectiveSupportStatus::ReferenceEmulation,
                DistributedCollectiveSupportStatus::ReferenceEmulation,
                DistributedCollectiveSupportStatus::ReferenceEmulation,
                DistributedCollectiveSupportStatus::ValidationOnly,
                DistributedCollectiveSupportStatus::ReferenceEmulation,
                String::from(
                    "Multi-rank collective helpers are currently bounded to explicit host-owned reference payloads above the group surface; backend transport execution and backend-family mapping remain later work.",
                ),
            )
        };
        DistributedCollectiveSupport {
            group_id: self.state.group_id.clone(),
            group_kind: self.state.kind,
            requested_backend: self.state.requested_backend,
            effective_backend: self.state.mesh.effective_backend.clone(),
            backend_transport_available: false,
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
                &subgroup_members,
                Some(self.state.group_id.as_str()),
            ),
            kind: DistributedGroupKind::SplitSubgroup,
            requested_backend: self.state.requested_backend,
            mesh: subgroup_mesh,
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
        if options.backend != DistributedBackend::Any {
            return Err(DistributedInitError::BackendFamilyMappingPending {
                backend: options.backend,
            });
        }
        let state = Arc::new(build_bootstrapped_state(options.backend, bootstrap)?);
        lock_global_groups()
            .groups
            .insert(options.backend, Arc::clone(&state));
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

fn resolve_collective_group(
    group: Option<DistributedGroup>,
) -> Result<DistributedGroup, DistributedCollectiveError> {
    match group {
        Some(group) => Ok(group),
        None => Ok(init(DistributedInitOptions::new())?),
    }
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
    let (mesh, members, local_rank, local_node_id) = validate_bootstrap(bootstrap)?;
    Ok(DistributedGroupState {
        group_id: stable_group_id(
            DistributedGroupKind::BootstrappedMesh,
            requested_backend,
            &mesh,
            &members,
            None,
        ),
        kind: DistributedGroupKind::BootstrappedMesh,
        requested_backend,
        mesh,
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
    Ok((bootstrap.mesh, members, local_rank, bootstrap.local_node_id))
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

fn sorted_distinct_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn global_groups() -> &'static Mutex<GlobalDistributedGroups> {
    static GLOBAL_GROUPS: OnceLock<Mutex<GlobalDistributedGroups>> = OnceLock::new();
    GLOBAL_GROUPS.get_or_init(|| Mutex::new(GlobalDistributedGroups::default()))
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
    use std::collections::BTreeMap;

    use psionic_array::ArrayContext;
    use psionic_core::{DType, Shape};

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
        DistributedGroupBootstrap::new(
            sample_mesh(),
            local_node_id,
            vec![
                DistributedGroupMember::new("node-a", 0, 0, "cuda:0"),
                DistributedGroupMember::new("node-b", 1, 1, "cuda:1"),
                DistributedGroupMember::new("node-c", 2, 2, "cuda:2"),
                DistributedGroupMember::new("node-d", 3, 3, "cuda:3"),
            ],
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
    fn named_backend_families_stay_unavailable_until_mapping_work_lands() {
        let _test_guard = lock_distributed_test();
        clear_global_groups();

        let error = init(
            DistributedInitOptions::new()
                .with_backend(DistributedBackend::Ring)
                .with_bootstrap(sample_bootstrap("node-a")),
        )
        .expect_err("named backend families should still refuse during the bounded group issue");

        assert_eq!(
            error,
            DistributedInitError::BackendFamilyMappingPending {
                backend: DistributedBackend::Ring
            }
        );
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
}
