use std::collections::{BTreeMap, BTreeSet};

use psionic_runtime::{
    CacheAction, CacheInvalidationTrigger, ClusterArtifactResidencyDisposition, ClusterCacheUsage,
    ClusterExecutionDisposition, ClusterExecutionLane, ClusterPolicyDigest,
    ClusterPolicyDigestKind, ClusterReplicaNode, ClusterReplicaRoutingDisposition,
    ClusterReplicaWarmState, ClusterSelectedNode as RuntimeClusterSelectedNode,
    DeviceInventoryQualifiers, ExecutionTopologyPlan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ClusterId, ClusterServingDecision, ClusterServingFailure, ClusterServingLoadSnapshot,
    ClusterServingPolicy, ClusterServingRequest, ClusterState, NodeId,
    WholeRequestSchedulingRequest, plan_cluster_serving_admission,
    replica_routing_communication_eligibility,
};

/// Stable identity for one replicated serving lane.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterReplicaLaneKey {
    /// Stable served product identifier.
    pub product_id: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Runtime backend shared by the replica lane.
    pub runtime_backend: String,
    /// Stable served-artifact digest shared by the replica lane.
    pub served_artifact_digest: String,
    /// Stable sharded-manifest digest for the lane, when replicas were provisioned from one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharded_model_manifest_digest: Option<String>,
}

impl ClusterReplicaLaneKey {
    /// Creates one replicated serving lane key.
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
        }
    }

    /// Attaches the sharded-model manifest digest backing the replica lane.
    #[must_use]
    pub fn with_sharded_model_manifest_digest(mut self, digest: impl Into<String>) -> Self {
        self.sharded_model_manifest_digest = Some(digest.into());
        self
    }
}

/// Warm-state truth for one replica node inside a replicated lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterReplicaRecord {
    /// Lane the replica belongs to.
    pub lane: ClusterReplicaLaneKey,
    /// Node that owns the replica.
    pub node_id: NodeId,
    /// Warm-state truth for the replica.
    pub warm_state: ClusterReplicaWarmState,
    /// Machine-checkable plain-language detail, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterReplicaRecord {
    /// Creates one replica record from lane, node, and warm-state truth.
    #[must_use]
    pub fn new(
        lane: ClusterReplicaLaneKey,
        node_id: NodeId,
        warm_state: ClusterReplicaWarmState,
    ) -> Self {
        Self {
            lane,
            node_id,
            warm_state,
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

/// Replayable replica warm-state snapshot for one lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterReplicaSnapshot {
    /// Cluster identity the replica facts belong to.
    pub cluster_id: ClusterId,
    /// Lane the snapshot describes.
    pub lane: ClusterReplicaLaneKey,
    /// Replica warm-state facts by node ID.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub replicas: BTreeMap<NodeId, ClusterReplicaRecord>,
}

impl ClusterReplicaSnapshot {
    /// Creates an empty replica snapshot for one cluster and lane.
    #[must_use]
    pub fn new(cluster_id: ClusterId, lane: ClusterReplicaLaneKey) -> Self {
        Self {
            cluster_id,
            lane,
            replicas: BTreeMap::new(),
        }
    }

    /// Inserts or replaces one replica record.
    #[must_use]
    pub fn with_replica(mut self, replica: ClusterReplicaRecord) -> Self {
        self.replicas.insert(replica.node_id.clone(), replica);
        self
    }

    /// Returns a stable digest of the replica warm-state snapshot.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(b"|replica_snapshot|");
        hasher.update(self.lane.product_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.lane.model_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.lane.runtime_backend.as_bytes());
        hasher.update(b"|");
        hasher.update(self.lane.served_artifact_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(
            self.lane
                .sharded_model_manifest_digest
                .as_deref()
                .unwrap_or_default()
                .as_bytes(),
        );
        for replica in self.replicas.values() {
            hasher.update(b"|replica|");
            hasher.update(replica.node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(replica_warm_state_label(replica.warm_state));
            hasher.update(b"|");
            hasher.update(replica.detail.as_deref().unwrap_or_default().as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// Explicit warm/load policy for one replicated serving lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterReplicaLifecyclePolicy {
    /// Minimum warm replicas required before the lane is considered truly replicated.
    pub min_warm_replicas: usize,
    /// Target steady-state warm replica count.
    pub target_warm_replicas: usize,
    /// Maximum warm replicas retained simultaneously.
    pub max_warm_replicas: usize,
    /// Idle keepalive budget before a warm replica may be unloaded.
    pub idle_keepalive_seconds: u64,
    /// Hard unload threshold after sustained idleness.
    pub unload_after_idle_seconds: u64,
}

impl ClusterReplicaLifecyclePolicy {
    /// Conservative default replicated-lane lifecycle policy.
    #[must_use]
    pub const fn replicated_lane() -> Self {
        Self {
            min_warm_replicas: 2,
            target_warm_replicas: 2,
            max_warm_replicas: 4,
            idle_keepalive_seconds: 60,
            unload_after_idle_seconds: 300,
        }
    }

    /// Returns a stable digest for the lifecycle policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.min_warm_replicas.to_string());
        hasher.update(b"|");
        hasher.update(self.target_warm_replicas.to_string());
        hasher.update(b"|");
        hasher.update(self.max_warm_replicas.to_string());
        hasher.update(b"|");
        hasher.update(self.idle_keepalive_seconds.to_string());
        hasher.update(b"|");
        hasher.update(self.unload_after_idle_seconds.to_string());
        hex::encode(hasher.finalize())
    }
}

impl Default for ClusterReplicaLifecyclePolicy {
    fn default() -> Self {
        Self::replicated_lane()
    }
}

/// Successful replicated serving decision for one request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterReplicatedServingDecision {
    /// Lane the request was routed through.
    pub lane: ClusterReplicaLaneKey,
    /// Stable digest of the replica warm-state snapshot used for the decision.
    pub replica_state_digest: String,
    /// Stable digest of the replica lifecycle policy used for the decision.
    pub lifecycle_policy_digest: String,
    /// Final serving decision layered on top of the replicated lane.
    pub serving_decision: ClusterServingDecision,
}

/// Stable failure code for replicated serving.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterReplicatedServingFailureCode {
    /// Replica snapshot belongs to another cluster.
    ReplicaSnapshotClusterMismatch,
    /// Requested backend or served artifact does not match the replica lane.
    LaneMismatch,
    /// The lane lacks enough warm replicas to claim replication honestly.
    InsufficientWarmReplicas,
    /// Replica routing failed inside the serving-policy planner.
    ServingFailure,
}

/// Machine-checkable replicated serving failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterReplicatedServingFailure {
    /// Stable failure code.
    pub code: ClusterReplicatedServingFailureCode,
    /// Plain-language failure detail.
    pub detail: String,
    /// Lane the planner attempted to use.
    pub lane: ClusterReplicaLaneKey,
    /// Stable digest of the replica warm-state snapshot used for the failed decision.
    pub replica_state_digest: String,
    /// Stable digest of the replica lifecycle policy used for the failed decision.
    pub lifecycle_policy_digest: String,
    /// Underlying serving-policy failure, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serving_failure: Option<Box<ClusterServingFailure>>,
}

/// Plans replicated serving across one truthful warm replica lane.
pub fn plan_replicated_serving(
    state: &ClusterState,
    load_snapshot: &ClusterServingLoadSnapshot,
    replica_snapshot: &ClusterReplicaSnapshot,
    lifecycle_policy: &ClusterReplicaLifecyclePolicy,
    serving_policy: &ClusterServingPolicy,
    serving_request: &ClusterServingRequest,
    scheduling_request: &WholeRequestSchedulingRequest,
) -> Result<ClusterReplicatedServingDecision, Box<ClusterReplicatedServingFailure>> {
    let replica_state_digest = replica_snapshot.stable_digest();
    let lifecycle_policy_digest = lifecycle_policy.stable_digest();

    if replica_snapshot.cluster_id != *state.cluster_id() {
        return Err(Box::new(ClusterReplicatedServingFailure {
            code: ClusterReplicatedServingFailureCode::ReplicaSnapshotClusterMismatch,
            detail: format!(
                "replica snapshot belongs to cluster `{}` but state belongs to `{}`",
                replica_snapshot.cluster_id.as_str(),
                state.cluster_id().as_str()
            ),
            lane: replica_snapshot.lane.clone(),
            replica_state_digest,
            lifecycle_policy_digest,
            serving_failure: None,
        }));
    }

    if scheduling_request.requested_backend != replica_snapshot.lane.runtime_backend
        || scheduling_request.served_artifact_digest.as_deref()
            != Some(replica_snapshot.lane.served_artifact_digest.as_str())
    {
        return Err(Box::new(ClusterReplicatedServingFailure {
            code: ClusterReplicatedServingFailureCode::LaneMismatch,
            detail: format!(
                "scheduling request backend/artifact does not match replicated lane `{}/{}`",
                replica_snapshot.lane.runtime_backend, replica_snapshot.lane.served_artifact_digest
            ),
            lane: replica_snapshot.lane.clone(),
            replica_state_digest,
            lifecycle_policy_digest,
            serving_failure: None,
        }));
    }

    let warm_replica_nodes = replica_snapshot
        .replicas
        .values()
        .filter(|replica| replica.warm_state == ClusterReplicaWarmState::Warm)
        .map(|replica| replica.node_id.clone())
        .collect::<BTreeSet<_>>();
    if warm_replica_nodes.len() < lifecycle_policy.min_warm_replicas {
        return Err(Box::new(ClusterReplicatedServingFailure {
            code: ClusterReplicatedServingFailureCode::InsufficientWarmReplicas,
            detail: format!(
                "replicated lane `{}` has {} warm replicas, below required {}",
                replica_snapshot.lane.model_id,
                warm_replica_nodes.len(),
                lifecycle_policy.min_warm_replicas
            ),
            lane: replica_snapshot.lane.clone(),
            replica_state_digest,
            lifecycle_policy_digest,
            serving_failure: None,
        }));
    }

    let route_request =
        restricted_replica_scheduling_request(state, scheduling_request, &warm_replica_nodes);
    let mut serving_decision = plan_cluster_serving_admission(
        state,
        load_snapshot,
        serving_policy,
        serving_request,
        &route_request,
    )
    .map_err(|serving_failure| {
        Box::new(ClusterReplicatedServingFailure {
            code: ClusterReplicatedServingFailureCode::ServingFailure,
            detail: format!(
                "replicated serving failed for lane `{}` request `{}`",
                replica_snapshot.lane.model_id, serving_request.request_id
            ),
            lane: replica_snapshot.lane.clone(),
            replica_state_digest: replica_state_digest.clone(),
            lifecycle_policy_digest: lifecycle_policy_digest.clone(),
            serving_failure: Some(Box::new(serving_failure)),
        })
    })?;

    let replica_nodes = build_replica_nodes(
        state,
        load_snapshot,
        replica_snapshot,
        &serving_decision.schedule.selected_node_id,
    );
    let replica_devices = replica_nodes
        .iter()
        .filter(|replica| replica.warm_state == ClusterReplicaWarmState::Warm)
        .filter_map(|replica| replica.node.device_inventory.clone())
        .collect::<Vec<_>>();
    let replicated_topology = ExecutionTopologyPlan::replicated(
        replica_snapshot.lane.runtime_backend.clone(),
        replica_devices,
    );
    let clustered_cache_usage = if serving_decision
        .schedule
        .cluster_execution
        .fallback_history
        .is_empty()
    {
        ClusterCacheUsage::new(
            ClusterExecutionLane::ReplicaRouted,
            psionic_runtime::ClusterCacheScope::ReplicaLocal,
            psionic_runtime::ClusterCacheScope::ReplicaLocal,
            CacheAction::Reuse,
            CacheAction::Reuse,
        )
        .with_detail(
            "replica-routed prefix and KV reuse remained valid on the selected warm replica",
        )
    } else {
        ClusterCacheUsage::new(
            ClusterExecutionLane::ReplicaRouted,
            psionic_runtime::ClusterCacheScope::ReplicaLocal,
            psionic_runtime::ClusterCacheScope::ReplicaLocal,
            CacheAction::Invalidate,
            CacheAction::Invalidate,
        )
        .with_invalidation_trigger(CacheInvalidationTrigger::ClusterRouteChange)
        .with_detail(
            "replica-routed prefix and KV reuse were invalidated because routing changed replicas",
        )
    };

    serving_decision.schedule.execution_topology = replicated_topology.clone();
    serving_decision.schedule.cluster_execution.disposition =
        ClusterExecutionDisposition::ReplicaRouted;
    serving_decision.schedule.cluster_execution = serving_decision
        .schedule
        .cluster_execution
        .clone()
        .with_communication_eligibility(replica_routing_communication_eligibility(
            &scheduling_request.capability_profile,
        ))
        .with_replica_state_digest(replica_state_digest.clone())
        .with_execution_topology(replicated_topology)
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Replication,
            lifecycle_policy_digest.clone(),
        ))
        .with_clustered_cache_usage(clustered_cache_usage)
        .with_replica_nodes(replica_nodes);
    if let Some(sharded_model_manifest_digest) =
        replica_snapshot.lane.sharded_model_manifest_digest.clone()
    {
        serving_decision.schedule.cluster_execution = serving_decision
            .schedule
            .cluster_execution
            .clone()
            .with_sharded_model_manifest_digest(sharded_model_manifest_digest);
    }

    Ok(ClusterReplicatedServingDecision {
        lane: replica_snapshot.lane.clone(),
        replica_state_digest,
        lifecycle_policy_digest,
        serving_decision,
    })
}

fn restricted_replica_scheduling_request(
    state: &ClusterState,
    scheduling_request: &WholeRequestSchedulingRequest,
    warm_replica_nodes: &BTreeSet<NodeId>,
) -> WholeRequestSchedulingRequest {
    let mut route_request = scheduling_request.clone();
    for node_id in state.memberships().keys() {
        if *node_id != scheduling_request.scheduler_node_id && !warm_replica_nodes.contains(node_id)
        {
            route_request = route_request.excluding_node(node_id.clone());
        }
    }
    route_request
}

fn build_replica_nodes(
    state: &ClusterState,
    load_snapshot: &ClusterServingLoadSnapshot,
    replica_snapshot: &ClusterReplicaSnapshot,
    selected_node_id: &NodeId,
) -> Vec<ClusterReplicaNode> {
    replica_snapshot
        .replicas
        .values()
        .enumerate()
        .map(|(replica_index, replica)| {
            let mut node = RuntimeClusterSelectedNode::new(
                replica.node_id.as_str(),
                replica.lane.runtime_backend.clone(),
            )
            .with_served_artifact_digest(replica.lane.served_artifact_digest.clone())
            .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident);
            if let Some(membership) = state.memberships().get(&replica.node_id) {
                node = node.with_role(match membership.identity.role {
                    crate::NodeRole::CoordinatorOnly => "coordinator_only",
                    crate::NodeRole::ExecutorOnly => "executor_only",
                    crate::NodeRole::Mixed => "mixed",
                });
            }
            if let Some(device_inventory) = replica_device_inventory(
                &replica.node_id,
                replica.lane.runtime_backend.as_str(),
                state.telemetry().get(&replica.node_id),
            ) {
                node = node
                    .with_device_inventory(device_inventory.clone())
                    .with_stable_device_id(device_inventory.stable_device_id.clone());
                if let Some(topology_key) = &device_inventory.topology_key {
                    node = node.with_topology_key(topology_key.clone());
                }
            }
            let routing = if replica.node_id == *selected_node_id {
                ClusterReplicaRoutingDisposition::Selected
            } else if replica.warm_state == ClusterReplicaWarmState::Warm {
                ClusterReplicaRoutingDisposition::WarmStandby
            } else {
                ClusterReplicaRoutingDisposition::Refused
            };
            let mut replica_node =
                ClusterReplicaNode::new(replica_index, node, replica.warm_state, routing);
            if let Some(node_load) = load_snapshot.nodes.get(&replica.node_id) {
                replica_node =
                    replica_node.with_load(node_load.active_requests, node_load.queued_requests);
            }
            let detail = match routing {
                ClusterReplicaRoutingDisposition::Selected => None,
                ClusterReplicaRoutingDisposition::WarmStandby => Some(format!(
                    "warm replica `{}` is available but not selected for this request",
                    replica.node_id.as_str()
                )),
                ClusterReplicaRoutingDisposition::Refused => Some(
                    replica
                        .detail
                        .clone()
                        .unwrap_or_else(|| default_refused_replica_detail(replica)),
                ),
            };
            if let Some(detail) = detail {
                replica_node = replica_node.with_detail(detail);
            }
            replica_node
        })
        .collect()
}

fn default_refused_replica_detail(replica: &ClusterReplicaRecord) -> String {
    match replica.warm_state {
        ClusterReplicaWarmState::Cold => format!(
            "replica node `{}` is not loaded for replicated routing",
            replica.node_id.as_str()
        ),
        ClusterReplicaWarmState::Warming => format!(
            "replica node `{}` is still warming and cannot serve routed traffic yet",
            replica.node_id.as_str()
        ),
        ClusterReplicaWarmState::Warm => format!(
            "replica node `{}` was warm but excluded by routing policy",
            replica.node_id.as_str()
        ),
        ClusterReplicaWarmState::Draining => format!(
            "replica node `{}` is draining and not eligible for new routed work",
            replica.node_id.as_str()
        ),
        ClusterReplicaWarmState::Refused => format!(
            "replica node `{}` was explicitly refused for routing",
            replica.node_id.as_str()
        ),
    }
}

fn replica_device_inventory(
    node_id: &NodeId,
    runtime_backend: &str,
    telemetry: Option<&crate::ClusterNodeTelemetry>,
) -> Option<DeviceInventoryQualifiers> {
    let telemetry = telemetry?;
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
    Some(DeviceInventoryQualifiers {
        stable_device_id: format!("cluster-node:{}:{runtime_backend}", node_id.as_str()),
        topology_key: None,
        performance_class,
        memory_class,
        total_memory_bytes: telemetry.total_memory_bytes,
        free_memory_bytes: telemetry.free_memory_bytes,
    })
}

const fn replica_warm_state_label(warm_state: ClusterReplicaWarmState) -> &'static [u8] {
    match warm_state {
        ClusterReplicaWarmState::Cold => b"cold",
        ClusterReplicaWarmState::Warming => b"warming",
        ClusterReplicaWarmState::Warm => b"warm",
        ClusterReplicaWarmState::Draining => b"draining",
        ClusterReplicaWarmState::Refused => b"refused",
    }
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use std::io::Error;

    use psionic_runtime::{
        CacheAction, CacheInvalidationTrigger, ClusterAdmissionFactKind, ClusterCacheCapability,
        ClusterCacheScope, ClusterExecutionCapabilityProfile, ClusterExecutionLane,
        ClusterPolicyDigestKind,
    };

    use crate::{
        AdmissionToken, ClusterArtifactReference, ClusterArtifactResidencyRecord,
        ClusterArtifactResidencyStatus, ClusterBackendReadinessStatus,
        ClusterCommandAuthorityScope, ClusterCommandAuthorization,
        ClusterDiscoveredCandidateRecord, ClusterDiscoveredCandidateStatus, ClusterLink,
        ClusterLinkStatus, ClusterMembershipRecord, ClusterMembershipStatus, ClusterNamespace,
        ClusterNodeIdentity, ClusterNodeTelemetry, ClusterServingDecisionDisposition,
        ClusterServingWorkClass, ClusterSnapshot, ClusterTransportClass, NodeEpoch, NodeRole,
    };

    use super::*;

    fn fixture_error(detail: &str) -> Error {
        Error::other(detail.to_owned())
    }

    fn sample_cluster_id() -> ClusterId {
        ClusterId::new(
            &ClusterNamespace::new("cluster-lan"),
            &AdmissionToken::new("cluster-secret"),
        )
    }

    fn replica_lane() -> ClusterReplicaLaneKey {
        ClusterReplicaLaneKey::new(
            "psionic.text_generation",
            "gpt-oss-demo",
            "cuda",
            "artifact-1",
        )
    }

    fn replica_lane_with_manifest() -> ClusterReplicaLaneKey {
        replica_lane().with_sharded_model_manifest_digest("replica-manifest-digest")
    }

    fn ready_membership(cluster_id: &ClusterId, node_id: &str) -> ClusterMembershipRecord {
        ClusterMembershipRecord::new(
            ClusterNodeIdentity {
                cluster_id: cluster_id.clone(),
                node_id: NodeId::new(node_id),
                node_epoch: NodeEpoch::initial(),
                role: NodeRole::ExecutorOnly,
                auth_public_key: String::new(),
                attestation: None,
            },
            None,
            ClusterMembershipStatus::Ready,
        )
    }

    fn scheduler_membership(cluster_id: &ClusterId) -> ClusterMembershipRecord {
        ClusterMembershipRecord::new(
            ClusterNodeIdentity {
                cluster_id: cluster_id.clone(),
                node_id: NodeId::new("scheduler"),
                node_epoch: NodeEpoch::initial(),
                role: NodeRole::Mixed,
                auth_public_key: String::new(),
                attestation: None,
            },
            None,
            ClusterMembershipStatus::Ready,
        )
    }

    fn healthy_link(left: &str, right: &str, latency_us: u64) -> ClusterLink {
        ClusterLink::new(
            NodeId::new(left),
            NodeId::new(right),
            ClusterTransportClass::LanUdp,
            ClusterLinkStatus::Healthy,
        )
        .with_latency_us(latency_us)
        .with_bandwidth_mbps(1000)
    }

    fn ready_cuda_telemetry(node_id: &str, free_memory_bytes: u64) -> ClusterNodeTelemetry {
        ClusterNodeTelemetry::new(NodeId::new(node_id))
            .with_memory(Some(64 * 1024 * 1024 * 1024), Some(free_memory_bytes))
            .with_accelerator_count(1)
            .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready)
    }

    fn cuda_replica_routed_capability_profile() -> ClusterExecutionCapabilityProfile {
        ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![
                ClusterExecutionLane::RemoteWholeRequest,
                ClusterExecutionLane::ReplicaRouted,
            ])
            .with_clustered_cache_capability(
                ClusterCacheCapability::new(
                    ClusterExecutionLane::ReplicaRouted,
                    ClusterCacheScope::ReplicaLocal,
                    ClusterCacheScope::ReplicaLocal,
                )
                .invalidates_on_route_change()
                .with_detail(
                    "replica-routed prefix and KV reuse are only truthful on one warm replica identity",
                ),
            )
            .with_detail(
                "backend `cuda` declares whole-request dispatch plus replica routing across warm lanes",
            )
    }

    fn sample_command_authorization(
        submitter_node_id: &str,
        authority_scope: ClusterCommandAuthorityScope,
        command_digest: &str,
    ) -> ClusterCommandAuthorization {
        ClusterCommandAuthorization {
            command_digest: String::from(command_digest),
            authorization_policy_digest: String::from("command-authorization-policy"),
            authority_scope,
            submitter_node_id: NodeId::new(submitter_node_id),
            submitter_role: NodeRole::Mixed,
            submitter_membership_status: ClusterMembershipStatus::Ready,
            coordinator_authority: None,
        }
    }

    fn accepted_discovery_candidate(node_id: &str) -> ClusterDiscoveredCandidateRecord {
        ClusterDiscoveredCandidateRecord {
            candidate: crate::ClusterDiscoveryCandidate::new(
                sample_cluster_id(),
                ClusterNamespace::new("cluster-lan"),
                NodeId::new(node_id),
                NodeRole::ExecutorOnly,
                String::new(),
                Vec::new(),
            ),
            introduced_by_source_id: String::from("operator-source"),
            introduction_policy_digest: String::from("introduction-policy-digest"),
            introduction_payload_digest: format!("introduction-payload-{node_id}"),
            introduced_at_ms: 10_000,
            expires_at_ms: 20_000,
            observed_trust_bundle_version: None,
            status: ClusterDiscoveredCandidateStatus::Accepted,
            last_policy_decision: None,
            revocation: None,
            detail: Some(String::from("admitted_into_membership")),
        }
    }

    fn replica_state() -> ClusterState {
        let cluster_id = sample_cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot
            .memberships
            .insert(NodeId::new("scheduler"), scheduler_membership(&cluster_id));
        snapshot.memberships.insert(
            NodeId::new("worker-a"),
            ready_membership(&cluster_id, "worker-a"),
        );
        snapshot.memberships.insert(
            NodeId::new("worker-b"),
            ready_membership(&cluster_id, "worker-b"),
        );
        snapshot.telemetry.insert(
            NodeId::new("worker-a"),
            ready_cuda_telemetry("worker-a", 48 * 1024 * 1024 * 1024),
        );
        snapshot.telemetry.insert(
            NodeId::new("worker-b"),
            ready_cuda_telemetry("worker-b", 32 * 1024 * 1024 * 1024),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(NodeId::new("scheduler"), NodeId::new("worker-a")),
            healthy_link("scheduler", "worker-a", 300),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(NodeId::new("scheduler"), NodeId::new("worker-b")),
            healthy_link("scheduler", "worker-b", 900),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(NodeId::new("worker-a"), "artifact-1"),
            ClusterArtifactResidencyRecord::new(
                NodeId::new("worker-a"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(NodeId::new("worker-b"), "artifact-1"),
            ClusterArtifactResidencyRecord::new(
                NodeId::new("worker-b"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        ClusterState::from_snapshot(snapshot)
    }

    fn replica_state_with_authority_and_candidate_truth() -> ClusterState {
        let cluster_id = sample_cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot
            .memberships
            .insert(NodeId::new("scheduler"), scheduler_membership(&cluster_id));
        snapshot.memberships.insert(
            NodeId::new("worker-a"),
            ready_membership(&cluster_id, "worker-a"),
        );
        snapshot.memberships.insert(
            NodeId::new("worker-b"),
            ready_membership(&cluster_id, "worker-b"),
        );
        snapshot.telemetry.insert(
            NodeId::new("worker-a"),
            ready_cuda_telemetry("worker-a", 48 * 1024 * 1024 * 1024),
        );
        snapshot.telemetry.insert(
            NodeId::new("worker-b"),
            ready_cuda_telemetry("worker-b", 32 * 1024 * 1024 * 1024),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(NodeId::new("scheduler"), NodeId::new("worker-a")),
            healthy_link("scheduler", "worker-a", 300),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(NodeId::new("scheduler"), NodeId::new("worker-b")),
            healthy_link("scheduler", "worker-b", 900),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(NodeId::new("worker-a"), "artifact-1"),
            ClusterArtifactResidencyRecord::new(
                NodeId::new("worker-a"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(NodeId::new("worker-b"), "artifact-1"),
            ClusterArtifactResidencyRecord::new(
                NodeId::new("worker-b"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.membership_provenance.insert(
            NodeId::new("scheduler"),
            sample_command_authorization(
                "scheduler",
                ClusterCommandAuthorityScope::SelfNode,
                "scheduler-membership-command",
            ),
        );
        snapshot.membership_provenance.insert(
            NodeId::new("worker-a"),
            sample_command_authorization(
                "worker-a",
                ClusterCommandAuthorityScope::SelfNode,
                "worker-a-membership-command",
            ),
        );
        snapshot.discovery_candidates.insert(
            NodeId::new("worker-a"),
            accepted_discovery_candidate("worker-a"),
        );
        snapshot.discovery_candidate_provenance.insert(
            NodeId::new("worker-a"),
            sample_command_authorization(
                "scheduler",
                ClusterCommandAuthorityScope::CoordinatorOnly,
                "worker-a-candidate-admission",
            ),
        );
        snapshot.artifact_residency_provenance.insert(
            crate::ClusterArtifactResidencyKey::new(NodeId::new("worker-a"), "artifact-1"),
            sample_command_authorization(
                "worker-a",
                ClusterCommandAuthorityScope::SelfNode,
                "worker-a-artifact-command",
            ),
        );
        snapshot.admission_policy_provenance = Some(sample_command_authorization(
            "scheduler",
            ClusterCommandAuthorityScope::CoordinatorOnly,
            "admission-policy-command",
        ));
        snapshot.leadership_provenance = Some(sample_command_authorization(
            "scheduler",
            ClusterCommandAuthorityScope::ProposedLeader,
            "leadership-command",
        ));
        snapshot.leadership = Some(crate::ClusterLeadershipRecord::new(
            crate::ClusterTerm::initial(),
            NodeId::new("scheduler"),
            crate::ClusterEventIndex::initial(),
        ));
        ClusterState::from_snapshot(snapshot)
    }

    fn scheduling_request() -> WholeRequestSchedulingRequest {
        WholeRequestSchedulingRequest::new(NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_replica_routed_capability_profile())
            .with_served_artifact_digest("artifact-1")
            .requiring_accelerator()
    }

    #[test]
    fn replicated_serving_builds_replicated_topology_and_selects_best_warm_replica()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = replica_state();
        let replica_snapshot =
            ClusterReplicaSnapshot::new(state.cluster_id().clone(), replica_lane())
                .with_replica(ClusterReplicaRecord::new(
                    replica_lane(),
                    NodeId::new("worker-a"),
                    ClusterReplicaWarmState::Warm,
                ))
                .with_replica(ClusterReplicaRecord::new(
                    replica_lane(),
                    NodeId::new("worker-b"),
                    ClusterReplicaWarmState::Warm,
                ));
        let load_snapshot = ClusterServingLoadSnapshot::new(state.cluster_id().clone())
            .with_node_load(crate::ClusterNodeServiceLoad::new(NodeId::new("worker-a")))
            .with_node_load(crate::ClusterNodeServiceLoad::new(NodeId::new("worker-b")));

        let decision = plan_replicated_serving(
            &state,
            &load_snapshot,
            &replica_snapshot,
            &ClusterReplicaLifecyclePolicy::replicated_lane(),
            &ClusterServingPolicy::direct_caller_latency_first(),
            &ClusterServingRequest::new("req-replica-1", ClusterServingWorkClass::Decode),
            &scheduling_request(),
        )
        .map_err(|err| fixture_error(&format!("replicated serving should succeed: {err:?}")))?;

        assert_eq!(
            decision.serving_decision.disposition,
            ClusterServingDecisionDisposition::ExecuteNow
        );
        assert_eq!(
            decision.serving_decision.schedule.selected_node_id,
            NodeId::new("worker-a")
        );
        assert_eq!(
            decision.serving_decision.schedule.execution_topology.kind,
            psionic_runtime::ExecutionTopologyKind::Replicated
        );
        assert_eq!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .disposition,
            ClusterExecutionDisposition::ReplicaRouted
        );
        assert_eq!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .execution_topology
                .as_ref()
                .map(|topology| topology.kind),
            Some(psionic_runtime::ExecutionTopologyKind::Replicated)
        );
        assert_eq!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .replica_nodes
                .iter()
                .filter(|replica| replica.routing == ClusterReplicaRoutingDisposition::WarmStandby)
                .count(),
            1
        );
        assert!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .communication_eligibility
                .as_ref()
                .and_then(|eligibility| eligibility.capability_profile_digest.as_deref())
                .is_some()
        );
        assert_eq!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .clustered_cache_usage
                .as_ref()
                .map(|usage| usage.prefix_action),
            Some(CacheAction::Reuse)
        );
        Ok(())
    }

    #[test]
    fn replicated_serving_reroutes_away_from_slow_replica_and_records_refused_peer()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = replica_state();
        let replica_snapshot =
            ClusterReplicaSnapshot::new(state.cluster_id().clone(), replica_lane())
                .with_replica(ClusterReplicaRecord::new(
                    replica_lane(),
                    NodeId::new("worker-a"),
                    ClusterReplicaWarmState::Warm,
                ))
                .with_replica(ClusterReplicaRecord::new(
                    replica_lane(),
                    NodeId::new("worker-b"),
                    ClusterReplicaWarmState::Warm,
                ));
        let load_snapshot = ClusterServingLoadSnapshot::new(state.cluster_id().clone())
            .with_node_load(
                crate::ClusterNodeServiceLoad::new(NodeId::new("worker-a"))
                    .with_service_health(crate::ClusterNodeServiceHealth::Slow),
            )
            .with_node_load(crate::ClusterNodeServiceLoad::new(NodeId::new("worker-b")));

        let decision = plan_replicated_serving(
            &state,
            &load_snapshot,
            &replica_snapshot,
            &ClusterReplicaLifecyclePolicy::replicated_lane(),
            &ClusterServingPolicy::direct_caller_latency_first(),
            &ClusterServingRequest::new("req-replica-2", ClusterServingWorkClass::Decode),
            &scheduling_request(),
        )
        .map_err(|err| fixture_error(&format!("replicated reroute should succeed: {err:?}")))?;

        assert_eq!(
            decision.serving_decision.schedule.selected_node_id,
            NodeId::new("worker-b")
        );
        assert!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .fallback_history
                .iter()
                .any(|step| {
                    step.from_node_id.as_deref() == Some("worker-a")
                        && step.to_node_id == "worker-b"
                })
        );
        assert!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .replica_nodes
                .iter()
                .any(|replica| {
                    replica.node.node_id == "worker-a"
                        && replica.routing == ClusterReplicaRoutingDisposition::WarmStandby
                })
        );
        assert_eq!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .clustered_cache_usage
                .as_ref()
                .and_then(|usage| usage.invalidation_trigger),
            Some(CacheInvalidationTrigger::ClusterRouteChange)
        );
        Ok(())
    }

    #[test]
    fn replicated_serving_preserves_admission_and_replication_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = replica_state_with_authority_and_candidate_truth();
        let replica_snapshot =
            ClusterReplicaSnapshot::new(state.cluster_id().clone(), replica_lane_with_manifest())
                .with_replica(ClusterReplicaRecord::new(
                    replica_lane_with_manifest(),
                    NodeId::new("worker-a"),
                    ClusterReplicaWarmState::Warm,
                ))
                .with_replica(ClusterReplicaRecord::new(
                    replica_lane_with_manifest(),
                    NodeId::new("worker-b"),
                    ClusterReplicaWarmState::Warm,
                ));
        let load_snapshot = ClusterServingLoadSnapshot::new(state.cluster_id().clone())
            .with_node_load(crate::ClusterNodeServiceLoad::new(NodeId::new("worker-a")))
            .with_node_load(crate::ClusterNodeServiceLoad::new(NodeId::new("worker-b")));

        let decision = plan_replicated_serving(
            &state,
            &load_snapshot,
            &replica_snapshot,
            &ClusterReplicaLifecyclePolicy::replicated_lane(),
            &ClusterServingPolicy::direct_caller_latency_first(),
            &ClusterServingRequest::new("req-replica-4", ClusterServingWorkClass::Decode),
            &scheduling_request(),
        )
        .map_err(|err| fixture_error(&format!("replicated serving should succeed: {err:?}")))?;

        assert!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .policy_digests
                .iter()
                .any(|digest| digest.kind == ClusterPolicyDigestKind::Admission),
            "replica-routed execution should retain admission-policy truth"
        );
        assert!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .policy_digests
                .iter()
                .any(|digest| digest.kind == ClusterPolicyDigestKind::Serving),
            "replica-routed execution should retain serving-policy truth"
        );
        assert!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .policy_digests
                .iter()
                .any(|digest| digest.kind == ClusterPolicyDigestKind::Replication),
            "replica-routed execution should retain replication-policy truth"
        );
        assert!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .command_provenance
                .iter()
                .any(|fact| fact.fact_kind == ClusterAdmissionFactKind::AdmissionPolicy),
            "replica-routed execution should carry admission-policy provenance"
        );
        assert!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .command_provenance
                .iter()
                .any(|fact| fact.fact_kind == ClusterAdmissionFactKind::SelectedCandidateAdmission),
            "replica-routed execution should carry selected-candidate provenance"
        );
        assert_eq!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .sharded_model_manifest_digest
                .as_deref(),
            Some("replica-manifest-digest")
        );
        Ok(())
    }

    #[test]
    fn replicated_serving_refuses_when_lane_lacks_enough_warm_replicas()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = replica_state();
        let replica_snapshot =
            ClusterReplicaSnapshot::new(state.cluster_id().clone(), replica_lane())
                .with_replica(ClusterReplicaRecord::new(
                    replica_lane(),
                    NodeId::new("worker-a"),
                    ClusterReplicaWarmState::Warm,
                ))
                .with_replica(
                    ClusterReplicaRecord::new(
                        replica_lane(),
                        NodeId::new("worker-b"),
                        ClusterReplicaWarmState::Warming,
                    )
                    .with_detail("still loading weights"),
                );
        let load_snapshot = ClusterServingLoadSnapshot::new(state.cluster_id().clone())
            .with_node_load(crate::ClusterNodeServiceLoad::new(NodeId::new("worker-a")))
            .with_node_load(crate::ClusterNodeServiceLoad::new(NodeId::new("worker-b")));

        let failure = match plan_replicated_serving(
            &state,
            &load_snapshot,
            &replica_snapshot,
            &ClusterReplicaLifecyclePolicy::replicated_lane(),
            &ClusterServingPolicy::direct_caller_latency_first(),
            &ClusterServingRequest::new("req-replica-3", ClusterServingWorkClass::Decode),
            &scheduling_request(),
        ) {
            Ok(decision) => {
                return Err(fixture_error(&format!(
                    "expected insufficient-warm-replica failure, got {decision:?}"
                ))
                .into());
            }
            Err(failure) => failure,
        };

        assert_eq!(
            failure.code,
            ClusterReplicatedServingFailureCode::InsufficientWarmReplicas
        );
        Ok(())
    }
}
