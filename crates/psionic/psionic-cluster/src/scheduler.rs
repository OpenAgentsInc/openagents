use std::collections::BTreeSet;

use psionic_runtime::{
    ClusterAdmissionFactKind,
    ClusterArtifactResidencyDisposition as RuntimeArtifactResidencyDisposition,
    ClusterCommandAuthorityScopeEvidence, ClusterCommandProvenanceEvidence,
    ClusterCommitAuthorityEvidence, ClusterCommunicationEligibility,
    ClusterExecutionCapabilityProfile, ClusterExecutionContext, ClusterExecutionDisposition,
    ClusterExecutionLane, ClusterPolicyDigest, ClusterPolicyDigestKind,
    ClusterSelectedNode as RuntimeClusterSelectedNode,
    ClusterTransportClass as RuntimeClusterTransportClass, DeviceInventoryQualifiers,
    DeviceMemoryClass, DevicePerformanceClass, ExecutionTopologyPlan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ClusterArtifactResidencyKey, ClusterArtifactResidencyStatus, ClusterBackendReadinessStatus,
    ClusterDiscoveredCandidateStatus, ClusterId, ClusterLink, ClusterLinkKey, ClusterLinkStatus,
    ClusterMembershipRecord, ClusterMembershipStatus, ClusterNodeTelemetry,
    ClusterStabilityPosture, ClusterState, ClusterTransportClass, NodeId, NodeRole,
};

/// Optional Exo-derived placement input for bounded scheduler experimentation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExoPlacementHint {
    /// Cluster this hint was derived for.
    pub cluster_id: ClusterId,
    /// Stable source identifier for the orchestrator or peer that emitted the hint.
    pub source_id: String,
    /// Stable topology digest the hint expects.
    pub topology_digest: String,
    /// Ordered candidate node set suggested by the hint.
    pub suggested_node_ids: Vec<NodeId>,
    /// Plain-language detail explaining the hint, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ExoPlacementHint {
    /// Creates a bounded Exo placement hint.
    #[must_use]
    pub fn new(
        cluster_id: ClusterId,
        source_id: impl Into<String>,
        topology_digest: impl Into<String>,
        suggested_node_ids: Vec<NodeId>,
    ) -> Self {
        let mut suggested_node_ids = suggested_node_ids;
        suggested_node_ids.sort_unstable();
        suggested_node_ids.dedup();
        Self {
            cluster_id,
            source_id: source_id.into(),
            topology_digest: topology_digest.into(),
            suggested_node_ids,
            detail: None,
        }
    }

    /// Attaches plain-language detail for the hint.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Returns a stable digest for this hint.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"exo_placement_hint|");
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(self.source_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.topology_digest.as_bytes());
        for node_id in &self.suggested_node_ids {
            hasher.update(b"|node|");
            hasher.update(node_id.as_str().as_bytes());
        }
        hasher.update(b"|");
        hasher.update(self.detail.as_deref().unwrap_or_default().as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Request for whole-request remote scheduling onto one cluster node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WholeRequestSchedulingRequest {
    /// Node performing the scheduling decision.
    pub scheduler_node_id: NodeId,
    /// Runtime backend required for the execution lane.
    pub requested_backend: String,
    /// Declared cluster execution capability contract for the requested backend.
    pub capability_profile: ClusterExecutionCapabilityProfile,
    /// Served-artifact digest that must be runnable on the selected node, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub served_artifact_digest: Option<String>,
    /// Minimum free memory required for the request on the selected node, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_free_memory_bytes: Option<u64>,
    /// Whether the request requires at least one visible accelerator.
    pub require_accelerator: bool,
    /// Whether peer-copy staging is allowed for the selected node.
    pub allow_copy_staging: bool,
    /// Whether pull-based staging is allowed for the selected node.
    pub allow_pull_staging: bool,
    /// Stable policy digests that constrained the decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_digests: Vec<ClusterPolicyDigest>,
    /// Optional bounded Exo-derived placement hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exo_placement_hint: Option<ExoPlacementHint>,
    /// Explicit nodes that this scheduling attempt must exclude.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub excluded_node_ids: BTreeSet<NodeId>,
}

impl WholeRequestSchedulingRequest {
    /// Creates a whole-request scheduling request for one scheduler node and backend.
    #[must_use]
    pub fn new(scheduler_node_id: NodeId, requested_backend: impl Into<String>) -> Self {
        let requested_backend = requested_backend.into();
        Self {
            scheduler_node_id,
            requested_backend: requested_backend.clone(),
            capability_profile: ClusterExecutionCapabilityProfile::new(requested_backend),
            served_artifact_digest: None,
            minimum_free_memory_bytes: None,
            require_accelerator: false,
            allow_copy_staging: true,
            allow_pull_staging: true,
            policy_digests: Vec::new(),
            exo_placement_hint: None,
            excluded_node_ids: BTreeSet::new(),
        }
    }

    /// Attaches a served-artifact digest required by the request.
    #[must_use]
    pub fn with_served_artifact_digest(
        mut self,
        served_artifact_digest: impl Into<String>,
    ) -> Self {
        self.served_artifact_digest = Some(served_artifact_digest.into());
        self
    }

    /// Attaches a minimum free-memory requirement.
    #[must_use]
    pub const fn with_minimum_free_memory_bytes(mut self, minimum_free_memory_bytes: u64) -> Self {
        self.minimum_free_memory_bytes = Some(minimum_free_memory_bytes);
        self
    }

    /// Marks the request as requiring accelerator-backed execution.
    #[must_use]
    pub const fn requiring_accelerator(mut self) -> Self {
        self.require_accelerator = true;
        self
    }

    /// Overrides whether copy and pull staging are allowed for this request.
    #[must_use]
    pub const fn with_staging_policy(
        mut self,
        allow_copy_staging: bool,
        allow_pull_staging: bool,
    ) -> Self {
        self.allow_copy_staging = allow_copy_staging;
        self.allow_pull_staging = allow_pull_staging;
        self
    }

    /// Attaches the declared capability profile and synchronizes the requested backend to it.
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

    /// Appends one policy digest reference.
    #[must_use]
    pub fn with_policy_digest(mut self, policy_digest: ClusterPolicyDigest) -> Self {
        self.policy_digests.push(policy_digest);
        self
    }

    /// Attaches an optional bounded Exo-derived placement hint.
    #[must_use]
    pub fn with_exo_placement_hint(mut self, exo_placement_hint: ExoPlacementHint) -> Self {
        self.exo_placement_hint = Some(exo_placement_hint);
        self
    }

    /// Excludes one node from the current scheduling attempt.
    #[must_use]
    pub fn excluding_node(mut self, node_id: NodeId) -> Self {
        self.excluded_node_ids.insert(node_id);
        self
    }

    /// Excludes multiple nodes from the current scheduling attempt.
    #[must_use]
    pub fn excluding_nodes<I>(mut self, node_ids: I) -> Self
    where
        I: IntoIterator<Item = NodeId>,
    {
        self.excluded_node_ids.extend(node_ids);
        self
    }
}

#[cfg(test)]
const METAL_CLUSTER_BLOCK_DETAIL: &str = "backend `metal` remains refused for cluster execution until the Metal roadmap queue `#3286` -> `#3285` -> `#3269` -> `#3262` closes";

/// Returns communication-class eligibility for one whole-request remote dispatch lane.
#[must_use]
pub fn remote_dispatch_communication_eligibility(
    capability_profile: &ClusterExecutionCapabilityProfile,
) -> ClusterCommunicationEligibility {
    ClusterCommunicationEligibility::from_capability_profile_lane(
        capability_profile,
        ClusterExecutionLane::RemoteWholeRequest,
    )
}

/// Returns communication-class eligibility for one replicated-routing lane.
#[must_use]
pub fn replica_routing_communication_eligibility(
    capability_profile: &ClusterExecutionCapabilityProfile,
) -> ClusterCommunicationEligibility {
    ClusterCommunicationEligibility::from_capability_profile_lane(
        capability_profile,
        ClusterExecutionLane::ReplicaRouted,
    )
}

/// Returns communication-class eligibility for one layer-sharded handoff lane.
#[must_use]
pub fn layer_shard_handoff_communication_eligibility(
    capability_profile: &ClusterExecutionCapabilityProfile,
) -> ClusterCommunicationEligibility {
    ClusterCommunicationEligibility::from_capability_profile_lane(
        capability_profile,
        ClusterExecutionLane::LayerSharded,
    )
}

/// Returns communication-class eligibility for one tensor-collective mesh lane.
#[must_use]
pub fn tensor_collective_communication_eligibility(
    capability_profile: &ClusterExecutionCapabilityProfile,
) -> ClusterCommunicationEligibility {
    ClusterCommunicationEligibility::from_capability_profile_lane(
        capability_profile,
        ClusterExecutionLane::TensorSharded,
    )
}

/// Stable refusal code for one remote whole-request candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WholeRequestSchedulingRefusalCode {
    /// The scheduler node itself cannot be selected for remote execution.
    LocalNodeExcluded,
    /// The candidate is not in `ready` membership state.
    MembershipNotReady,
    /// The candidate cannot execute work under its current declared role.
    NodeRoleIneligible,
    /// The candidate lacks current telemetry or backend-readiness facts.
    TelemetryMissing,
    /// The candidate is too unstable for trusted scheduling.
    NodeUnstable,
    /// The requested backend is unavailable on the candidate.
    BackendUnavailable,
    /// The candidate lacks the required served-artifact residency fact.
    ArtifactStateMissing,
    /// The candidate explicitly refuses the required artifact.
    ArtifactUnavailable,
    /// The candidate requires staging that current policy forbids.
    ArtifactStagingNotAllowed,
    /// The candidate does not meet the explicit free-memory bound.
    InsufficientFreeMemory,
    /// The request requires an accelerator and the candidate lacks one.
    AcceleratorUnavailable,
    /// No usable transport fact exists between the scheduler and candidate.
    TransportMissing,
    /// The transport fact exists, but is not usable for scheduling.
    TransportUnavailable,
    /// The scheduler excluded this node before evaluating it.
    PolicyExcluded,
    /// The candidate exists in wider-network discovery truth but has not been admitted.
    CandidateNotAccepted,
    /// The candidate exists in wider-network discovery truth but was explicitly revoked.
    CandidateRevoked,
}

/// One explicit candidate refusal emitted by the whole-request scheduler.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WholeRequestSchedulingRefusal {
    /// Candidate node that was refused, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<NodeId>,
    /// Stable refusal code.
    pub code: WholeRequestSchedulingRefusalCode,
    /// Plain-language refusal detail.
    pub detail: String,
}

impl WholeRequestSchedulingRefusal {
    fn for_node(
        node_id: NodeId,
        code: WholeRequestSchedulingRefusalCode,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            node_id: Some(node_id),
            code,
            detail: detail.into(),
        }
    }
}

/// Stable note code describing why one node was selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WholeRequestSchedulingSelectionCode {
    /// The scheduler selected the best fully ready remote candidate.
    SelectedBestCandidate,
    /// An Exo-derived hint matched the final selected node without widening eligibility.
    ExoPlacementHintAccepted,
    /// An Exo-derived hint was ignored under current authoritative cluster truth.
    ExoPlacementHintIgnored,
    /// The winning node requires a peer copy before execution.
    ArtifactCopyRequired,
    /// The winning node requires a pull before execution.
    ArtifactPullRequired,
    /// The winning node's requested backend is degraded.
    BackendDegraded,
    /// The winning node is reachable only over a degraded link.
    TransportDegraded,
    /// The winning node is only flaky rather than stable.
    NodeFlaky,
}

/// One explicit selection note for the winning remote whole-request node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WholeRequestSchedulingSelectionNote {
    /// Stable note code.
    pub code: WholeRequestSchedulingSelectionCode,
    /// Plain-language detail for the selected path.
    pub detail: String,
}

impl WholeRequestSchedulingSelectionNote {
    fn new(code: WholeRequestSchedulingSelectionCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

/// Stable failure code for whole-request remote scheduling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WholeRequestSchedulingFailureCode {
    /// The supplied scheduler node is absent from authoritative membership state.
    SchedulerNodeUnknown,
    /// The supplied scheduler node is present, but not ready to make remote scheduling decisions.
    SchedulerNodeNotReady,
    /// The requested backend does not satisfy the required communication class for cluster execution.
    CommunicationClassIneligible,
    /// No eligible remote execution candidate exists under the current cluster facts.
    NoEligibleRemoteNode,
}

/// Machine-checkable scheduling failure with explicit per-candidate refusals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WholeRequestSchedulingFailure {
    /// High-level failure code.
    pub code: WholeRequestSchedulingFailureCode,
    /// Plain-language failure detail.
    pub detail: String,
    /// Cluster identity used for the failed decision.
    pub cluster_id: crate::ClusterId,
    /// Node that attempted to schedule the work.
    pub scheduler_node_id: NodeId,
    /// Runtime backend required for the request.
    pub requested_backend: String,
    /// Stable digest of the authoritative cluster-state snapshot.
    pub cluster_state_digest: String,
    /// Stable digest of topology facts used for the failed decision.
    pub topology_digest: String,
    /// Stable digest of artifact residency facts used for the failed decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_residency_digest: Option<String>,
    /// Stable policy digests that constrained the failed decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_digests: Vec<ClusterPolicyDigest>,
    /// Explicit backend communication-class eligibility for the failed path.
    pub communication_eligibility: ClusterCommunicationEligibility,
    /// Explicit per-candidate refusals that explain why scheduling failed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusals: Vec<WholeRequestSchedulingRefusal>,
}

/// Successful whole-request remote scheduling result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WholeRequestClusterSchedule {
    /// Cluster identity used for the decision.
    pub cluster_id: crate::ClusterId,
    /// Node that performed the scheduling decision.
    pub scheduler_node_id: NodeId,
    /// Remote node selected for execution.
    pub selected_node_id: NodeId,
    /// Runtime backend selected for the request.
    pub runtime_backend: String,
    /// Derived single-node remote device inventory for truthful topology reporting.
    pub selected_device: DeviceInventoryQualifiers,
    /// Truthful single-node execution topology for the scheduled remote request.
    pub execution_topology: ExecutionTopologyPlan,
    /// Cluster execution evidence for provider and receipt surfaces.
    pub cluster_execution: ClusterExecutionContext,
    /// Explicit selection notes for the chosen path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selection_notes: Vec<WholeRequestSchedulingSelectionNote>,
    /// Explicit refusals observed while evaluating other candidates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusals: Vec<WholeRequestSchedulingRefusal>,
}

/// Schedules a whole request onto one best remote node under the current cluster facts.
pub fn schedule_remote_whole_request(
    state: &ClusterState,
    request: &WholeRequestSchedulingRequest,
) -> Result<WholeRequestClusterSchedule, Box<WholeRequestSchedulingFailure>> {
    let cluster_state_digest = state.stable_digest();
    let topology_digest = state.topology_digest();
    let artifact_residency_digest = Some(state.artifact_residency_digest());
    let communication_eligibility =
        remote_dispatch_communication_eligibility(&request.capability_profile);
    let Some(scheduler_membership) = state.memberships().get(&request.scheduler_node_id) else {
        return Err(Box::new(WholeRequestSchedulingFailure {
            code: WholeRequestSchedulingFailureCode::SchedulerNodeUnknown,
            detail: format!(
                "scheduler node `{}` is not present in authoritative cluster membership",
                request.scheduler_node_id.as_str()
            ),
            cluster_id: state.cluster_id().clone(),
            scheduler_node_id: request.scheduler_node_id.clone(),
            requested_backend: request.requested_backend.clone(),
            cluster_state_digest,
            topology_digest,
            artifact_residency_digest,
            policy_digests: request.policy_digests.clone(),
            communication_eligibility,
            refusals: Vec::new(),
        }));
    };
    if scheduler_membership.status != ClusterMembershipStatus::Ready {
        return Err(Box::new(WholeRequestSchedulingFailure {
            code: WholeRequestSchedulingFailureCode::SchedulerNodeNotReady,
            detail: format!(
                "scheduler node `{}` is not ready for remote scheduling",
                request.scheduler_node_id.as_str()
            ),
            cluster_id: state.cluster_id().clone(),
            scheduler_node_id: request.scheduler_node_id.clone(),
            requested_backend: request.requested_backend.clone(),
            cluster_state_digest,
            topology_digest,
            artifact_residency_digest,
            policy_digests: request.policy_digests.clone(),
            communication_eligibility,
            refusals: Vec::new(),
        }));
    }
    if !communication_eligibility.eligible {
        return Err(Box::new(WholeRequestSchedulingFailure {
            code: WholeRequestSchedulingFailureCode::CommunicationClassIneligible,
            detail: communication_eligibility
                .detail
                .clone()
                .unwrap_or_else(|| {
                    format!(
                        "backend `{}` does not satisfy whole-request cluster dispatch communication eligibility",
                        request.requested_backend
                    )
                }),
            cluster_id: state.cluster_id().clone(),
            scheduler_node_id: request.scheduler_node_id.clone(),
            requested_backend: request.requested_backend.clone(),
            cluster_state_digest,
            topology_digest,
            artifact_residency_digest,
            policy_digests: request.policy_digests.clone(),
            communication_eligibility,
            refusals: Vec::new(),
        }));
    }

    let exo_placement_hint =
        evaluate_exo_placement_hint(state, request.exo_placement_hint.as_ref());
    let mut refusals = Vec::new();
    let mut candidates = Vec::new();
    for (node_id, membership) in state.memberships() {
        if *node_id == request.scheduler_node_id {
            refusals.push(WholeRequestSchedulingRefusal::for_node(
                node_id.clone(),
                WholeRequestSchedulingRefusalCode::LocalNodeExcluded,
                "whole-request remote scheduling excludes the scheduler node itself",
            ));
            continue;
        }
        if request.excluded_node_ids.contains(node_id) {
            refusals.push(WholeRequestSchedulingRefusal::for_node(
                node_id.clone(),
                WholeRequestSchedulingRefusalCode::PolicyExcluded,
                format!(
                    "candidate node `{}` was excluded by the current serving-policy attempt",
                    node_id.as_str()
                ),
            ));
            continue;
        }
        if membership.status != ClusterMembershipStatus::Ready {
            refusals.push(WholeRequestSchedulingRefusal::for_node(
                node_id.clone(),
                WholeRequestSchedulingRefusalCode::MembershipNotReady,
                format!(
                    "candidate node `{}` is not in ready membership state",
                    node_id.as_str()
                ),
            ));
            continue;
        }
        if let Some(candidate) = state.discovery_candidates().get(node_id) {
            match candidate.status {
                ClusterDiscoveredCandidateStatus::Accepted => {}
                ClusterDiscoveredCandidateStatus::Revoked => {
                    let revocation_detail = candidate
                        .revocation
                        .as_ref()
                        .and_then(|revocation| revocation.detail.as_deref())
                        .unwrap_or("candidate_revoked");
                    refusals.push(WholeRequestSchedulingRefusal::for_node(
                        node_id.clone(),
                        WholeRequestSchedulingRefusalCode::CandidateRevoked,
                        format!(
                            "candidate node `{}` was explicitly revoked under admission policy: {revocation_detail}",
                            node_id.as_str()
                        ),
                    ));
                    continue;
                }
                status => {
                    refusals.push(WholeRequestSchedulingRefusal::for_node(
                        node_id.clone(),
                        WholeRequestSchedulingRefusalCode::CandidateNotAccepted,
                        format!(
                            "candidate node `{}` is present in discovery truth but is not admitted for scheduling while status is `{status:?}`",
                            node_id.as_str()
                        ),
                    ));
                    continue;
                }
            }
        }
        if !matches!(
            membership.identity.role,
            NodeRole::ExecutorOnly | NodeRole::Mixed
        ) {
            refusals.push(WholeRequestSchedulingRefusal::for_node(
                node_id.clone(),
                WholeRequestSchedulingRefusalCode::NodeRoleIneligible,
                format!(
                    "candidate node `{}` cannot execute work under role `{}`",
                    node_id.as_str(),
                    node_role_name(membership.identity.role)
                ),
            ));
            continue;
        }
        let Some(telemetry) = state.telemetry().get(node_id) else {
            refusals.push(WholeRequestSchedulingRefusal::for_node(
                node_id.clone(),
                WholeRequestSchedulingRefusalCode::TelemetryMissing,
                format!(
                    "candidate node `{}` has no authoritative telemetry record",
                    node_id.as_str()
                ),
            ));
            continue;
        };
        if telemetry.stability == ClusterStabilityPosture::Unstable {
            refusals.push(WholeRequestSchedulingRefusal::for_node(
                node_id.clone(),
                WholeRequestSchedulingRefusalCode::NodeUnstable,
                format!(
                    "candidate node `{}` is marked unstable in authoritative telemetry",
                    node_id.as_str()
                ),
            ));
            continue;
        }

        let backend_readiness = telemetry
            .backend_readiness
            .get(request.requested_backend.as_str())
            .copied()
            .unwrap_or(ClusterBackendReadinessStatus::Unknown);
        if matches!(
            backend_readiness,
            ClusterBackendReadinessStatus::Refused | ClusterBackendReadinessStatus::Unknown
        ) {
            refusals.push(WholeRequestSchedulingRefusal::for_node(
                node_id.clone(),
                WholeRequestSchedulingRefusalCode::BackendUnavailable,
                format!(
                    "candidate node `{}` is not ready for backend `{}`",
                    node_id.as_str(),
                    request.requested_backend
                ),
            ));
            continue;
        }

        if request.require_accelerator && telemetry.accelerator_count.unwrap_or_default() == 0 {
            refusals.push(WholeRequestSchedulingRefusal::for_node(
                node_id.clone(),
                WholeRequestSchedulingRefusalCode::AcceleratorUnavailable,
                format!(
                    "candidate node `{}` has no visible accelerator for backend `{}`",
                    node_id.as_str(),
                    request.requested_backend
                ),
            ));
            continue;
        }

        if let Some(minimum_free_memory_bytes) = request.minimum_free_memory_bytes {
            match telemetry.free_memory_bytes {
                Some(free_memory_bytes) if free_memory_bytes >= minimum_free_memory_bytes => {}
                Some(free_memory_bytes) => {
                    refusals.push(WholeRequestSchedulingRefusal::for_node(
                        node_id.clone(),
                        WholeRequestSchedulingRefusalCode::InsufficientFreeMemory,
                        format!(
                            "candidate node `{}` exposes {free_memory_bytes} free bytes, below required {minimum_free_memory_bytes}",
                            node_id.as_str()
                        ),
                    ));
                    continue;
                }
                None => {
                    refusals.push(WholeRequestSchedulingRefusal::for_node(
                        node_id.clone(),
                        WholeRequestSchedulingRefusalCode::InsufficientFreeMemory,
                        format!(
                            "candidate node `{}` does not expose free-memory telemetry for an explicit memory-bounded request",
                            node_id.as_str()
                        ),
                    ));
                    continue;
                }
            }
        }

        let artifact_status = match request.served_artifact_digest.as_deref() {
            Some(served_artifact_digest) => {
                let key = ClusterArtifactResidencyKey::new(node_id.clone(), served_artifact_digest);
                let Some(record) = state.artifact_residency().get(&key) else {
                    refusals.push(WholeRequestSchedulingRefusal::for_node(
                        node_id.clone(),
                        WholeRequestSchedulingRefusalCode::ArtifactStateMissing,
                        format!(
                            "candidate node `{}` has no artifact residency fact for `{served_artifact_digest}`",
                            node_id.as_str()
                        ),
                    ));
                    continue;
                };
                match record.status {
                    ClusterArtifactResidencyStatus::Resident => Some(record.status),
                    ClusterArtifactResidencyStatus::CopyRequired if request.allow_copy_staging => {
                        Some(record.status)
                    }
                    ClusterArtifactResidencyStatus::PullRequired if request.allow_pull_staging => {
                        Some(record.status)
                    }
                    ClusterArtifactResidencyStatus::Refused => {
                        refusals.push(WholeRequestSchedulingRefusal::for_node(
                            node_id.clone(),
                            WholeRequestSchedulingRefusalCode::ArtifactUnavailable,
                            format!(
                                "candidate node `{}` refuses served artifact `{served_artifact_digest}`",
                                node_id.as_str()
                            ),
                        ));
                        continue;
                    }
                    ClusterArtifactResidencyStatus::CopyRequired
                    | ClusterArtifactResidencyStatus::PullRequired => {
                        refusals.push(WholeRequestSchedulingRefusal::for_node(
                            node_id.clone(),
                            WholeRequestSchedulingRefusalCode::ArtifactStagingNotAllowed,
                            format!(
                                "candidate node `{}` requires disallowed artifact staging for `{served_artifact_digest}`",
                                node_id.as_str()
                            ),
                        ));
                        continue;
                    }
                }
            }
            None => None,
        };

        let link_key = ClusterLinkKey::new(request.scheduler_node_id.clone(), node_id.clone());
        let Some(link) = state.links().get(&link_key) else {
            refusals.push(WholeRequestSchedulingRefusal::for_node(
                node_id.clone(),
                WholeRequestSchedulingRefusalCode::TransportMissing,
                format!(
                    "candidate node `{}` has no authoritative transport fact from scheduler `{}`",
                    node_id.as_str(),
                    request.scheduler_node_id.as_str()
                ),
            ));
            continue;
        };
        if !transport_is_schedulable(link) {
            refusals.push(WholeRequestSchedulingRefusal::for_node(
                node_id.clone(),
                WholeRequestSchedulingRefusalCode::TransportUnavailable,
                format!(
                    "candidate node `{}` is reachable only through unschedulable transport state `{}`",
                    node_id.as_str(),
                    link_status_name(link.status)
                ),
            ));
            continue;
        }

        candidates.push(SchedulerCandidate {
            node_id: node_id.clone(),
            membership,
            telemetry,
            link,
            backend_readiness,
            artifact_status,
            exo_hint_match: exo_hint_matches(&exo_placement_hint, node_id),
        });
    }

    candidates.sort_by(candidate_order);
    let Some(best) = candidates.into_iter().next() else {
        return Err(Box::new(WholeRequestSchedulingFailure {
            code: WholeRequestSchedulingFailureCode::NoEligibleRemoteNode,
            detail: format!(
                "no remote candidate satisfies whole-request scheduling for backend `{}`",
                request.requested_backend
            ),
            cluster_id: state.cluster_id().clone(),
            scheduler_node_id: request.scheduler_node_id.clone(),
            requested_backend: request.requested_backend.clone(),
            cluster_state_digest,
            topology_digest,
            artifact_residency_digest,
            policy_digests: request.policy_digests.clone(),
            communication_eligibility: communication_eligibility.clone(),
            refusals,
        }));
    };

    let selected_device = remote_device_inventory_for_candidate(
        &best.node_id,
        request.requested_backend.as_str(),
        best.telemetry,
    );
    let execution_topology = ExecutionTopologyPlan::single_device(
        request.requested_backend.clone(),
        selected_device.clone(),
    );
    let mut selected_node =
        RuntimeClusterSelectedNode::new(best.node_id.as_str(), request.requested_backend.clone())
            .with_device_inventory(selected_device.clone())
            .with_role(node_role_name(best.membership.identity.role))
            .with_stable_device_id(selected_device.stable_device_id.clone())
            .with_artifact_residency(runtime_artifact_residency(best.artifact_status));
    if let Some(topology_key) = &selected_device.topology_key {
        selected_node = selected_node.with_topology_key(topology_key.clone());
    }
    if let Some(served_artifact_digest) = &request.served_artifact_digest {
        selected_node = selected_node.with_served_artifact_digest(served_artifact_digest.clone());
    }

    let exo_selection_note = exo_selection_note(&exo_placement_hint, &best);
    let selection_notes = selection_notes_for_candidate(&best, exo_selection_note.as_ref());
    let degraded_reason = selection_notes
        .iter()
        .filter(|note| {
            !matches!(
                note.code,
                WholeRequestSchedulingSelectionCode::SelectedBestCandidate
                    | WholeRequestSchedulingSelectionCode::ExoPlacementHintAccepted
                    | WholeRequestSchedulingSelectionCode::ExoPlacementHintIgnored
            )
        })
        .map(|note| note.detail.as_str())
        .collect::<Vec<_>>()
        .join("; ");

    let mut cluster_execution = ClusterExecutionContext::new(
        state.cluster_id().as_str(),
        cluster_state_digest,
        topology_digest,
        request.scheduler_node_id.as_str(),
        runtime_transport_class(best.link.transport),
        ClusterExecutionDisposition::RemoteWholeRequest,
    )
    .with_communication_eligibility(communication_eligibility)
    .with_execution_topology(execution_topology.clone())
    .with_selected_nodes(vec![selected_node])
    .with_policy_digest(ClusterPolicyDigest::new(
        ClusterPolicyDigestKind::Admission,
        state.admission_policy().stable_digest(),
    ));
    cluster_execution = cluster_execution
        .with_command_provenance(command_provenance_for_request(state, request, &best));
    if let Some(artifact_residency_digest) = artifact_residency_digest {
        cluster_execution =
            cluster_execution.with_artifact_residency_digest(artifact_residency_digest);
    }
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
    if let EvaluatedExoPlacementHint::Valid(valid_hint) = &exo_placement_hint {
        cluster_execution = cluster_execution.with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Placement,
            valid_hint.hint.stable_digest(),
        ));
    }
    if let Some(exo_selection_note) = &exo_selection_note {
        cluster_execution =
            cluster_execution.with_placement_diagnostic(exo_selection_note.detail.clone());
    }
    if !degraded_reason.is_empty() {
        cluster_execution = cluster_execution.with_degraded_reason(degraded_reason);
    }

    Ok(WholeRequestClusterSchedule {
        cluster_id: state.cluster_id().clone(),
        scheduler_node_id: request.scheduler_node_id.clone(),
        selected_node_id: best.node_id.clone(),
        runtime_backend: request.requested_backend.clone(),
        selected_device,
        execution_topology,
        cluster_execution,
        selection_notes,
        refusals,
    })
}

struct SchedulerCandidate<'a> {
    node_id: NodeId,
    membership: &'a ClusterMembershipRecord,
    telemetry: &'a ClusterNodeTelemetry,
    link: &'a ClusterLink,
    backend_readiness: ClusterBackendReadinessStatus,
    artifact_status: Option<ClusterArtifactResidencyStatus>,
    exo_hint_match: bool,
}

fn candidate_order(
    left: &SchedulerCandidate<'_>,
    right: &SchedulerCandidate<'_>,
) -> std::cmp::Ordering {
    candidate_backend_rank(left.backend_readiness)
        .cmp(&candidate_backend_rank(right.backend_readiness))
        .then(
            candidate_artifact_rank(left.artifact_status)
                .cmp(&candidate_artifact_rank(right.artifact_status)),
        )
        .then(candidate_link_rank(left.link.status).cmp(&candidate_link_rank(right.link.status)))
        .then(
            candidate_stability_rank(left.telemetry.stability)
                .cmp(&candidate_stability_rank(right.telemetry.stability)),
        )
        .then(exo_hint_rank(left.exo_hint_match).cmp(&exo_hint_rank(right.exo_hint_match)))
        .then_with(|| {
            right
                .telemetry
                .free_memory_bytes
                .unwrap_or(0)
                .cmp(&left.telemetry.free_memory_bytes.unwrap_or(0))
        })
        .then_with(|| {
            left.link
                .latency_us
                .unwrap_or(u64::MAX)
                .cmp(&right.link.latency_us.unwrap_or(u64::MAX))
        })
        .then_with(|| {
            right
                .link
                .bandwidth_mbps
                .unwrap_or(0)
                .cmp(&left.link.bandwidth_mbps.unwrap_or(0))
        })
        .then_with(|| left.node_id.as_str().cmp(right.node_id.as_str()))
}

fn command_provenance_for_request(
    state: &ClusterState,
    request: &WholeRequestSchedulingRequest,
    best: &SchedulerCandidate<'_>,
) -> Vec<ClusterCommandProvenanceEvidence> {
    let mut provenance = Vec::new();

    if let Some(authorization) = state.admission_policy_provenance() {
        provenance.push(runtime_command_provenance(
            ClusterAdmissionFactKind::AdmissionPolicy,
            authorization,
        ));
    }
    if let Some(authorization) = state.membership_provenance(&request.scheduler_node_id) {
        provenance.push(
            runtime_command_provenance(
                ClusterAdmissionFactKind::SchedulerMembership,
                authorization,
            )
            .with_target_node_id(request.scheduler_node_id.as_str()),
        );
    }
    if let Some(authorization) = state.membership_provenance(&best.node_id) {
        provenance.push(
            runtime_command_provenance(ClusterAdmissionFactKind::SelectedMembership, authorization)
                .with_target_node_id(best.node_id.as_str()),
        );
    }
    if state.discovery_candidates().contains_key(&best.node_id) {
        if let Some(authorization) = state.discovery_candidate_provenance(&best.node_id) {
            provenance.push(
                runtime_command_provenance(
                    ClusterAdmissionFactKind::SelectedCandidateAdmission,
                    authorization,
                )
                .with_target_node_id(best.node_id.as_str()),
            );
        }
    }
    if let Some(served_artifact_digest) = &request.served_artifact_digest {
        let artifact_key =
            ClusterArtifactResidencyKey::new(best.node_id.clone(), served_artifact_digest.clone());
        if let Some(authorization) = state.artifact_residency_provenance(&artifact_key) {
            provenance.push(
                runtime_command_provenance(
                    ClusterAdmissionFactKind::ArtifactResidency,
                    authorization,
                )
                .with_target_node_id(best.node_id.as_str()),
            );
        }
    }
    if let Some(authorization) = state.leadership_provenance() {
        provenance.push(runtime_command_provenance(
            ClusterAdmissionFactKind::Leadership,
            authorization,
        ));
    }

    provenance
}

fn runtime_command_provenance(
    fact_kind: ClusterAdmissionFactKind,
    authorization: &crate::ClusterCommandAuthorization,
) -> ClusterCommandProvenanceEvidence {
    ClusterCommandProvenanceEvidence::new(
        fact_kind,
        authorization.submitter_node_id.as_str(),
        runtime_authority_scope(authorization.authority_scope),
        authorization.command_digest.clone(),
        authorization.stable_digest(),
        authorization.authorization_policy_digest.clone(),
    )
}

const fn runtime_authority_scope(
    scope: crate::ClusterCommandAuthorityScope,
) -> ClusterCommandAuthorityScopeEvidence {
    match scope {
        crate::ClusterCommandAuthorityScope::CoordinatorOnly => {
            ClusterCommandAuthorityScopeEvidence::CoordinatorOnly
        }
        crate::ClusterCommandAuthorityScope::ClusterMember => {
            ClusterCommandAuthorityScopeEvidence::ClusterMember
        }
        crate::ClusterCommandAuthorityScope::SelfNode => {
            ClusterCommandAuthorityScopeEvidence::SelfNode
        }
        crate::ClusterCommandAuthorityScope::LinkPeer => {
            ClusterCommandAuthorityScopeEvidence::LinkPeer
        }
        crate::ClusterCommandAuthorityScope::ProposedLeader => {
            ClusterCommandAuthorityScopeEvidence::ProposedLeader
        }
    }
}

const fn candidate_backend_rank(readiness: ClusterBackendReadinessStatus) -> u8 {
    match readiness {
        ClusterBackendReadinessStatus::Ready => 0,
        ClusterBackendReadinessStatus::Degraded => 1,
        ClusterBackendReadinessStatus::Refused | ClusterBackendReadinessStatus::Unknown => 2,
    }
}

const fn candidate_artifact_rank(status: Option<ClusterArtifactResidencyStatus>) -> u8 {
    match status {
        Some(ClusterArtifactResidencyStatus::Resident) | None => 0,
        Some(ClusterArtifactResidencyStatus::CopyRequired) => 1,
        Some(ClusterArtifactResidencyStatus::PullRequired) => 2,
        Some(ClusterArtifactResidencyStatus::Refused) => 3,
    }
}

const fn candidate_link_rank(status: ClusterLinkStatus) -> u8 {
    match status {
        ClusterLinkStatus::Healthy => 0,
        ClusterLinkStatus::Degraded => 1,
        ClusterLinkStatus::Pending => 2,
        ClusterLinkStatus::Disconnected => 3,
    }
}

const fn candidate_stability_rank(stability: ClusterStabilityPosture) -> u8 {
    match stability {
        ClusterStabilityPosture::Stable => 0,
        ClusterStabilityPosture::Flaky => 1,
        ClusterStabilityPosture::Unstable => 2,
    }
}

const fn exo_hint_rank(exo_hint_match: bool) -> u8 {
    if exo_hint_match { 0 } else { 1 }
}

const fn transport_is_schedulable(link: &ClusterLink) -> bool {
    matches!(
        link.status,
        ClusterLinkStatus::Healthy | ClusterLinkStatus::Degraded
    ) && !matches!(link.stability, ClusterStabilityPosture::Unstable)
}

fn runtime_transport_class(transport: ClusterTransportClass) -> RuntimeClusterTransportClass {
    match transport {
        ClusterTransportClass::LoopbackUdp => RuntimeClusterTransportClass::Loopback,
        ClusterTransportClass::LanUdp => RuntimeClusterTransportClass::TrustedLanDatagram,
        ClusterTransportClass::Tcp | ClusterTransportClass::Rdma => {
            RuntimeClusterTransportClass::TrustedLanStream
        }
        ClusterTransportClass::Unknown => RuntimeClusterTransportClass::Mixed,
    }
}

fn runtime_artifact_residency(
    artifact_status: Option<ClusterArtifactResidencyStatus>,
) -> RuntimeArtifactResidencyDisposition {
    match artifact_status.unwrap_or(ClusterArtifactResidencyStatus::Resident) {
        ClusterArtifactResidencyStatus::Resident => RuntimeArtifactResidencyDisposition::Resident,
        ClusterArtifactResidencyStatus::CopyRequired => {
            RuntimeArtifactResidencyDisposition::CopyRequired
        }
        ClusterArtifactResidencyStatus::PullRequired => {
            RuntimeArtifactResidencyDisposition::PullRequired
        }
        ClusterArtifactResidencyStatus::Refused => RuntimeArtifactResidencyDisposition::Refused,
    }
}

fn remote_device_inventory_for_candidate(
    node_id: &NodeId,
    runtime_backend: &str,
    telemetry: &ClusterNodeTelemetry,
) -> DeviceInventoryQualifiers {
    DeviceInventoryQualifiers {
        stable_device_id: format!("cluster-node:{}:{runtime_backend}", node_id.as_str()),
        topology_key: None,
        performance_class: remote_performance_class(runtime_backend, telemetry),
        memory_class: remote_memory_class(runtime_backend, telemetry),
        total_memory_bytes: telemetry.total_memory_bytes,
        free_memory_bytes: telemetry.free_memory_bytes,
    }
}

fn remote_performance_class(
    runtime_backend: &str,
    telemetry: &ClusterNodeTelemetry,
) -> DevicePerformanceClass {
    if runtime_backend == "cpu" {
        DevicePerformanceClass::Reference
    } else if runtime_backend == "metal" {
        DevicePerformanceClass::IntegratedAccelerator
    } else if matches!(
        runtime_backend,
        "cuda" | "rocm" | "amd" | "amd_kfd" | "amd_userspace"
    ) || telemetry.accelerator_count.unwrap_or_default() > 0
    {
        DevicePerformanceClass::DiscreteAccelerator
    } else {
        DevicePerformanceClass::IntegratedAccelerator
    }
}

fn remote_memory_class(
    runtime_backend: &str,
    telemetry: &ClusterNodeTelemetry,
) -> DeviceMemoryClass {
    if runtime_backend == "cpu" {
        DeviceMemoryClass::HostOnly
    } else if runtime_backend == "metal" {
        DeviceMemoryClass::SharedHostDevice
    } else if telemetry.accelerator_count.unwrap_or_default() > 0 {
        DeviceMemoryClass::DedicatedDevice
    } else {
        DeviceMemoryClass::SharedHostDevice
    }
}

fn selection_notes_for_candidate(
    candidate: &SchedulerCandidate<'_>,
    exo_selection_note: Option<&WholeRequestSchedulingSelectionNote>,
) -> Vec<WholeRequestSchedulingSelectionNote> {
    let mut notes = vec![WholeRequestSchedulingSelectionNote::new(
        WholeRequestSchedulingSelectionCode::SelectedBestCandidate,
        format!(
            "selected remote node `{}` for whole-request execution",
            candidate.node_id.as_str()
        ),
    )];
    if let Some(exo_selection_note) = exo_selection_note {
        notes.push(exo_selection_note.clone());
    }
    match candidate.artifact_status {
        Some(ClusterArtifactResidencyStatus::CopyRequired) => {
            notes.push(WholeRequestSchedulingSelectionNote::new(
                WholeRequestSchedulingSelectionCode::ArtifactCopyRequired,
                format!(
                    "selected node `{}` requires peer-copy artifact staging before execution",
                    candidate.node_id.as_str()
                ),
            ));
        }
        Some(ClusterArtifactResidencyStatus::PullRequired) => {
            notes.push(WholeRequestSchedulingSelectionNote::new(
                WholeRequestSchedulingSelectionCode::ArtifactPullRequired,
                format!(
                    "selected node `{}` requires pull-based artifact staging before execution",
                    candidate.node_id.as_str()
                ),
            ));
        }
        Some(
            ClusterArtifactResidencyStatus::Resident | ClusterArtifactResidencyStatus::Refused,
        )
        | None => {}
    }
    if candidate.backend_readiness == ClusterBackendReadinessStatus::Degraded {
        notes.push(WholeRequestSchedulingSelectionNote::new(
            WholeRequestSchedulingSelectionCode::BackendDegraded,
            format!(
                "selected node `{}` is only backend-ready in degraded posture",
                candidate.node_id.as_str()
            ),
        ));
    }
    if candidate.link.status == ClusterLinkStatus::Degraded {
        notes.push(WholeRequestSchedulingSelectionNote::new(
            WholeRequestSchedulingSelectionCode::TransportDegraded,
            format!(
                "selected node `{}` is reachable only over a degraded transport path",
                candidate.node_id.as_str()
            ),
        ));
    }
    if candidate.telemetry.stability == ClusterStabilityPosture::Flaky {
        notes.push(WholeRequestSchedulingSelectionNote::new(
            WholeRequestSchedulingSelectionCode::NodeFlaky,
            format!(
                "selected node `{}` is only marked flaky rather than stable",
                candidate.node_id.as_str()
            ),
        ));
    }
    notes
}

#[derive(Clone, Debug)]
enum EvaluatedExoPlacementHint {
    None,
    Ignored { detail: String },
    Valid(ValidatedExoPlacementHint),
}

#[derive(Clone, Debug)]
struct ValidatedExoPlacementHint {
    hint: ExoPlacementHint,
    suggested_node_ids: BTreeSet<NodeId>,
}

fn evaluate_exo_placement_hint(
    state: &ClusterState,
    exo_placement_hint: Option<&ExoPlacementHint>,
) -> EvaluatedExoPlacementHint {
    let Some(exo_placement_hint) = exo_placement_hint else {
        return EvaluatedExoPlacementHint::None;
    };
    if &exo_placement_hint.cluster_id != state.cluster_id() {
        return EvaluatedExoPlacementHint::Ignored {
            detail: format!(
                "ignored Exo placement hint from `{}` because it targeted cluster `{}` instead of `{}`",
                exo_placement_hint.source_id,
                exo_placement_hint.cluster_id.as_str(),
                state.cluster_id().as_str()
            ),
        };
    }
    let current_topology_digest = state.topology_digest();
    if exo_placement_hint.topology_digest != current_topology_digest {
        return EvaluatedExoPlacementHint::Ignored {
            detail: format!(
                "ignored Exo placement hint from `{}` because topology digest `{}` no longer matches current `{}`",
                exo_placement_hint.source_id,
                exo_placement_hint.topology_digest,
                current_topology_digest
            ),
        };
    }
    if exo_placement_hint.suggested_node_ids.is_empty() {
        return EvaluatedExoPlacementHint::Ignored {
            detail: format!(
                "ignored Exo placement hint from `{}` because it contained no suggested nodes",
                exo_placement_hint.source_id
            ),
        };
    }
    EvaluatedExoPlacementHint::Valid(ValidatedExoPlacementHint {
        hint: exo_placement_hint.clone(),
        suggested_node_ids: exo_placement_hint
            .suggested_node_ids
            .iter()
            .cloned()
            .collect(),
    })
}

fn exo_hint_matches(exo_placement_hint: &EvaluatedExoPlacementHint, node_id: &NodeId) -> bool {
    match exo_placement_hint {
        EvaluatedExoPlacementHint::Valid(valid_hint) => {
            valid_hint.suggested_node_ids.contains(node_id)
        }
        EvaluatedExoPlacementHint::None | EvaluatedExoPlacementHint::Ignored { .. } => false,
    }
}

fn exo_selection_note(
    exo_placement_hint: &EvaluatedExoPlacementHint,
    best: &SchedulerCandidate<'_>,
) -> Option<WholeRequestSchedulingSelectionNote> {
    match exo_placement_hint {
        EvaluatedExoPlacementHint::None => None,
        EvaluatedExoPlacementHint::Ignored { detail } => {
            Some(WholeRequestSchedulingSelectionNote::new(
                WholeRequestSchedulingSelectionCode::ExoPlacementHintIgnored,
                detail.clone(),
            ))
        }
        EvaluatedExoPlacementHint::Valid(valid_hint) if best.exo_hint_match => {
            Some(WholeRequestSchedulingSelectionNote::new(
                WholeRequestSchedulingSelectionCode::ExoPlacementHintAccepted,
                format!(
                    "accepted Exo placement hint from `{}` for selected node `{}` without widening eligibility",
                    valid_hint.hint.source_id,
                    best.node_id.as_str()
                ),
            ))
        }
        EvaluatedExoPlacementHint::Valid(valid_hint) => {
            let hinted_nodes = valid_hint
                .hint
                .suggested_node_ids
                .iter()
                .map(|node_id| node_id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Some(WholeRequestSchedulingSelectionNote::new(
                WholeRequestSchedulingSelectionCode::ExoPlacementHintIgnored,
                format!(
                    "ignored Exo placement hint from `{}` because authoritative local ordering kept `{}` ahead of hinted nodes [{}]",
                    valid_hint.hint.source_id,
                    best.node_id.as_str(),
                    hinted_nodes
                ),
            ))
        }
    }
}

const fn node_role_name(role: NodeRole) -> &'static str {
    match role {
        NodeRole::CoordinatorOnly => "coordinator_only",
        NodeRole::ExecutorOnly => "executor_only",
        NodeRole::Mixed => "mixed",
    }
}

const fn link_status_name(status: ClusterLinkStatus) -> &'static str {
    match status {
        ClusterLinkStatus::Pending => "pending",
        ClusterLinkStatus::Healthy => "healthy",
        ClusterLinkStatus::Degraded => "degraded",
        ClusterLinkStatus::Disconnected => "disconnected",
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic_in_result_fn)]
mod tests {
    use std::io::Error;

    use psionic_runtime::{
        ClusterAdmissionFactKind, ClusterCommunicationClass, ClusterExecutionCapabilityProfile,
        ClusterExecutionLane, ClusterPolicyDigest, ClusterPolicyDigestKind, ExecutionTopologyKind,
    };

    use crate::{
        AdmissionToken, ClusterArtifactReference, ClusterArtifactResidencyKey,
        ClusterArtifactResidencyStatus, ClusterBackendReadinessStatus, ClusterCandidateRevocation,
        ClusterCandidateRevocationReason, ClusterCommandAuthorityScope,
        ClusterCommandAuthorization, ClusterDiscoveredCandidateRecord,
        ClusterDiscoveredCandidateStatus, ClusterLeadershipRecord, ClusterLink, ClusterLinkStatus,
        ClusterMembershipRecord, ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity,
        ClusterNodeTelemetry, ClusterSnapshot, ClusterStabilityPosture, ClusterState, ClusterTerm,
        ClusterTransportClass, NodeEpoch, NodeRole,
    };

    use super::{
        ExoPlacementHint, WholeRequestClusterSchedule, WholeRequestSchedulingFailureCode,
        WholeRequestSchedulingRefusalCode, WholeRequestSchedulingRequest,
        schedule_remote_whole_request,
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

    fn healthy_link(left: &str, right: &str) -> ClusterLink {
        ClusterLink::new(
            crate::NodeId::new(left),
            crate::NodeId::new(right),
            ClusterTransportClass::LanUdp,
            ClusterLinkStatus::Healthy,
        )
    }

    fn ready_cuda_telemetry(node_id: &str, free_memory_bytes: u64) -> ClusterNodeTelemetry {
        ClusterNodeTelemetry::new(crate::NodeId::new(node_id))
            .with_memory(Some(64 * 1024 * 1024 * 1024), Some(free_memory_bytes))
            .with_cpu_logical_cores(16)
            .with_accelerator_count(1)
            .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready)
    }

    fn ready_metal_telemetry(node_id: &str, free_memory_bytes: u64) -> ClusterNodeTelemetry {
        ClusterNodeTelemetry::new(crate::NodeId::new(node_id))
            .with_memory(Some(64 * 1024 * 1024 * 1024), Some(free_memory_bytes))
            .with_cpu_logical_cores(16)
            .with_accelerator_count(1)
            .with_backend_readiness("metal", ClusterBackendReadinessStatus::Ready)
    }

    fn cuda_remote_dispatch_capability_profile() -> ClusterExecutionCapabilityProfile {
        ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![ClusterExecutionLane::RemoteWholeRequest])
            .with_detail(
                "backend `cuda` declares whole-request remote dispatch on ready cluster nodes",
            )
    }

    fn metal_cluster_blocked_capability_profile() -> ClusterExecutionCapabilityProfile {
        ClusterExecutionCapabilityProfile::new("metal")
            .with_detail(super::METAL_CLUSTER_BLOCK_DETAIL)
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
            submitter_node_id: crate::NodeId::new(submitter_node_id),
            submitter_role: NodeRole::Mixed,
            submitter_membership_status: ClusterMembershipStatus::Ready,
            coordinator_authority: None,
        }
    }

    fn sample_snapshot() -> ClusterSnapshot {
        let cluster_id = sample_cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships.insert(
            crate::NodeId::new("scheduler"),
            ready_membership(&cluster_id, "scheduler", NodeRole::Mixed),
        );
        snapshot.memberships.insert(
            crate::NodeId::new("worker-a"),
            ready_membership(&cluster_id, "worker-a", NodeRole::Mixed),
        );
        snapshot.memberships.insert(
            crate::NodeId::new("worker-b"),
            ready_membership(&cluster_id, "worker-b", NodeRole::ExecutorOnly),
        );
        snapshot.telemetry.insert(
            crate::NodeId::new("worker-a"),
            ready_cuda_telemetry("worker-a", 32 * 1024 * 1024 * 1024),
        );
        snapshot.telemetry.insert(
            crate::NodeId::new("worker-b"),
            ready_cuda_telemetry("worker-b", 48 * 1024 * 1024 * 1024),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("scheduler"),
                crate::NodeId::new("worker-a"),
            ),
            healthy_link("scheduler", "worker-a")
                .with_latency_us(700)
                .with_bandwidth_mbps(1000),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("scheduler"),
                crate::NodeId::new("worker-b"),
            ),
            healthy_link("scheduler", "worker-b")
                .with_latency_us(500)
                .with_bandwidth_mbps(1000),
        );
        snapshot
    }

    fn accepted_discovery_candidate(node_id: &str) -> ClusterDiscoveredCandidateRecord {
        ClusterDiscoveredCandidateRecord {
            candidate: crate::ClusterDiscoveryCandidate::new(
                sample_cluster_id(),
                ClusterNamespace::new("cluster-lan"),
                crate::NodeId::new(node_id),
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

    fn revoked_discovery_candidate(node_id: &str) -> ClusterDiscoveredCandidateRecord {
        let mut candidate = accepted_discovery_candidate(node_id);
        candidate.status = ClusterDiscoveredCandidateStatus::Revoked;
        candidate.revocation = Some(ClusterCandidateRevocation {
            reason: ClusterCandidateRevocationReason::PolicyChanged,
            policy_digest: String::from("admission-policy-digest"),
            trust_policy_digest: String::from("trust-policy-digest"),
            trust_assessment_digest: String::from("trust-assessment-digest"),
            revoked_at_ms: 30_000,
            detail: Some(String::from("trust_bundle_rotation")),
        });
        candidate.detail = Some(String::from("trust_bundle_rotation"));
        candidate
    }

    #[test]
    fn whole_request_scheduler_prefers_resident_candidate_and_emits_single_device_topology()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot.leadership = Some(ClusterLeadershipRecord::new(
            ClusterTerm::initial(),
            crate::NodeId::new("scheduler"),
            crate::ClusterEventIndex::initial(),
        ));
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-a"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-a"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-b"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-b"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::CopyRequired,
            ),
        );
        snapshot.membership_provenance.insert(
            crate::NodeId::new("scheduler"),
            sample_command_authorization(
                "scheduler",
                ClusterCommandAuthorityScope::SelfNode,
                "scheduler-membership-command",
            ),
        );
        snapshot.membership_provenance.insert(
            crate::NodeId::new("worker-a"),
            sample_command_authorization(
                "worker-a",
                ClusterCommandAuthorityScope::SelfNode,
                "worker-a-membership-command",
            ),
        );
        snapshot.artifact_residency_provenance.insert(
            ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-a"), "artifact-1"),
            sample_command_authorization(
                "worker-a",
                ClusterCommandAuthorityScope::SelfNode,
                "worker-a-artifact-command",
            ),
        );
        snapshot.leadership_provenance = Some(sample_command_authorization(
            "scheduler",
            ClusterCommandAuthorityScope::ProposedLeader,
            "leadership-command",
        ));

        let state = ClusterState::from_snapshot(snapshot);
        let request = WholeRequestSchedulingRequest::new(crate::NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile())
            .with_served_artifact_digest("artifact-1")
            .with_minimum_free_memory_bytes(16 * 1024 * 1024 * 1024)
            .requiring_accelerator()
            .with_policy_digest(ClusterPolicyDigest::new(
                ClusterPolicyDigestKind::Placement,
                "placement-policy-digest",
            ));

        let schedule = schedule_remote_whole_request(&state, &request)
            .map_err(|err| fixture_error(&format!("schedule should succeed: {err:?}")))?;

        assert_eq!(schedule.selected_node_id, crate::NodeId::new("worker-a"));
        assert_eq!(schedule.runtime_backend, "cuda");
        assert_eq!(
            schedule.execution_topology.kind,
            ExecutionTopologyKind::SingleDevice
        );
        assert_eq!(
            schedule.selected_device.stable_device_id,
            String::from("cluster-node:worker-a:cuda")
        );
        assert_eq!(
            schedule
                .cluster_execution
                .selected_nodes
                .first()
                .and_then(|node| node.stable_device_id.as_deref()),
            Some("cluster-node:worker-a:cuda")
        );
        assert_eq!(
            schedule
                .cluster_execution
                .commit_authority
                .as_ref()
                .map(|authority| authority.coordinator_node_id.as_str()),
            Some("scheduler")
        );
        assert_eq!(
            schedule
                .cluster_execution
                .commit_authority
                .as_ref()
                .map(|authority| authority.term),
            Some(1)
        );
        assert!(
            schedule
                .cluster_execution
                .policy_digests
                .iter()
                .any(|digest| digest.kind == ClusterPolicyDigestKind::Authority),
            "cluster execution should carry authority policy truth"
        );
        assert_eq!(
            schedule
                .cluster_execution
                .selected_nodes
                .first()
                .and_then(|node| node.artifact_residency),
            Some(psionic_runtime::ClusterArtifactResidencyDisposition::Resident)
        );
        assert_eq!(schedule.cluster_execution.command_provenance.len(), 4);
        assert_eq!(
            schedule.cluster_execution.command_provenance[0].fact_kind,
            ClusterAdmissionFactKind::SchedulerMembership
        );
        assert_eq!(
            schedule.cluster_execution.command_provenance[1].fact_kind,
            ClusterAdmissionFactKind::SelectedMembership
        );
        assert_eq!(
            schedule.cluster_execution.command_provenance[2].fact_kind,
            ClusterAdmissionFactKind::ArtifactResidency
        );
        assert_eq!(
            schedule.cluster_execution.command_provenance[3].fact_kind,
            ClusterAdmissionFactKind::Leadership
        );
        Ok(())
    }

    #[test]
    fn whole_request_scheduler_carries_admission_policy_and_candidate_provenance()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot.leadership = Some(ClusterLeadershipRecord::new(
            ClusterTerm::initial(),
            crate::NodeId::new("scheduler"),
            crate::ClusterEventIndex::initial(),
        ));
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-a"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-a"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.membership_provenance.insert(
            crate::NodeId::new("scheduler"),
            sample_command_authorization(
                "scheduler",
                ClusterCommandAuthorityScope::SelfNode,
                "scheduler-membership-command",
            ),
        );
        snapshot.membership_provenance.insert(
            crate::NodeId::new("worker-a"),
            sample_command_authorization(
                "worker-a",
                ClusterCommandAuthorityScope::SelfNode,
                "worker-a-membership-command",
            ),
        );
        snapshot.discovery_candidates.insert(
            crate::NodeId::new("worker-a"),
            accepted_discovery_candidate("worker-a"),
        );
        snapshot.discovery_candidate_provenance.insert(
            crate::NodeId::new("worker-a"),
            sample_command_authorization(
                "scheduler",
                ClusterCommandAuthorityScope::CoordinatorOnly,
                "worker-a-candidate-admission",
            ),
        );
        snapshot.admission_policy_provenance = Some(sample_command_authorization(
            "scheduler",
            ClusterCommandAuthorityScope::CoordinatorOnly,
            "admission-policy-command",
        ));
        snapshot.artifact_residency_provenance.insert(
            ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-a"), "artifact-1"),
            sample_command_authorization(
                "worker-a",
                ClusterCommandAuthorityScope::SelfNode,
                "worker-a-artifact-command",
            ),
        );
        snapshot.leadership_provenance = Some(sample_command_authorization(
            "scheduler",
            ClusterCommandAuthorityScope::ProposedLeader,
            "leadership-command",
        ));

        let state = ClusterState::from_snapshot(snapshot);
        let request = WholeRequestSchedulingRequest::new(crate::NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile())
            .with_served_artifact_digest("artifact-1");

        let schedule = schedule_remote_whole_request(&state, &request)
            .map_err(|err| fixture_error(&format!("schedule should succeed: {err:?}")))?;

        assert!(
            schedule
                .cluster_execution
                .policy_digests
                .iter()
                .any(|digest| digest.kind == ClusterPolicyDigestKind::Admission),
            "cluster execution should retain admission policy digest truth"
        );
        assert_eq!(schedule.cluster_execution.command_provenance.len(), 6);
        assert_eq!(
            schedule.cluster_execution.command_provenance[0].fact_kind,
            ClusterAdmissionFactKind::AdmissionPolicy
        );
        assert_eq!(
            schedule.cluster_execution.command_provenance[3].fact_kind,
            ClusterAdmissionFactKind::SelectedCandidateAdmission
        );
        Ok(())
    }

    #[test]
    fn whole_request_scheduler_breaks_ties_by_node_id_deterministically()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot
            .links
            .get_mut(&crate::ClusterLinkKey::new(
                crate::NodeId::new("scheduler"),
                crate::NodeId::new("worker-a"),
            ))
            .ok_or_else(|| fixture_error("worker-a link"))?
            .latency_us = Some(500);
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-a"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-a"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-b"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-b"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.telemetry.insert(
            crate::NodeId::new("worker-b"),
            ready_cuda_telemetry("worker-b", 32 * 1024 * 1024 * 1024),
        );

        let state = ClusterState::from_snapshot(snapshot);
        let request = WholeRequestSchedulingRequest::new(crate::NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile())
            .with_served_artifact_digest("artifact-1");

        let WholeRequestClusterSchedule {
            selected_node_id, ..
        } = schedule_remote_whole_request(&state, &request)
            .map_err(|err| fixture_error(&format!("schedule should succeed: {err:?}")))?;
        assert_eq!(selected_node_id, crate::NodeId::new("worker-a"));
        Ok(())
    }

    #[test]
    fn whole_request_scheduler_accepts_bounded_exo_hint_for_tie_break()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot
            .links
            .get_mut(&crate::ClusterLinkKey::new(
                crate::NodeId::new("scheduler"),
                crate::NodeId::new("worker-a"),
            ))
            .ok_or_else(|| fixture_error("worker-a link"))?
            .latency_us = Some(500);
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-a"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-a"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-b"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-b"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.telemetry.insert(
            crate::NodeId::new("worker-b"),
            ready_cuda_telemetry("worker-b", 32 * 1024 * 1024 * 1024),
        );

        let state = ClusterState::from_snapshot(snapshot);
        let request = WholeRequestSchedulingRequest::new(crate::NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile())
            .with_served_artifact_digest("artifact-1")
            .with_exo_placement_hint(
                ExoPlacementHint::new(
                    sample_cluster_id(),
                    "exo-coordinator",
                    state.topology_digest(),
                    vec![crate::NodeId::new("worker-b")],
                )
                .with_detail("prefer the lower-latency remote worker"),
            );

        let WholeRequestClusterSchedule {
            selected_node_id,
            selection_notes,
            cluster_execution,
            ..
        } = schedule_remote_whole_request(&state, &request)
            .map_err(|err| fixture_error(&format!("schedule should succeed: {err:?}")))?;
        assert_eq!(selected_node_id, crate::NodeId::new("worker-b"));
        assert!(selection_notes.iter().any(|note| {
            note.code == super::WholeRequestSchedulingSelectionCode::ExoPlacementHintAccepted
        }));
        assert!(
            cluster_execution
                .policy_digests
                .iter()
                .any(|digest| digest.kind == ClusterPolicyDigestKind::Placement),
            "cluster execution should retain the accepted placement hint digest"
        );
        assert!(
            cluster_execution
                .placement_diagnostics
                .iter()
                .any(|detail| detail.contains("accepted Exo placement hint")),
            "cluster execution should surface accepted hint diagnostics"
        );
        Ok(())
    }

    #[test]
    fn whole_request_scheduler_ignores_stale_exo_hint_and_preserves_local_choice()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-a"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-a"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-b"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-b"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );

        let state = ClusterState::from_snapshot(snapshot);
        let request = WholeRequestSchedulingRequest::new(crate::NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile())
            .with_served_artifact_digest("artifact-1")
            .with_exo_placement_hint(ExoPlacementHint::new(
                sample_cluster_id(),
                "exo-coordinator",
                "stale-topology-digest",
                vec![crate::NodeId::new("worker-a")],
            ));

        let WholeRequestClusterSchedule {
            selected_node_id,
            selection_notes,
            cluster_execution,
            ..
        } = schedule_remote_whole_request(&state, &request)
            .map_err(|err| fixture_error(&format!("schedule should succeed: {err:?}")))?;
        assert_eq!(selected_node_id, crate::NodeId::new("worker-b"));
        assert!(selection_notes.iter().any(|note| {
            note.code == super::WholeRequestSchedulingSelectionCode::ExoPlacementHintIgnored
                && note.detail.contains("stale-topology-digest")
        }));
        assert!(
            !cluster_execution
                .policy_digests
                .iter()
                .any(|digest| digest.kind == ClusterPolicyDigestKind::Placement),
            "ignored hint should not add placement digests"
        );
        assert!(
            cluster_execution
                .placement_diagnostics
                .iter()
                .any(|detail| detail.contains("ignored Exo placement hint")),
            "cluster execution should surface ignored hint diagnostics"
        );
        Ok(())
    }

    #[test]
    fn whole_request_scheduler_surfaces_degraded_selection_reasons()
    -> Result<(), Box<dyn std::error::Error>> {
        let cluster_id = sample_cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships.insert(
            crate::NodeId::new("scheduler"),
            ready_membership(&cluster_id, "scheduler", NodeRole::Mixed),
        );
        snapshot.memberships.insert(
            crate::NodeId::new("worker-a"),
            ready_membership(&cluster_id, "worker-a", NodeRole::ExecutorOnly),
        );
        snapshot.telemetry.insert(
            crate::NodeId::new("worker-a"),
            ClusterNodeTelemetry::new(crate::NodeId::new("worker-a"))
                .with_memory(Some(64 * 1024 * 1024 * 1024), Some(48 * 1024 * 1024 * 1024))
                .with_accelerator_count(1)
                .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Degraded)
                .with_stability_posture(ClusterStabilityPosture::Flaky),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("scheduler"),
                crate::NodeId::new("worker-a"),
            ),
            healthy_link("scheduler", "worker-a")
                .with_latency_us(900)
                .with_bandwidth_mbps(250)
                .with_stability_posture(ClusterStabilityPosture::Flaky)
                .with_link_class(crate::ClusterLinkClass::Wifi),
        );
        snapshot
            .links
            .get_mut(&crate::ClusterLinkKey::new(
                crate::NodeId::new("scheduler"),
                crate::NodeId::new("worker-a"),
            ))
            .ok_or_else(|| fixture_error("worker-a link"))?
            .status = ClusterLinkStatus::Degraded;
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-a"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-a"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::CopyRequired,
            ),
        );

        let state = ClusterState::from_snapshot(snapshot);
        let request = WholeRequestSchedulingRequest::new(crate::NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile())
            .with_served_artifact_digest("artifact-1")
            .requiring_accelerator();

        let schedule = schedule_remote_whole_request(&state, &request)
            .map_err(|err| fixture_error(&format!("schedule should succeed: {err:?}")))?;
        assert_eq!(schedule.selected_node_id, crate::NodeId::new("worker-a"));
        assert!(schedule.selection_notes.iter().any(|note| {
            note.code == super::WholeRequestSchedulingSelectionCode::ArtifactCopyRequired
        }));
        assert!(schedule.selection_notes.iter().any(|note| {
            note.code == super::WholeRequestSchedulingSelectionCode::BackendDegraded
        }));
        assert!(schedule.selection_notes.iter().any(|note| {
            note.code == super::WholeRequestSchedulingSelectionCode::TransportDegraded
        }));
        assert_eq!(
            schedule.cluster_execution.degraded_reason.as_deref(),
            Some(
                "selected node `worker-a` requires peer-copy artifact staging before execution; selected node `worker-a` is only backend-ready in degraded posture; selected node `worker-a` is reachable only over a degraded transport path; selected node `worker-a` is only marked flaky rather than stable"
            )
        );
        Ok(())
    }

    #[test]
    fn whole_request_scheduler_emits_machine_checkable_refusals()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot.telemetry.insert(
            crate::NodeId::new("worker-a"),
            ready_cuda_telemetry("worker-a", 8 * 1024 * 1024 * 1024)
                .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Refused),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-b"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-b"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot
            .links
            .get_mut(&crate::ClusterLinkKey::new(
                crate::NodeId::new("scheduler"),
                crate::NodeId::new("worker-b"),
            ))
            .ok_or_else(|| fixture_error("worker-b link"))?
            .status = ClusterLinkStatus::Disconnected;

        let state = ClusterState::from_snapshot(snapshot);
        let request = WholeRequestSchedulingRequest::new(crate::NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile())
            .with_served_artifact_digest("artifact-1")
            .with_minimum_free_memory_bytes(16 * 1024 * 1024 * 1024)
            .requiring_accelerator();

        let failure = match schedule_remote_whole_request(&state, &request) {
            Ok(schedule) => {
                return Err(fixture_error(&format!(
                    "expected scheduling failure, got {schedule:?}"
                ))
                .into());
            }
            Err(failure) => failure,
        };

        assert_eq!(
            failure.code,
            WholeRequestSchedulingFailureCode::NoEligibleRemoteNode
        );
        assert!(failure.refusals.iter().any(|refusal| {
            refusal.node_id.as_ref() == Some(&crate::NodeId::new("worker-a"))
                && refusal.code == WholeRequestSchedulingRefusalCode::BackendUnavailable
        }));
        assert!(failure.refusals.iter().any(|refusal| {
            refusal.node_id.as_ref() == Some(&crate::NodeId::new("worker-b"))
                && refusal.code == WholeRequestSchedulingRefusalCode::TransportUnavailable
        }));
        Ok(())
    }

    #[test]
    fn whole_request_scheduler_refuses_disallowed_artifact_staging_explicitly()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-a"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-a"),
                ClusterArtifactReference::new("decoder", "artifact-1")
                    .with_provenance_digest("prov-digest")
                    .with_governance_digest("gov-digest"),
                ClusterArtifactResidencyStatus::CopyRequired,
            )
            .with_transfer_method(crate::ClusterArtifactTransferMethod::PeerCopy)
            .with_detail("copy required before execution"),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-b"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-b"),
                ClusterArtifactReference::new("decoder", "artifact-1")
                    .with_provenance_digest("prov-digest")
                    .with_governance_digest("gov-digest"),
                ClusterArtifactResidencyStatus::PullRequired,
            )
            .with_transfer_method(crate::ClusterArtifactTransferMethod::OciPull)
            .with_detail("pull required before execution"),
        );

        let state = ClusterState::from_snapshot(snapshot);
        let request = WholeRequestSchedulingRequest::new(crate::NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile())
            .with_served_artifact_digest("artifact-1")
            .with_staging_policy(false, false);

        let failure = schedule_remote_whole_request(&state, &request)
            .expect_err("disallowed artifact staging should refuse scheduling");

        assert_eq!(
            failure.code,
            WholeRequestSchedulingFailureCode::NoEligibleRemoteNode
        );
        assert!(failure.artifact_residency_digest.is_some());
        assert!(failure.refusals.iter().any(|refusal| {
            refusal.node_id.as_ref() == Some(&crate::NodeId::new("worker-a"))
                && refusal.code == WholeRequestSchedulingRefusalCode::ArtifactStagingNotAllowed
                && refusal
                    .detail
                    .contains("requires disallowed artifact staging")
        }));
        assert!(failure.refusals.iter().any(|refusal| {
            refusal.node_id.as_ref() == Some(&crate::NodeId::new("worker-b"))
                && refusal.code == WholeRequestSchedulingRefusalCode::ArtifactStagingNotAllowed
                && refusal
                    .detail
                    .contains("requires disallowed artifact staging")
        }));
        Ok(())
    }

    #[test]
    fn whole_request_scheduler_refuses_revoked_remote_candidate_even_if_membership_is_ready()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot.discovery_candidates.insert(
            crate::NodeId::new("worker-a"),
            revoked_discovery_candidate("worker-a"),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-a"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-a"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-b"), "artifact-1"),
            crate::ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-b"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot
            .links
            .get_mut(&crate::ClusterLinkKey::new(
                crate::NodeId::new("scheduler"),
                crate::NodeId::new("worker-b"),
            ))
            .ok_or_else(|| fixture_error("worker-b link"))?
            .status = ClusterLinkStatus::Disconnected;

        let state = ClusterState::from_snapshot(snapshot);
        let request = WholeRequestSchedulingRequest::new(crate::NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile())
            .with_served_artifact_digest("artifact-1");

        let failure = schedule_remote_whole_request(&state, &request)
            .expect_err("revoked and disconnected workers should refuse scheduling");

        assert_eq!(
            failure.code,
            WholeRequestSchedulingFailureCode::NoEligibleRemoteNode
        );
        assert!(failure.refusals.iter().any(|refusal| {
            refusal.node_id.as_ref() == Some(&crate::NodeId::new("worker-a"))
                && refusal.code == WholeRequestSchedulingRefusalCode::CandidateRevoked
                && refusal.detail.contains("trust_bundle_rotation")
        }));
        Ok(())
    }

    #[test]
    fn whole_request_scheduler_refuses_metal_cluster_dispatch_explicitly()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot.telemetry.insert(
            crate::NodeId::new("worker-a"),
            ready_metal_telemetry("worker-a", 32 * 1024 * 1024 * 1024),
        );
        snapshot.telemetry.insert(
            crate::NodeId::new("worker-b"),
            ready_metal_telemetry("worker-b", 48 * 1024 * 1024 * 1024),
        );
        let state = ClusterState::from_snapshot(snapshot);
        let request = WholeRequestSchedulingRequest::new(crate::NodeId::new("scheduler"), "metal")
            .with_capability_profile(metal_cluster_blocked_capability_profile());

        let failure = schedule_remote_whole_request(&state, &request)
            .expect_err("metal cluster dispatch should remain explicitly refused");

        assert_eq!(
            failure.code,
            WholeRequestSchedulingFailureCode::CommunicationClassIneligible
        );
        assert_eq!(
            failure.communication_eligibility.required_class,
            ClusterCommunicationClass::RemoteDispatch
        );
        assert!(!failure.communication_eligibility.eligible);
        assert!(
            failure
                .detail
                .contains("`#3286` -> `#3285` -> `#3269` -> `#3262`"),
            "refusal detail should point at the active Metal gate"
        );
        Ok(())
    }
}
