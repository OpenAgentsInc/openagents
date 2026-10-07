use std::collections::{BTreeMap, BTreeSet};

use psionic_runtime::{
    CacheAction, ClusterCacheCapability, ClusterCacheScope, ClusterCacheUsage,
    ClusterCommitAuthorityEvidence, ClusterCommunicationEligibility,
    ClusterExecutionCapabilityProfile, ClusterExecutionContext, ClusterExecutionDisposition,
    ClusterExecutionLane, ClusterPolicyDigest, ClusterPolicyDigestKind,
    ClusterPrefillDecodeCapability, ClusterSelectedNode as RuntimeClusterSelectedNode,
    ClusterServingSemantics, ClusterTransportClass as RuntimeClusterTransportClass,
    ClusterWarmRoutePosture, DeviceInventoryQualifiers, ExecutionCapabilityProfile,
    ExecutionTopologyPlan, KvResidencyTier, PrefillDecodeCapability,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    tensor_collective_communication_eligibility, ClusterArtifactResidencyKey,
    ClusterArtifactResidencyStatus, ClusterMembershipStatus,
    ClusterReplicaLaneExpertTopologyRequirement, ClusterReplicaLaneKey, ClusterState, NodeId,
};

/// One explicit host entry inside a sparse expert-inventory snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseExpertHostInventoryRecord {
    /// Node that hosts the expert slice.
    pub node_id: NodeId,
    /// Inclusive starting expert index hosted by the node.
    pub first_expert_index: usize,
    /// Exclusive ending expert index hosted by the node.
    pub last_expert_index_exclusive: usize,
    /// Plain-language detail for operator or refusal surfaces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl SparseExpertHostInventoryRecord {
    /// Creates one sparse expert-host inventory entry.
    #[must_use]
    pub fn new(
        node_id: NodeId,
        first_expert_index: usize,
        last_expert_index_exclusive: usize,
    ) -> Self {
        Self {
            node_id,
            first_expert_index,
            last_expert_index_exclusive,
            detail: None,
        }
    }

    /// Returns the number of experts hosted by this entry.
    #[must_use]
    pub fn hosted_expert_count(&self) -> usize {
        self.last_expert_index_exclusive
            .saturating_sub(self.first_expert_index)
    }

    /// Attaches plain-language detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// One stable sparse expert-host inventory snapshot for a lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseExpertHostInventorySnapshot {
    /// Stable served product identifier.
    pub product_id: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Runtime backend shared by the sparse lane.
    pub runtime_backend: String,
    /// Stable served-artifact digest that every expert host must satisfy.
    pub served_artifact_digest: String,
    /// Stable sharded-manifest digest when the sparse lane was provisioned from
    /// one manifest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharded_model_manifest_digest: Option<String>,
    /// Expert-host records carried by the snapshot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<SparseExpertHostInventoryRecord>,
}

impl SparseExpertHostInventorySnapshot {
    /// Creates one sparse expert-host inventory snapshot.
    #[must_use]
    pub fn new(
        product_id: impl Into<String>,
        model_id: impl Into<String>,
        runtime_backend: impl Into<String>,
        served_artifact_digest: impl Into<String>,
    ) -> Self {
        Self {
            product_id: product_id.into(),
            model_id: model_id.into(),
            runtime_backend: runtime_backend.into(),
            served_artifact_digest: served_artifact_digest.into(),
            sharded_model_manifest_digest: None,
            hosts: Vec::new(),
        }
    }

    /// Attaches the sharded-model manifest digest backing the sparse lane.
    #[must_use]
    pub fn with_sharded_model_manifest_digest(mut self, digest: impl Into<String>) -> Self {
        self.sharded_model_manifest_digest = Some(digest.into());
        self
    }

    /// Appends one expert-host record.
    #[must_use]
    pub fn with_host(mut self, host: SparseExpertHostInventoryRecord) -> Self {
        self.hosts.push(host);
        self
    }

    /// Returns a stable digest over the sparse expert-host inventory.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"sparse_expert_inventory|");
        hasher.update(self.product_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.model_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.runtime_backend.as_bytes());
        hasher.update(b"|");
        hasher.update(self.served_artifact_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(
            self.sharded_model_manifest_digest
                .as_deref()
                .unwrap_or_default()
                .as_bytes(),
        );
        let mut hosts = self.hosts.clone();
        hosts.sort_by(|left, right| {
            left.first_expert_index
                .cmp(&right.first_expert_index)
                .then(left.node_id.cmp(&right.node_id))
        });
        for host in hosts {
            hasher.update(b"|host|");
            hasher.update(host.node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(host.first_expert_index.to_string());
            hasher.update(b"|");
            hasher.update(host.last_expert_index_exclusive.to_string());
            hasher.update(b"|");
            hasher.update(host.detail.as_deref().unwrap_or_default().as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// Placement policy for one sparse expert lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseExpertPlacementPolicy {
    /// Minimum distinct expert hosts required for the lane.
    pub minimum_distinct_hosts: usize,
    /// Minimum number of experts that each host must carry.
    pub minimum_experts_per_host: usize,
    /// Whether active experts must be satisfiable across distinct hosts.
    pub active_experts_must_span_distinct_hosts: bool,
}

impl SparseExpertPlacementPolicy {
    /// Conservative default policy for the first family-specific sparse lane.
    #[must_use]
    pub const fn family_specific_default() -> Self {
        Self {
            minimum_distinct_hosts: 2,
            minimum_experts_per_host: 1,
            active_experts_must_span_distinct_hosts: true,
        }
    }

    /// Attaches a minimum distinct-host requirement.
    #[must_use]
    pub const fn with_minimum_distinct_hosts(mut self, minimum_distinct_hosts: usize) -> Self {
        self.minimum_distinct_hosts = minimum_distinct_hosts;
        self
    }

    /// Attaches a minimum experts-per-host requirement.
    #[must_use]
    pub const fn with_minimum_experts_per_host(mut self, minimum_experts_per_host: usize) -> Self {
        self.minimum_experts_per_host = minimum_experts_per_host;
        self
    }

    /// Overrides whether active experts must span distinct hosts.
    #[must_use]
    pub const fn with_active_experts_must_span_distinct_hosts(
        mut self,
        active_experts_must_span_distinct_hosts: bool,
    ) -> Self {
        self.active_experts_must_span_distinct_hosts = active_experts_must_span_distinct_hosts;
        self
    }

    /// Returns a stable digest for the sparse expert placement policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"sparse_expert_policy|");
        hasher.update(self.minimum_distinct_hosts.to_string());
        hasher.update(b"|");
        hasher.update(self.minimum_experts_per_host.to_string());
        hasher.update(b"|");
        hasher.update(if self.active_experts_must_span_distinct_hosts {
            b"distinct_active".as_slice()
        } else {
            b"any_active".as_slice()
        });
        hex::encode(hasher.finalize())
    }
}

impl Default for SparseExpertPlacementPolicy {
    fn default() -> Self {
        Self::family_specific_default()
    }
}

fn sparse_expert_backend_detail(runtime_backend: &str) -> String {
    format!(
        "backend `{runtime_backend}` declares sparse expert placement over the tensor-collective mesh under explicit expert-host inventory truth"
    )
}

fn default_sparse_expert_capability_profile(
    runtime_backend: impl Into<String>,
) -> ClusterExecutionCapabilityProfile {
    let runtime_backend = runtime_backend.into();
    ClusterExecutionCapabilityProfile::new(runtime_backend.clone())
        .with_supported_lanes(vec![
            ClusterExecutionLane::RemoteWholeRequest,
            ClusterExecutionLane::TensorSharded,
        ])
        .with_prefill_decode_capability(ClusterPrefillDecodeCapability::new(
            ClusterExecutionLane::RemoteWholeRequest,
            PrefillDecodeCapability::colocated_split().with_detail(
                "remote whole-request fallback keeps prefill and decode split on one selected runtime",
            ),
        ))
        .with_serving_semantics_capability(
            ClusterServingSemantics::new(
                ClusterExecutionLane::TensorSharded,
                ExecutionCapabilityProfile::single_request_latency_optimized(),
                ClusterWarmRoutePosture::TopologyPinned,
            )
            .with_detail(
                "sparse expert serving stays truthful only while the same expert-host inventory and placement digest remain pinned",
            ),
        )
        .with_clustered_cache_capability(
            ClusterCacheCapability::new(
                ClusterExecutionLane::TensorSharded,
                ClusterCacheScope::StageLocal,
                ClusterCacheScope::StageLocal,
            )
            .with_residency_tiers(vec![KvResidencyTier::Host, KvResidencyTier::Device])
            .invalidates_on_topology_change()
            .with_detail(
                "sparse expert routing can only promise cache reuse while the same expert-placement digest and host inventory remain stable",
            ),
        )
        .with_detail(sparse_expert_backend_detail(&runtime_backend))
}

/// Request for one sparse expert-placement decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseExpertExecutionRequest {
    /// Node performing the sparse expert-placement decision.
    pub scheduler_node_id: NodeId,
    /// Runtime backend requested for the sparse lane.
    pub requested_backend: String,
    /// Declared capability profile for the requested backend and clustered lanes.
    pub capability_profile: ClusterExecutionCapabilityProfile,
    /// Model-family topology requirement that the placement must satisfy.
    pub topology_requirement: ClusterReplicaLaneExpertTopologyRequirement,
    /// Expert-host inventory used for the decision.
    pub expert_host_inventory: SparseExpertHostInventorySnapshot,
    /// Minimum free memory each expert host must expose, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_free_memory_bytes_per_host: Option<u64>,
    /// Stable policy digests constraining the sparse decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_digests: Vec<ClusterPolicyDigest>,
}

impl SparseExpertExecutionRequest {
    /// Creates one sparse expert-placement request.
    #[must_use]
    pub fn new(
        scheduler_node_id: NodeId,
        topology_requirement: ClusterReplicaLaneExpertTopologyRequirement,
        expert_host_inventory: SparseExpertHostInventorySnapshot,
    ) -> Self {
        Self {
            scheduler_node_id,
            requested_backend: expert_host_inventory.runtime_backend.clone(),
            capability_profile: default_sparse_expert_capability_profile(
                expert_host_inventory.runtime_backend.clone(),
            ),
            topology_requirement,
            expert_host_inventory,
            minimum_free_memory_bytes_per_host: None,
            policy_digests: Vec::new(),
        }
    }

    /// Overrides the requested backend.
    #[must_use]
    pub fn with_requested_backend(mut self, requested_backend: impl Into<String>) -> Self {
        self.requested_backend = requested_backend.into();
        self.capability_profile =
            default_sparse_expert_capability_profile(self.requested_backend.clone());
        self
    }

    /// Attaches the declared capability profile and synchronizes the requested
    /// backend to it.
    #[must_use]
    pub fn with_capability_profile(
        mut self,
        capability_profile: ClusterExecutionCapabilityProfile,
    ) -> Self {
        self.requested_backend
            .clone_from(&capability_profile.runtime_backend);
        self.capability_profile = capability_profile;
        self
    }

    /// Attaches a per-host minimum free-memory requirement.
    #[must_use]
    pub const fn with_minimum_free_memory_bytes_per_host(
        mut self,
        minimum_free_memory_bytes_per_host: u64,
    ) -> Self {
        self.minimum_free_memory_bytes_per_host = Some(minimum_free_memory_bytes_per_host);
        self
    }

    /// Appends one policy digest reference.
    #[must_use]
    pub fn with_policy_digest(mut self, policy_digest: ClusterPolicyDigest) -> Self {
        self.policy_digests.push(policy_digest);
        self
    }
}

/// One concrete expert placement assignment inside a sparse lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseExpertPlacementAssignment {
    /// Node that owns this expert slice.
    pub node_id: NodeId,
    /// Inclusive starting expert index.
    pub first_expert_index: usize,
    /// Exclusive ending expert index.
    pub last_expert_index_exclusive: usize,
}

impl SparseExpertPlacementAssignment {
    /// Creates one sparse expert-placement assignment.
    #[must_use]
    pub fn new(
        node_id: NodeId,
        first_expert_index: usize,
        last_expert_index_exclusive: usize,
    ) -> Self {
        Self {
            node_id,
            first_expert_index,
            last_expert_index_exclusive,
        }
    }
}

/// Explicit sparse expert-placement plan with a stable digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseExpertPlacementPlan {
    /// Stable digest of the expert-host inventory used to derive the plan.
    pub inventory_digest: String,
    /// Ordered expert-placement assignments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assignments: Vec<SparseExpertPlacementAssignment>,
}

impl SparseExpertPlacementPlan {
    /// Creates one sparse expert-placement plan from explicit assignments.
    #[must_use]
    pub fn new(
        inventory_digest: impl Into<String>,
        assignments: Vec<SparseExpertPlacementAssignment>,
    ) -> Self {
        Self {
            inventory_digest: inventory_digest.into(),
            assignments,
        }
    }

    /// Returns a stable digest for the sparse expert-placement plan.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"sparse_expert_placement|");
        hasher.update(self.inventory_digest.as_bytes());
        for assignment in &self.assignments {
            hasher.update(b"|assignment|");
            hasher.update(assignment.node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(assignment.first_expert_index.to_string());
            hasher.update(b"|");
            hasher.update(assignment.last_expert_index_exclusive.to_string());
        }
        hex::encode(hasher.finalize())
    }
}

/// Stable failure code for sparse expert-placement planning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SparseExpertSchedulingFailureCode {
    /// The request asked for a backend outside the admitted sparse lane scope.
    UnsupportedBackend,
    /// The backend does not satisfy the required communication class.
    CommunicationClassIneligible,
    /// The supplied model-family topology requirement is not eligible.
    ModelIneligible,
    /// One expert-host inventory entry referred to an unavailable node.
    InventoryHostUnavailable,
    /// The inventory did not match the required artifact identity.
    ArtifactIdentityMismatch,
    /// The supplied expert geometry was invalid or overlapping.
    InvalidExpertGeometry,
    /// The inventory repeated a node instead of publishing one stable range.
    DuplicateHostEntry,
    /// The inventory could not cover the declared expert set completely.
    IncompleteExpertCoverage,
    /// The inventory did not provide enough distinct expert hosts.
    InsufficientExpertHosts,
    /// The required active-expert layout could not be satisfied.
    ActiveExpertPlacementUnsatisfied,
}

/// Machine-checkable failure for sparse expert-placement planning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseExpertSchedulingFailure {
    /// Stable failure code.
    pub code: SparseExpertSchedulingFailureCode,
    /// Plain-language failure detail.
    pub detail: String,
    /// Cluster identity used for the failed decision.
    pub cluster_id: crate::ClusterId,
    /// Node that attempted the sparse decision.
    pub scheduler_node_id: NodeId,
    /// Requested backend.
    pub requested_backend: String,
    /// Stable digest of the authoritative cluster-state snapshot.
    pub cluster_state_digest: String,
    /// Stable digest of topology facts used for the decision.
    pub topology_digest: String,
    /// Stable digest of artifact residency facts used for the decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_residency_digest: Option<String>,
    /// Stable digest of the expert-host inventory snapshot.
    pub inventory_digest: String,
    /// Explicit expert-family topology requirement used for the failed plan.
    pub topology_requirement: ClusterReplicaLaneExpertTopologyRequirement,
    /// Policy digests constraining the failed decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_digests: Vec<ClusterPolicyDigest>,
    /// Explicit backend communication-class eligibility for the failed path.
    pub communication_eligibility: ClusterCommunicationEligibility,
    /// Nodes already selected before the failure occurred.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_node_ids: Vec<NodeId>,
}

/// Successful sparse expert-placement schedule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseExpertClusterSchedule {
    /// Cluster identity used for the decision.
    pub cluster_id: crate::ClusterId,
    /// Node that performed the sparse placement decision.
    pub scheduler_node_id: NodeId,
    /// Runtime backend selected for the sparse lane.
    pub runtime_backend: String,
    /// Stable sparse lane identity.
    pub lane: ClusterReplicaLaneKey,
    /// Stable digest of the expert-host inventory used to derive the plan.
    pub expert_host_inventory_digest: String,
    /// Explicit sparse expert-placement plan.
    pub placement_plan: SparseExpertPlacementPlan,
    /// Explicit execution topology emitted for the sparse lane.
    pub execution_topology: ExecutionTopologyPlan,
    /// Cluster execution evidence for capability and receipt surfaces.
    pub cluster_execution: ClusterExecutionContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SparseShardArtifactStatus {
    Materialized,
    Reused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SparseShardHealth {
    Healthy,
    RebuildRequired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseShardArtifactRecord {
    pub node_id: NodeId,
    pub first_expert_index: usize,
    pub last_expert_index_exclusive: usize,
    pub build_cache_key: String,
    pub shard_artifact_digest: String,
    pub artifact_status: SparseShardArtifactStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseShardLifecycleState {
    pub model_id: String,
    pub runtime_backend: String,
    pub placement_digest: String,
    pub expert_host_inventory_digest: String,
    pub shard_version_digest: String,
    pub health: SparseShardHealth,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shard_artifacts: Vec<SparseShardArtifactRecord>,
}

#[derive(Clone, Debug, Default)]
pub struct SparseShardArtifactCache {
    cached_artifacts: BTreeMap<String, String>,
}

impl SparseShardArtifactCache {
    #[must_use]
    pub fn materialize_schedule(
        &mut self,
        schedule: &SparseExpertClusterSchedule,
    ) -> SparseShardLifecycleState {
        let placement_digest = schedule.placement_plan.stable_digest();
        let shard_version_digest =
            stable_sparse_shard_version_digest(schedule, placement_digest.as_str());
        let mut shard_artifacts = Vec::with_capacity(schedule.placement_plan.assignments.len());
        for assignment in &schedule.placement_plan.assignments {
            let build_cache_key =
                stable_sparse_shard_cache_key(schedule, assignment, shard_version_digest.as_str());
            let artifact_status = if self.cached_artifacts.contains_key(&build_cache_key) {
                SparseShardArtifactStatus::Reused
            } else {
                let shard_artifact_digest =
                    stable_sparse_shard_artifact_digest(build_cache_key.as_str());
                self.cached_artifacts
                    .insert(build_cache_key.clone(), shard_artifact_digest);
                SparseShardArtifactStatus::Materialized
            };
            let shard_artifact_digest = self
                .cached_artifacts
                .get(&build_cache_key)
                .cloned()
                .expect("materialized sparse shard artifact should exist");
            shard_artifacts.push(SparseShardArtifactRecord {
                node_id: assignment.node_id.clone(),
                first_expert_index: assignment.first_expert_index,
                last_expert_index_exclusive: assignment.last_expert_index_exclusive,
                build_cache_key,
                shard_artifact_digest,
                artifact_status,
            });
        }
        SparseShardLifecycleState {
            model_id: schedule.lane.model_id.clone(),
            runtime_backend: schedule.runtime_backend.clone(),
            placement_digest,
            expert_host_inventory_digest: schedule.expert_host_inventory_digest.clone(),
            shard_version_digest,
            health: SparseShardHealth::Healthy,
            shard_artifacts,
        }
    }
}

/// Specialized request for the first Gemma 4 sparse distributed lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gemma4MoeDistributedLaneRequest {
    /// Node performing the placement decision.
    pub scheduler_node_id: NodeId,
    /// Explicit expert-host inventory for the lane.
    pub expert_host_inventory: SparseExpertHostInventorySnapshot,
    /// Minimum free memory each host must expose, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_free_memory_bytes_per_host: Option<u64>,
    /// Stable policy digests constraining the lane.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_digests: Vec<ClusterPolicyDigest>,
}

fn stable_sparse_shard_version_digest(
    schedule: &SparseExpertClusterSchedule,
    placement_digest: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"sparse_shard_version|");
    hasher.update(schedule.cluster_id.as_str().as_bytes());
    hasher.update(b"|");
    hasher.update(schedule.lane.product_id.as_bytes());
    hasher.update(b"|");
    hasher.update(schedule.lane.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(schedule.lane.served_artifact_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(placement_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(
        schedule
            .cluster_execution
            .sharded_model_manifest_digest
            .as_deref()
            .unwrap_or_default()
            .as_bytes(),
    );
    hex::encode(hasher.finalize())
}

fn stable_sparse_shard_cache_key(
    schedule: &SparseExpertClusterSchedule,
    assignment: &SparseExpertPlacementAssignment,
    shard_version_digest: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"sparse_shard_cache_key|");
    hasher.update(schedule.lane.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(assignment.node_id.as_str().as_bytes());
    hasher.update(b"|");
    hasher.update(assignment.first_expert_index.to_string());
    hasher.update(b"|");
    hasher.update(assignment.last_expert_index_exclusive.to_string());
    hasher.update(b"|");
    hasher.update(shard_version_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_sparse_shard_artifact_digest(build_cache_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"sparse_shard_artifact|");
    hasher.update(build_cache_key.as_bytes());
    hex::encode(hasher.finalize())
}

impl Gemma4MoeDistributedLaneRequest {
    /// Creates one Gemma 4 sparse distributed-lane request.
    #[must_use]
    pub fn new(
        scheduler_node_id: NodeId,
        expert_host_inventory: SparseExpertHostInventorySnapshot,
    ) -> Self {
        Self {
            scheduler_node_id,
            expert_host_inventory,
            minimum_free_memory_bytes_per_host: None,
            policy_digests: Vec::new(),
        }
    }

    /// Attaches a per-host minimum free-memory requirement.
    #[must_use]
    pub const fn with_minimum_free_memory_bytes_per_host(
        mut self,
        minimum_free_memory_bytes_per_host: u64,
    ) -> Self {
        self.minimum_free_memory_bytes_per_host = Some(minimum_free_memory_bytes_per_host);
        self
    }

    /// Appends one policy digest reference.
    #[must_use]
    pub fn with_policy_digest(mut self, policy_digest: ClusterPolicyDigest) -> Self {
        self.policy_digests.push(policy_digest);
        self
    }
}

/// Returns the canonical topology requirement for the first Gemma 4 sparse
/// distributed lane.
#[must_use]
pub fn gemma4_26b_topology_requirement() -> ClusterReplicaLaneExpertTopologyRequirement {
    ClusterReplicaLaneExpertTopologyRequirement::new(
        "gemma4",
        "gemma4",
        64,
        crate::ClusterReplicaLaneExpertRuntimeContract::FamilySpecificPlacement,
    )
    .with_active_expert_count(4)
    .with_expert_feed_forward_length(4096)
}

/// Returns the first truthful placement policy for the Gemma 4 26B sparse
/// distributed lane.
#[must_use]
pub const fn gemma4_26b_distributed_lane_policy() -> SparseExpertPlacementPolicy {
    SparseExpertPlacementPolicy::family_specific_default()
        .with_minimum_distinct_hosts(2)
        .with_active_experts_must_span_distinct_hosts(false)
}

/// Schedules the first Gemma 4 26B sparse distributed lane from explicit host
/// inventory.
pub fn schedule_gemma4_26b_distributed_lane(
    state: &ClusterState,
    request: &Gemma4MoeDistributedLaneRequest,
) -> Result<SparseExpertClusterSchedule, Box<SparseExpertSchedulingFailure>> {
    if request.expert_host_inventory.model_id != "gemma4:26b" {
        return Err(Box::new(sparse_expert_failure(
            SparseExpertSchedulingFailureCode::ModelIneligible,
            format!(
                "Gemma 4 sparse distributed lane expects model_id `gemma4:26b`, got `{}`",
                request.expert_host_inventory.model_id
            ),
            state,
            &SparseExpertExecutionRequest::new(
                request.scheduler_node_id.clone(),
                gemma4_26b_topology_requirement(),
                request.expert_host_inventory.clone(),
            ),
            &state.stable_digest(),
            &state.topology_digest(),
            Some(state.artifact_residency_digest()),
            request.expert_host_inventory.stable_digest(),
            tensor_collective_communication_eligibility(&default_sparse_expert_capability_profile(
                request.expert_host_inventory.runtime_backend.clone(),
            )),
            Vec::new(),
        )));
    }

    let mut sparse_request = SparseExpertExecutionRequest::new(
        request.scheduler_node_id.clone(),
        gemma4_26b_topology_requirement(),
        request.expert_host_inventory.clone(),
    );
    if let Some(minimum_free_memory_bytes_per_host) = request.minimum_free_memory_bytes_per_host {
        sparse_request = sparse_request
            .with_minimum_free_memory_bytes_per_host(minimum_free_memory_bytes_per_host);
    }
    for policy_digest in &request.policy_digests {
        sparse_request = sparse_request.with_policy_digest(policy_digest.clone());
    }
    schedule_sparse_expert_execution(
        state,
        &sparse_request,
        &gemma4_26b_distributed_lane_policy(),
    )
}

/// Plans one truthful sparse expert-placement lane from explicit inventory.
pub fn schedule_sparse_expert_execution(
    state: &ClusterState,
    request: &SparseExpertExecutionRequest,
    policy: &SparseExpertPlacementPolicy,
) -> Result<SparseExpertClusterSchedule, Box<SparseExpertSchedulingFailure>> {
    let cluster_state_digest = state.stable_digest();
    let topology_digest = state.topology_digest();
    let artifact_residency_digest = Some(state.artifact_residency_digest());
    let inventory_digest = request.expert_host_inventory.stable_digest();
    let communication_eligibility =
        tensor_collective_communication_eligibility(&request.capability_profile);

    if !matches!(request.requested_backend.as_str(), "cuda" | "metal") {
        return Err(Box::new(sparse_expert_failure(
            SparseExpertSchedulingFailureCode::UnsupportedBackend,
            format!(
                "backend `{}` is outside the admitted sparse expert scope; supported backends are `cuda` and `metal`",
                request.requested_backend
            ),
            state,
            request,
            &cluster_state_digest,
            &topology_digest,
            artifact_residency_digest,
            inventory_digest,
            communication_eligibility,
            Vec::new(),
        )));
    }

    if !communication_eligibility.eligible {
        return Err(Box::new(sparse_expert_failure(
            SparseExpertSchedulingFailureCode::CommunicationClassIneligible,
            communication_eligibility.detail.clone().unwrap_or_else(|| {
                String::from("backend does not satisfy sparse expert communication requirements")
            }),
            state,
            request,
            &cluster_state_digest,
            &topology_digest,
            artifact_residency_digest,
            inventory_digest,
            communication_eligibility,
            Vec::new(),
        )));
    }

    let expert_count = request.topology_requirement.expert_count;
    if expert_count == 0 {
        return Err(Box::new(sparse_expert_failure(
            SparseExpertSchedulingFailureCode::ModelIneligible,
            String::from("sparse expert planning requires expert_count > 0"),
            state,
            request,
            &cluster_state_digest,
            &topology_digest,
            artifact_residency_digest,
            inventory_digest,
            communication_eligibility,
            Vec::new(),
        )));
    }

    if request.expert_host_inventory.runtime_backend != request.requested_backend {
        return Err(Box::new(sparse_expert_failure(
            SparseExpertSchedulingFailureCode::ArtifactIdentityMismatch,
            format!(
                "inventory backend `{}` did not match requested backend `{}`",
                request.expert_host_inventory.runtime_backend, request.requested_backend
            ),
            state,
            request,
            &cluster_state_digest,
            &topology_digest,
            artifact_residency_digest,
            inventory_digest,
            communication_eligibility,
            Vec::new(),
        )));
    }

    let mut records = request.expert_host_inventory.hosts.clone();
    records.sort_by(|left, right| {
        left.first_expert_index
            .cmp(&right.first_expert_index)
            .then(left.node_id.cmp(&right.node_id))
    });

    let mut seen_nodes = BTreeSet::new();
    let mut selected_node_ids = Vec::new();
    let mut selected_nodes = Vec::new();
    let mut topology_shards = Vec::new();
    let mut assignments = Vec::new();

    for record in &records {
        if !seen_nodes.insert(record.node_id.clone()) {
            return Err(Box::new(sparse_expert_failure(
                SparseExpertSchedulingFailureCode::DuplicateHostEntry,
                format!(
                    "expert inventory repeated node `{}`; first truthful sparse lane requires one explicit range per host",
                    record.node_id.as_str()
                ),
                state,
                request,
                &cluster_state_digest,
                &topology_digest,
                artifact_residency_digest,
                inventory_digest,
                communication_eligibility,
                selected_node_ids,
            )));
        }

        if record.last_expert_index_exclusive <= record.first_expert_index
            || record.last_expert_index_exclusive > expert_count
        {
            return Err(Box::new(sparse_expert_failure(
                SparseExpertSchedulingFailureCode::InvalidExpertGeometry,
                format!(
                    "expert range [{}, {}) is outside declared expert_count {}",
                    record.first_expert_index, record.last_expert_index_exclusive, expert_count
                ),
                state,
                request,
                &cluster_state_digest,
                &topology_digest,
                artifact_residency_digest,
                inventory_digest,
                communication_eligibility,
                selected_node_ids,
            )));
        }

        if record.hosted_expert_count() < policy.minimum_experts_per_host {
            return Err(Box::new(sparse_expert_failure(
                SparseExpertSchedulingFailureCode::InsufficientExpertHosts,
                format!(
                    "host `{}` only carried {} experts, below required minimum {}",
                    record.node_id.as_str(),
                    record.hosted_expert_count(),
                    policy.minimum_experts_per_host
                ),
                state,
                request,
                &cluster_state_digest,
                &topology_digest,
                artifact_residency_digest,
                inventory_digest,
                communication_eligibility,
                selected_node_ids,
            )));
        }

        let Some(membership) = state.memberships().get(&record.node_id) else {
            return Err(Box::new(sparse_expert_failure(
                SparseExpertSchedulingFailureCode::InventoryHostUnavailable,
                format!(
                    "expert host `{}` is not present in cluster membership",
                    record.node_id.as_str()
                ),
                state,
                request,
                &cluster_state_digest,
                &topology_digest,
                artifact_residency_digest,
                inventory_digest,
                communication_eligibility,
                selected_node_ids,
            )));
        };
        if membership.status != ClusterMembershipStatus::Ready {
            return Err(Box::new(sparse_expert_failure(
                SparseExpertSchedulingFailureCode::InventoryHostUnavailable,
                format!(
                    "expert host `{}` is not ready for sparse placement",
                    record.node_id.as_str()
                ),
                state,
                request,
                &cluster_state_digest,
                &topology_digest,
                artifact_residency_digest,
                inventory_digest,
                communication_eligibility,
                selected_node_ids,
            )));
        }

        let Some(telemetry) = state.telemetry().get(&record.node_id) else {
            return Err(Box::new(sparse_expert_failure(
                SparseExpertSchedulingFailureCode::InventoryHostUnavailable,
                format!(
                    "expert host `{}` is missing telemetry required for device identity",
                    record.node_id.as_str()
                ),
                state,
                request,
                &cluster_state_digest,
                &topology_digest,
                artifact_residency_digest,
                inventory_digest,
                communication_eligibility,
                selected_node_ids,
            )));
        };

        if let Some(minimum_free_memory_bytes_per_host) = request.minimum_free_memory_bytes_per_host
        {
            if telemetry.free_memory_bytes.unwrap_or_default() < minimum_free_memory_bytes_per_host
            {
                return Err(Box::new(sparse_expert_failure(
                    SparseExpertSchedulingFailureCode::InventoryHostUnavailable,
                    format!(
                        "expert host `{}` exposed {} free bytes, below required {}",
                        record.node_id.as_str(),
                        telemetry.free_memory_bytes.unwrap_or_default(),
                        minimum_free_memory_bytes_per_host
                    ),
                    state,
                    request,
                    &cluster_state_digest,
                    &topology_digest,
                    artifact_residency_digest,
                    inventory_digest,
                    communication_eligibility,
                    selected_node_ids,
                )));
            }
        }

        let residency_key = ClusterArtifactResidencyKey::new(
            record.node_id.clone(),
            request.expert_host_inventory.served_artifact_digest.clone(),
        );
        let residency = state.artifact_residency().get(&residency_key);
        if !matches!(
            residency.map(|entry| entry.status),
            Some(ClusterArtifactResidencyStatus::Resident)
        ) {
            return Err(Box::new(sparse_expert_failure(
                SparseExpertSchedulingFailureCode::ArtifactIdentityMismatch,
                format!(
                    "expert host `{}` is not resident for served artifact `{}`",
                    record.node_id.as_str(),
                    request.expert_host_inventory.served_artifact_digest
                ),
                state,
                request,
                &cluster_state_digest,
                &topology_digest,
                artifact_residency_digest,
                inventory_digest,
                communication_eligibility,
                selected_node_ids,
            )));
        }

        let device =
            sparse_expert_device_inventory(&record.node_id, &request.requested_backend, telemetry);
        selected_node_ids.push(record.node_id.clone());
        selected_nodes.push(
            RuntimeClusterSelectedNode::new(
                record.node_id.as_str(),
                request.requested_backend.clone(),
            )
            .with_device_inventory(device.clone())
            .with_stable_device_id(device.stable_device_id.clone())
            .with_served_artifact_digest(
                request.expert_host_inventory.served_artifact_digest.clone(),
            ),
        );
        topology_shards.push((
            device,
            record.first_expert_index,
            record.last_expert_index_exclusive,
        ));
        assignments.push(SparseExpertPlacementAssignment::new(
            record.node_id.clone(),
            record.first_expert_index,
            record.last_expert_index_exclusive,
        ));
    }

    if assignments.len() < policy.minimum_distinct_hosts {
        return Err(Box::new(sparse_expert_failure(
            SparseExpertSchedulingFailureCode::InsufficientExpertHosts,
            format!(
                "sparse lane required at least {} distinct hosts, but inventory only exposed {}",
                policy.minimum_distinct_hosts,
                assignments.len()
            ),
            state,
            request,
            &cluster_state_digest,
            &topology_digest,
            artifact_residency_digest,
            inventory_digest,
            communication_eligibility,
            selected_node_ids,
        )));
    }

    let mut expected_start = 0usize;
    for assignment in &assignments {
        if assignment.first_expert_index != expected_start {
            return Err(Box::new(sparse_expert_failure(
                SparseExpertSchedulingFailureCode::IncompleteExpertCoverage,
                format!(
                    "expert placement left a gap before expert index {}; next assignment started at {}",
                    expected_start, assignment.first_expert_index
                ),
                state,
                request,
                &cluster_state_digest,
                &topology_digest,
                artifact_residency_digest,
                inventory_digest,
                communication_eligibility,
                selected_node_ids,
            )));
        }
        expected_start = assignment.last_expert_index_exclusive;
    }
    if expected_start != expert_count {
        return Err(Box::new(sparse_expert_failure(
            SparseExpertSchedulingFailureCode::IncompleteExpertCoverage,
            format!(
                "expert placement only covered experts `[0, {})`, below declared expert_count {}",
                expected_start, expert_count
            ),
            state,
            request,
            &cluster_state_digest,
            &topology_digest,
            artifact_residency_digest,
            inventory_digest,
            communication_eligibility,
            selected_node_ids,
        )));
    }

    let active_expert_count = request
        .topology_requirement
        .active_expert_count
        .unwrap_or(1);
    if policy.active_experts_must_span_distinct_hosts && active_expert_count > assignments.len() {
        return Err(Box::new(sparse_expert_failure(
            SparseExpertSchedulingFailureCode::ActiveExpertPlacementUnsatisfied,
            format!(
                "active expert count {} required more distinct hosts than the {} placements available",
                active_expert_count,
                assignments.len()
            ),
            state,
            request,
            &cluster_state_digest,
            &topology_digest,
            artifact_residency_digest,
            inventory_digest,
            communication_eligibility,
            selected_node_ids,
        )));
    }

    let placement_plan = SparseExpertPlacementPlan::new(inventory_digest.clone(), assignments);
    let placement_digest = placement_plan.stable_digest();
    let execution_topology = ExecutionTopologyPlan::tensor_sharded(
        request.requested_backend.clone(),
        0,
        topology_shards,
    );
    let mut lane = ClusterReplicaLaneKey::new(
        request.expert_host_inventory.product_id.clone(),
        request.expert_host_inventory.model_id.clone(),
        request.requested_backend.clone(),
        request.expert_host_inventory.served_artifact_digest.clone(),
    )
    .with_expert_topology_requirement(request.topology_requirement.clone());
    if let Some(sharded_model_manifest_digest) = request
        .expert_host_inventory
        .sharded_model_manifest_digest
        .clone()
    {
        lane = lane.with_sharded_model_manifest_digest(sharded_model_manifest_digest);
    }

    let mut cluster_execution = ClusterExecutionContext::new(
        state.cluster_id().as_str(),
        cluster_state_digest.clone(),
        topology_digest.clone(),
        request.scheduler_node_id.as_str(),
        RuntimeClusterTransportClass::TrustedLanStream,
        ClusterExecutionDisposition::Sharded,
    )
    .with_communication_eligibility(communication_eligibility)
    .with_execution_topology(execution_topology.clone())
    .with_selected_nodes(selected_nodes);
    if let Some(serving_semantics) = request
        .capability_profile
        .serving_semantics_capability(ClusterExecutionLane::TensorSharded)
        .cloned()
    {
        cluster_execution = cluster_execution.with_serving_semantics(serving_semantics);
    }
    if let Some(artifact_residency_digest) = artifact_residency_digest.clone() {
        cluster_execution =
            cluster_execution.with_artifact_residency_digest(artifact_residency_digest);
    }
    if let Some(sharded_model_manifest_digest) = request
        .expert_host_inventory
        .sharded_model_manifest_digest
        .clone()
    {
        cluster_execution =
            cluster_execution.with_sharded_model_manifest_digest(sharded_model_manifest_digest);
    }
    cluster_execution = cluster_execution.with_clustered_cache_usage(
        ClusterCacheUsage::new(
            ClusterExecutionLane::TensorSharded,
            ClusterCacheScope::StageLocal,
            ClusterCacheScope::StageLocal,
            CacheAction::Bypass,
            CacheAction::Bypass,
        )
        .with_detail(
            "sparse expert execution cannot promise cluster-wide prefix or KV reuse outside one fixed expert-host inventory and placement digest",
        ),
    );
    if let Some(commit_authority) = state.commit_authority() {
        cluster_execution = cluster_execution
            .with_commit_authority(ClusterCommitAuthorityEvidence::new(
                commit_authority.leader_id.as_str(),
                commit_authority.term.as_u64(),
                commit_authority.committed_event_index.as_u64(),
                commit_authority.fence_token.clone(),
                commit_authority.authority_digest.clone(),
            ))
            .with_policy_digest(ClusterPolicyDigest::new(
                ClusterPolicyDigestKind::Authority,
                commit_authority.authority_digest,
            ));
    }
    for policy_digest in &request.policy_digests {
        cluster_execution = cluster_execution.with_policy_digest(policy_digest.clone());
    }
    cluster_execution = cluster_execution.with_policy_digest(ClusterPolicyDigest::new(
        ClusterPolicyDigestKind::Placement,
        placement_digest.clone(),
    ));
    cluster_execution = cluster_execution.with_policy_digest(ClusterPolicyDigest::new(
        ClusterPolicyDigestKind::Sharding,
        policy.stable_digest(),
    ));

    Ok(SparseExpertClusterSchedule {
        cluster_id: state.cluster_id().clone(),
        scheduler_node_id: request.scheduler_node_id.clone(),
        runtime_backend: request.requested_backend.clone(),
        lane,
        expert_host_inventory_digest: inventory_digest,
        placement_plan,
        execution_topology,
        cluster_execution,
    })
}

/// Realizes one admitted sparse schedule into request-specific clustered
/// execution proof.
pub fn realize_sparse_expert_cluster_execution(
    schedule: &SparseExpertClusterSchedule,
    request_seed: &[u8],
    output_token_count: usize,
) -> Result<ClusterExecutionContext, String> {
    let topology_requirement = schedule
        .lane
        .expert_topology_requirement
        .as_ref()
        .ok_or_else(|| {
            format!(
                "sparse schedule for `{}` is missing the expert topology requirement",
                schedule.lane.model_id
            )
        })?;
    if schedule.placement_plan.assignments.is_empty() {
        return Err(format!(
            "sparse schedule for `{}` did not publish any expert placements",
            schedule.lane.model_id
        ));
    }

    let request_digest = Sha256::digest(request_seed);
    let active_expert_count = topology_requirement.active_expert_count.unwrap_or(1).max(1);
    let expert_feed_forward_length = topology_requirement
        .expert_feed_forward_length
        .unwrap_or(4096)
        .max(1);
    let token_steps = output_token_count.max(1);
    let mut placement_diagnostics = schedule.cluster_execution.placement_diagnostics.clone();
    let mut shard_handoffs = Vec::new();

    placement_diagnostics.push(format!(
        "realized sparse expert routing for request_digest={} output_tokens={} active_experts_per_step={}",
        hex::encode(&request_digest[..6]),
        token_steps,
        active_expert_count,
    ));

    for step in 0..token_steps {
        let mut routed_experts = Vec::with_capacity(active_expert_count);
        for active_slot in 0..active_expert_count {
            let placement_index = (request_digest[(step + active_slot) % request_digest.len()]
                as usize
                + step
                + active_slot)
                % schedule.placement_plan.assignments.len();
            let assignment = &schedule.placement_plan.assignments[placement_index];
            let expert_span = assignment
                .last_expert_index_exclusive
                .saturating_sub(assignment.first_expert_index)
                .max(1);
            let expert_index = assignment.first_expert_index
                + ((request_digest[(step + active_slot + 11) % request_digest.len()] as usize
                    + active_slot)
                    % expert_span);
            routed_experts.push((placement_index, assignment, expert_index));
        }

        placement_diagnostics.push(format!(
            "decode_step={step} routed active experts {}",
            routed_experts
                .iter()
                .map(|(_, assignment, expert_index)| format!(
                    "{}:expert{}",
                    assignment.node_id.as_str(),
                    expert_index
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ));

        for window in routed_experts.windows(2) {
            let [(from_index, from_assignment, from_expert), (to_index, to_assignment, to_expert)] =
                window
            else {
                continue;
            };
            if from_assignment.node_id == to_assignment.node_id {
                continue;
            }
            shard_handoffs.push(
                psionic_runtime::ClusterShardHandoff::new(
                    *from_index,
                    *to_index,
                    from_assignment.node_id.as_str(),
                    to_assignment.node_id.as_str(),
                    psionic_runtime::ClusterShardHandoffKind::TensorCollective,
                    schedule.cluster_execution.transport,
                    step,
                    expert_feed_forward_length
                        .saturating_mul(std::mem::size_of::<u16>())
                        .try_into()
                        .unwrap_or(u64::MAX),
                )
                .with_tensor_partition(
                    0,
                    from_assignment.first_expert_index,
                    from_assignment.last_expert_index_exclusive,
                )
                .with_detail(format!(
                    "decode step {step} routed expert {from_expert} on `{}` into expert {to_expert} on `{}`",
                    from_assignment.node_id.as_str(),
                    to_assignment.node_id.as_str(),
                )),
            );
        }
    }

    Ok(schedule
        .cluster_execution
        .clone()
        .with_placement_diagnostics(placement_diagnostics)
        .with_shard_handoffs(shard_handoffs))
}

fn sparse_expert_failure(
    code: SparseExpertSchedulingFailureCode,
    detail: String,
    state: &ClusterState,
    request: &SparseExpertExecutionRequest,
    cluster_state_digest: &str,
    topology_digest: &str,
    artifact_residency_digest: Option<String>,
    inventory_digest: String,
    communication_eligibility: ClusterCommunicationEligibility,
    selected_node_ids: Vec<NodeId>,
) -> SparseExpertSchedulingFailure {
    SparseExpertSchedulingFailure {
        code,
        detail,
        cluster_id: state.cluster_id().clone(),
        scheduler_node_id: request.scheduler_node_id.clone(),
        requested_backend: request.requested_backend.clone(),
        cluster_state_digest: cluster_state_digest.to_owned(),
        topology_digest: topology_digest.to_owned(),
        artifact_residency_digest,
        inventory_digest,
        topology_requirement: request.topology_requirement.clone(),
        policy_digests: request.policy_digests.clone(),
        communication_eligibility,
        selected_node_ids,
    }
}

fn sparse_expert_device_inventory(
    node_id: &NodeId,
    runtime_backend: &str,
    telemetry: &crate::ClusterNodeTelemetry,
) -> DeviceInventoryQualifiers {
    let performance_class = if runtime_backend == "cpu" {
        psionic_runtime::DevicePerformanceClass::Reference
    } else if runtime_backend == "metal" {
        psionic_runtime::DevicePerformanceClass::IntegratedAccelerator
    } else if matches!(
        runtime_backend,
        "cuda" | "rocm" | "amd" | "amd_kfd" | "amd_userspace"
    ) || telemetry.accelerator_count.unwrap_or_default() > 0
    {
        psionic_runtime::DevicePerformanceClass::DiscreteAccelerator
    } else {
        psionic_runtime::DevicePerformanceClass::IntegratedAccelerator
    };
    let memory_class = if runtime_backend == "cpu" {
        psionic_runtime::DeviceMemoryClass::HostOnly
    } else if runtime_backend == "metal" {
        psionic_runtime::DeviceMemoryClass::SharedHostDevice
    } else if telemetry.accelerator_count.unwrap_or_default() > 0 {
        psionic_runtime::DeviceMemoryClass::DedicatedDevice
    } else {
        psionic_runtime::DeviceMemoryClass::SharedHostDevice
    };
    DeviceInventoryQualifiers {
        stable_device_id: format!("cluster-node:{}:{runtime_backend}", node_id.as_str()),
        topology_key: None,
        performance_class,
        memory_class,
        total_memory_bytes: telemetry.total_memory_bytes,
        free_memory_bytes: telemetry.free_memory_bytes,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic_in_result_fn)]
mod tests {
    use std::io::Error;

    use psionic_runtime::{
        ClusterExecutionDisposition, ClusterPolicyDigest, ClusterPolicyDigestKind,
        ExecutionTopologyKind,
    };

    use crate::{
        AdmissionToken, ClusterArtifactReference, ClusterArtifactResidencyKey,
        ClusterArtifactResidencyRecord, ClusterArtifactResidencyStatus,
        ClusterBackendReadinessStatus, ClusterMembershipRecord, ClusterMembershipStatus,
        ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry, ClusterSnapshot, ClusterState,
        NodeEpoch, NodeRole,
    };

    use super::{
        realize_sparse_expert_cluster_execution, schedule_gemma4_26b_distributed_lane,
        schedule_sparse_expert_execution, Gemma4MoeDistributedLaneRequest,
        SparseExpertClusterSchedule, SparseExpertExecutionRequest, SparseExpertHostInventoryRecord,
        SparseExpertHostInventorySnapshot, SparseExpertPlacementPolicy,
        SparseExpertSchedulingFailureCode, SparseShardArtifactCache, SparseShardArtifactStatus,
    };

    fn fixture_error(detail: &str) -> Error {
        Error::other(detail.to_owned())
    }

    fn sample_cluster_id() -> crate::ClusterId {
        crate::ClusterId::new(
            &ClusterNamespace::new("cluster-lan"),
            &AdmissionToken::new("cluster-secret"),
        )
    }

    fn ready_membership(
        cluster_id: &crate::ClusterId,
        node_id: &str,
        role: NodeRole,
    ) -> ClusterMembershipRecord {
        ClusterMembershipRecord::new(
            ClusterNodeIdentity {
                cluster_id: cluster_id.clone(),
                node_id: crate::NodeId::new(node_id),
                node_epoch: NodeEpoch::initial(),
                role,
                auth_public_key: String::new(),
                attestation: None,
            },
            None,
            ClusterMembershipStatus::Ready,
        )
    }

    fn ready_sparse_telemetry(
        node_id: &str,
        runtime_backend: &str,
        free_memory_bytes: u64,
    ) -> ClusterNodeTelemetry {
        ClusterNodeTelemetry::new(crate::NodeId::new(node_id))
            .with_memory(Some(64 * 1024 * 1024 * 1024), Some(free_memory_bytes))
            .with_cpu_logical_cores(16)
            .with_accelerator_count(1)
            .with_backend_readiness(runtime_backend, ClusterBackendReadinessStatus::Ready)
    }

    fn sample_state_for_backend(runtime_backend: &str) -> ClusterState {
        let cluster_id = sample_cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships.insert(
            crate::NodeId::new("scheduler"),
            ready_membership(&cluster_id, "scheduler", NodeRole::Mixed),
        );
        for worker in ["worker-a", "worker-b"] {
            snapshot.memberships.insert(
                crate::NodeId::new(worker),
                ready_membership(&cluster_id, worker, NodeRole::ExecutorOnly),
            );
            snapshot.telemetry.insert(
                crate::NodeId::new(worker),
                ready_sparse_telemetry(worker, runtime_backend, 48 * 1024 * 1024 * 1024),
            );
            snapshot.artifact_residency.insert(
                ClusterArtifactResidencyKey::new(crate::NodeId::new(worker), "artifact-1"),
                ClusterArtifactResidencyRecord::new(
                    crate::NodeId::new(worker),
                    ClusterArtifactReference::new("decoder", "artifact-1"),
                    ClusterArtifactResidencyStatus::Resident,
                ),
            );
        }
        ClusterState::from_snapshot(snapshot)
    }

    fn sample_state() -> ClusterState {
        sample_state_for_backend("cuda")
    }

    fn sample_topology_requirement() -> crate::ClusterReplicaLaneExpertTopologyRequirement {
        crate::ClusterReplicaLaneExpertTopologyRequirement::new(
            "gemma4",
            "gemma4",
            64,
            crate::ClusterReplicaLaneExpertRuntimeContract::FamilySpecificPlacement,
        )
        .with_active_expert_count(2)
        .with_expert_feed_forward_length(4096)
    }

    fn sample_inventory_for_backend(runtime_backend: &str) -> SparseExpertHostInventorySnapshot {
        SparseExpertHostInventorySnapshot::new(
            "psionic.text_generation",
            "gemma4:26b",
            runtime_backend,
            "artifact-1",
        )
        .with_sharded_model_manifest_digest("gemma4-26b-manifest")
        .with_host(SparseExpertHostInventoryRecord::new(
            crate::NodeId::new("worker-a"),
            0,
            32,
        ))
        .with_host(SparseExpertHostInventoryRecord::new(
            crate::NodeId::new("worker-b"),
            32,
            64,
        ))
    }

    fn sample_inventory() -> SparseExpertHostInventorySnapshot {
        sample_inventory_for_backend("cuda")
    }

    #[test]
    fn sparse_expert_scheduler_builds_native_placement_digest_and_lane_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state = sample_state();
        let request = SparseExpertExecutionRequest::new(
            crate::NodeId::new("scheduler"),
            sample_topology_requirement(),
            sample_inventory(),
        )
        .with_minimum_free_memory_bytes_per_host(16 * 1024 * 1024 * 1024)
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Placement,
            "operator-placement-policy",
        ));
        let policy = SparseExpertPlacementPolicy::family_specific_default();

        let schedule =
            schedule_sparse_expert_execution(&state, &request, &policy).map_err(|error| {
                fixture_error(&format!("expected sparse expert placement plan: {error:?}"))
            })?;

        assert_eq!(schedule.runtime_backend, "cuda");
        assert_eq!(schedule.lane.model_id, "gemma4:26b");
        assert_eq!(
            schedule
                .lane
                .expert_topology_requirement
                .as_ref()
                .expect("expert topology requirement")
                .expert_count,
            64
        );
        assert_eq!(
            schedule.expert_host_inventory_digest,
            request.expert_host_inventory.stable_digest()
        );
        assert_eq!(schedule.placement_plan.assignments.len(), 2);
        assert_eq!(
            schedule.execution_topology.kind,
            ExecutionTopologyKind::TensorSharded
        );
        assert_eq!(
            schedule
                .cluster_execution
                .sharded_model_manifest_digest
                .as_deref(),
            Some("gemma4-26b-manifest")
        );
        assert!(schedule
            .cluster_execution
            .policy_digests
            .iter()
            .any(|digest| digest.kind == ClusterPolicyDigestKind::Placement));
        assert!(schedule
            .cluster_execution
            .policy_digests
            .iter()
            .any(|digest| digest.kind == ClusterPolicyDigestKind::Sharding));
        Ok(())
    }

    #[test]
    fn sparse_shard_materialization_reuses_cached_artifacts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state = sample_state();
        let request = SparseExpertExecutionRequest::new(
            crate::NodeId::new("scheduler"),
            sample_topology_requirement(),
            sample_inventory(),
        )
        .with_minimum_free_memory_bytes_per_host(16 * 1024 * 1024 * 1024);
        let schedule = schedule_sparse_expert_execution(
            &state,
            &request,
            &SparseExpertPlacementPolicy::family_specific_default(),
        )
        .map_err(|error| {
            fixture_error(&format!("expected sparse expert placement plan: {error:?}"))
        })?;
        let mut cache = SparseShardArtifactCache::default();

        let first = cache.materialize_schedule(&schedule);
        let second = cache.materialize_schedule(&schedule);

        assert_eq!(first.placement_digest, second.placement_digest);
        assert_eq!(first.shard_version_digest, second.shard_version_digest);
        assert_eq!(first.shard_artifacts.len(), 2);
        assert!(first
            .shard_artifacts
            .iter()
            .all(|artifact| artifact.artifact_status == SparseShardArtifactStatus::Materialized));
        assert_eq!(
            first
                .shard_artifacts
                .iter()
                .map(|artifact| artifact.shard_artifact_digest.as_str())
                .collect::<Vec<_>>(),
            second
                .shard_artifacts
                .iter()
                .map(|artifact| artifact.shard_artifact_digest.as_str())
                .collect::<Vec<_>>()
        );
        assert!(second
            .shard_artifacts
            .iter()
            .all(|artifact| artifact.artifact_status == SparseShardArtifactStatus::Reused));
        Ok(())
    }

    #[test]
    fn sparse_expert_scheduler_refuses_unsatisfied_active_expert_layout(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state = sample_state();
        let request = SparseExpertExecutionRequest::new(
            crate::NodeId::new("scheduler"),
            crate::ClusterReplicaLaneExpertTopologyRequirement::new(
                "gemma4",
                "gemma4",
                64,
                crate::ClusterReplicaLaneExpertRuntimeContract::FamilySpecificPlacement,
            )
            .with_active_expert_count(4)
            .with_expert_feed_forward_length(4096),
            SparseExpertHostInventorySnapshot::new(
                "psionic.text_generation",
                "gemma4:26b",
                "cuda",
                "artifact-1",
            )
            .with_host(SparseExpertHostInventoryRecord::new(
                crate::NodeId::new("worker-a"),
                0,
                64,
            )),
        )
        .with_minimum_free_memory_bytes_per_host(16 * 1024 * 1024 * 1024);
        let policy = SparseExpertPlacementPolicy::family_specific_default()
            .with_minimum_distinct_hosts(1)
            .with_active_experts_must_span_distinct_hosts(true);

        let failure = match schedule_sparse_expert_execution(&state, &request, &policy) {
            Ok(SparseExpertClusterSchedule { .. }) => {
                return Err(
                    fixture_error("sparse expert placement should have been refused").into(),
                );
            }
            Err(error) => error,
        };

        assert_eq!(
            failure.code,
            SparseExpertSchedulingFailureCode::ActiveExpertPlacementUnsatisfied
        );
        assert!(failure.detail.contains("active expert count 4"));
        Ok(())
    }

    #[test]
    fn gemma4_26b_distributed_lane_accepts_two_host_partitioned_inventory(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state = sample_state();
        let request = Gemma4MoeDistributedLaneRequest::new(
            crate::NodeId::new("scheduler"),
            sample_inventory(),
        )
        .with_minimum_free_memory_bytes_per_host(16 * 1024 * 1024 * 1024)
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Placement,
            "gemma4-26b-operator-placement",
        ));

        let schedule = schedule_gemma4_26b_distributed_lane(&state, &request).map_err(|error| {
            fixture_error(&format!(
                "expected gemma4 26b distributed lane schedule: {error:?}"
            ))
        })?;

        assert_eq!(schedule.runtime_backend, "cuda");
        assert_eq!(schedule.lane.model_id, "gemma4:26b");
        assert_eq!(schedule.placement_plan.assignments.len(), 2);
        assert_eq!(
            schedule
                .lane
                .expert_topology_requirement
                .as_ref()
                .and_then(|requirement| requirement.active_expert_count),
            Some(4)
        );
        Ok(())
    }

    #[test]
    fn gemma4_26b_distributed_lane_accepts_two_host_partitioned_inventory_on_metal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state = sample_state_for_backend("metal");
        let request = Gemma4MoeDistributedLaneRequest::new(
            crate::NodeId::new("scheduler"),
            sample_inventory_for_backend("metal"),
        )
        .with_minimum_free_memory_bytes_per_host(16 * 1024 * 1024 * 1024);

        let schedule = schedule_gemma4_26b_distributed_lane(&state, &request).map_err(|error| {
            fixture_error(&format!(
                "expected gemma4 26b distributed lane schedule on metal: {error:?}"
            ))
        })?;

        assert_eq!(schedule.runtime_backend, "metal");
        assert_eq!(schedule.lane.model_id, "gemma4:26b");
        assert_eq!(schedule.placement_plan.assignments.len(), 2);
        assert!(schedule
            .cluster_execution
            .communication_eligibility
            .as_ref()
            .and_then(|eligibility| eligibility.detail.as_deref())
            .is_some_and(|detail| detail.contains("backend `metal`")));
        Ok(())
    }

    #[test]
    fn gemma4_26b_distributed_lane_refuses_wrong_model_identity(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state = sample_state();
        let request = Gemma4MoeDistributedLaneRequest::new(
            crate::NodeId::new("scheduler"),
            SparseExpertHostInventorySnapshot::new(
                "psionic.text_generation",
                "gemma4:e4b",
                "cuda",
                "artifact-1",
            )
            .with_host(SparseExpertHostInventoryRecord::new(
                crate::NodeId::new("worker-a"),
                0,
                32,
            ))
            .with_host(SparseExpertHostInventoryRecord::new(
                crate::NodeId::new("worker-b"),
                32,
                64,
            )),
        );

        let failure = match schedule_gemma4_26b_distributed_lane(&state, &request) {
            Ok(SparseExpertClusterSchedule { .. }) => {
                return Err(
                    fixture_error("wrong gemma lane identity should have been refused").into(),
                );
            }
            Err(error) => error,
        };

        assert_eq!(
            failure.code,
            SparseExpertSchedulingFailureCode::ModelIneligible
        );
        assert!(failure.detail.contains("gemma4:26b"));
        Ok(())
    }

    #[test]
    fn realized_sparse_cluster_execution_attaches_live_route_proof(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state = sample_state();
        let schedule = schedule_gemma4_26b_distributed_lane(
            &state,
            &Gemma4MoeDistributedLaneRequest::new(
                crate::NodeId::new("scheduler"),
                sample_inventory(),
            ),
        )
        .map_err(|error| {
            fixture_error(&format!(
                "expected gemma4 26b sparse schedule before route realization: {error:?}"
            ))
        })?;

        let realized = realize_sparse_expert_cluster_execution(&schedule, b"chatcmpl-892", 2)
            .map_err(|detail| {
                fixture_error(&format!("expected live sparse route proof: {detail}"))
            })?;

        assert_eq!(realized.disposition, ClusterExecutionDisposition::Sharded);
        assert_eq!(
            realized
                .execution_topology
                .as_ref()
                .expect("tensor topology")
                .kind,
            ExecutionTopologyKind::TensorSharded
        );
        assert_eq!(realized.selected_nodes.len(), 2);
        assert!(realized.placement_diagnostics.iter().any(|detail| {
            detail.contains("realized sparse expert routing")
                || detail.contains("decode_step=0 routed active experts")
        }));
        assert!(realized.shard_handoffs.iter().any(|handoff| {
            handoff.kind == psionic_runtime::ClusterShardHandoffKind::TensorCollective
                && handoff.from_node_id != handoff.to_node_id
        }));
        Ok(())
    }
}
