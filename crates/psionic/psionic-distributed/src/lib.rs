//! Public framework-distributed group semantics above Psionic mesh truth.
//!
//! This crate intentionally lands only the first bounded distributed slice:
//! explicit group initialization from current mesh facts, honest singleton
//! fallback, global-group reuse, and plan-backed subgroup split semantics.
//! Collective helpers still land later on top of this group surface.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};

use psionic_runtime::{
    ClusterCommunicationClass, TrainingDeviceMeshContext, TrainingElasticMembershipContext,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "public framework-distributed groups above Psionic runtime mesh truth";

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
mod tests {
    use super::*;

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
}
