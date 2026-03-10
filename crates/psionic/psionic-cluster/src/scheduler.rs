use std::collections::BTreeSet;

use psionic_runtime::{
    ClusterArtifactResidencyDisposition as RuntimeArtifactResidencyDisposition,
    ClusterExecutionContext, ClusterExecutionDisposition, ClusterPolicyDigest,
    ClusterSelectedNode as RuntimeClusterSelectedNode,
    ClusterTransportClass as RuntimeClusterTransportClass, DeviceInventoryQualifiers,
    DeviceMemoryClass, DevicePerformanceClass, ExecutionTopologyPlan,
};
use serde::{Deserialize, Serialize};

use crate::{
    ClusterArtifactResidencyKey, ClusterArtifactResidencyStatus, ClusterBackendReadinessStatus,
    ClusterLink, ClusterLinkKey, ClusterLinkStatus, ClusterMembershipRecord,
    ClusterMembershipStatus, ClusterNodeTelemetry, ClusterStabilityPosture, ClusterState,
    ClusterTransportClass, NodeId, NodeRole,
};

/// Request for whole-request remote scheduling onto one cluster node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WholeRequestSchedulingRequest {
    /// Node performing the scheduling decision.
    pub scheduler_node_id: NodeId,
    /// Runtime backend required for the execution lane.
    pub requested_backend: String,
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
    /// Explicit nodes that this scheduling attempt must exclude.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub excluded_node_ids: BTreeSet<NodeId>,
}

impl WholeRequestSchedulingRequest {
    /// Creates a whole-request scheduling request for one scheduler node and backend.
    #[must_use]
    pub fn new(scheduler_node_id: NodeId, requested_backend: impl Into<String>) -> Self {
        Self {
            scheduler_node_id,
            requested_backend: requested_backend.into(),
            served_artifact_digest: None,
            minimum_free_memory_bytes: None,
            require_accelerator: false,
            allow_copy_staging: true,
            allow_pull_staging: true,
            policy_digests: Vec::new(),
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

    /// Appends one policy digest reference.
    #[must_use]
    pub fn with_policy_digest(mut self, policy_digest: ClusterPolicyDigest) -> Self {
        self.policy_digests.push(policy_digest);
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
            refusals: Vec::new(),
        }));
    }

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

    let selection_notes = selection_notes_for_candidate(&best);
    let degraded_reason = selection_notes
        .iter()
        .filter(|note| note.code != WholeRequestSchedulingSelectionCode::SelectedBestCandidate)
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
    .with_execution_topology(execution_topology.clone())
    .with_selected_nodes(vec![selected_node]);
    if let Some(artifact_residency_digest) = artifact_residency_digest {
        cluster_execution =
            cluster_execution.with_artifact_residency_digest(artifact_residency_digest);
    }
    for policy_digest in &request.policy_digests {
        cluster_execution = cluster_execution.with_policy_digest(policy_digest.clone());
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
) -> Vec<WholeRequestSchedulingSelectionNote> {
    let mut notes = vec![WholeRequestSchedulingSelectionNote::new(
        WholeRequestSchedulingSelectionCode::SelectedBestCandidate,
        format!(
            "selected remote node `{}` for whole-request execution",
            candidate.node_id.as_str()
        ),
    )];
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
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use std::io::Error;

    use psionic_runtime::{ClusterPolicyDigest, ClusterPolicyDigestKind, ExecutionTopologyKind};

    use crate::{
        AdmissionToken, ClusterArtifactReference, ClusterArtifactResidencyStatus,
        ClusterBackendReadinessStatus, ClusterLink, ClusterLinkStatus, ClusterMembershipRecord,
        ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry,
        ClusterSnapshot, ClusterStabilityPosture, ClusterState, ClusterTransportClass, NodeEpoch,
        NodeRole,
    };

    use super::{
        WholeRequestClusterSchedule, WholeRequestSchedulingFailureCode,
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

    #[test]
    fn whole_request_scheduler_prefers_resident_candidate_and_emits_single_device_topology()
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
                ClusterArtifactResidencyStatus::CopyRequired,
            ),
        );

        let state = ClusterState::from_snapshot(snapshot);
        let request = WholeRequestSchedulingRequest::new(crate::NodeId::new("scheduler"), "cuda")
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
                .selected_nodes
                .first()
                .and_then(|node| node.artifact_residency),
            Some(psionic_runtime::ClusterArtifactResidencyDisposition::Resident)
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
            .with_served_artifact_digest("artifact-1");

        let WholeRequestClusterSchedule {
            selected_node_id, ..
        } = schedule_remote_whole_request(&state, &request)
            .map_err(|err| fixture_error(&format!("schedule should succeed: {err:?}")))?;
        assert_eq!(selected_node_id, crate::NodeId::new("worker-a"));
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
}
