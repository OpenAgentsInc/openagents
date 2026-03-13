use std::collections::BTreeSet;

use psionic_runtime::{
    ClusterCommitAuthorityEvidence, ClusterCommunicationEligibility,
    ClusterExecutionCapabilityProfile, ClusterExecutionContext, ClusterExecutionDisposition,
    ClusterExecutionLane, ClusterPolicyDigest, ClusterPolicyDigestKind,
    ClusterSelectedNode as RuntimeClusterSelectedNode, ClusterShardHandoff,
    ClusterShardHandoffKind, ClusterTransportClass as RuntimeClusterTransportClass,
    ExecutionTopologyPlan, ShardedModelManifest, ShardedModelManifestError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ClusterId, ClusterLinkClass, ClusterLinkKey, ClusterLinkStatus, ClusterStabilityPosture,
    ClusterState, ClusterTransportClass, NodeId, WholeRequestSchedulingFailure,
    WholeRequestSchedulingFailureCode, WholeRequestSchedulingRequest,
    schedule_remote_whole_request, tensor_collective_communication_eligibility,
};

/// Explicit model-eligibility flags for the first tensor-sharded CUDA lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TensorShardedModelEligibility {
    /// Whether the served model family supports tensor sharding at all.
    pub supports_tensor_sharding: bool,
    /// Tensor axis to partition.
    pub axis: usize,
    /// Logical size of the sharded tensor axis.
    pub axis_size: usize,
    /// Minimum allowed tensor range width for each shard.
    pub minimum_partition_size: usize,
    /// Plain-language model-eligibility detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl TensorShardedModelEligibility {
    /// Creates tensor-sharding eligibility for one axis and logical size.
    #[must_use]
    pub fn new(axis: usize, axis_size: usize) -> Self {
        Self {
            supports_tensor_sharding: true,
            axis,
            axis_size,
            minimum_partition_size: 1,
            detail: None,
        }
    }

    /// Marks the model as ineligible, with a stable plain-language reason.
    #[must_use]
    pub fn refused(mut self, detail: impl Into<String>) -> Self {
        self.supports_tensor_sharding = false;
        self.detail = Some(detail.into());
        self
    }

    /// Requires a minimum tensor span per shard.
    #[must_use]
    pub const fn with_minimum_partition_size(mut self, minimum_partition_size: usize) -> Self {
        self.minimum_partition_size = minimum_partition_size;
        self
    }

    /// Attaches plain-language detail while remaining eligible.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Transport and connectivity policy for the first tensor-sharded lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TensorShardedTransportPolicy {
    /// Required inter-shard transport class.
    pub required_transport: ClusterTransportClass,
    /// Required link classification.
    pub required_link_class: ClusterLinkClass,
    /// Whether every shard must connect to every other shard.
    pub require_full_mesh: bool,
    /// Minimum acceptable inter-shard bandwidth, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_bandwidth_mbps: Option<u64>,
    /// Maximum acceptable inter-shard latency, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_latency_us: Option<u64>,
}

impl TensorShardedTransportPolicy {
    /// Conservative default policy for the first homogeneous CUDA tensor lane.
    #[must_use]
    pub const fn cuda_default() -> Self {
        Self {
            required_transport: ClusterTransportClass::Rdma,
            required_link_class: ClusterLinkClass::Rdma,
            require_full_mesh: true,
            minimum_bandwidth_mbps: Some(40_000),
            maximum_latency_us: Some(500),
        }
    }

    /// Returns a stable digest for the tensor transport policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tensor_sharded_policy|");
        hasher.update(transport_name(self.required_transport).as_bytes());
        hasher.update(b"|");
        hasher.update(link_class_name(self.required_link_class).as_bytes());
        hasher.update(b"|");
        hasher.update(if self.require_full_mesh {
            b"full_mesh".as_slice()
        } else {
            b"pairwise".as_slice()
        });
        hasher.update(b"|");
        hasher.update(
            self.minimum_bandwidth_mbps
                .map_or_else(String::new, |value| value.to_string()),
        );
        hasher.update(b"|");
        hasher.update(
            self.maximum_latency_us
                .map_or_else(String::new, |value| value.to_string()),
        );
        hex::encode(hasher.finalize())
    }
}

impl Default for TensorShardedTransportPolicy {
    fn default() -> Self {
        Self::cuda_default()
    }
}

fn default_tensor_sharded_capability_profile() -> ClusterExecutionCapabilityProfile {
    ClusterExecutionCapabilityProfile::new("cuda")
        .with_supported_lanes(vec![
            ClusterExecutionLane::RemoteWholeRequest,
            ClusterExecutionLane::TensorSharded,
        ])
        .with_detail(
            "backend `cuda` declares whole-request dispatch plus tensor-collective mesh support under explicit low-latency transport policy",
        )
}

/// Request for one homogeneous tensor-sharded CUDA execution plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TensorShardedExecutionRequest {
    /// Node performing the tensor-sharded placement decision.
    pub scheduler_node_id: NodeId,
    /// Runtime backend requested for the lane. The first truthful path is CUDA-only.
    pub requested_backend: String,
    /// Declared capability profile for the requested backend and clustered lanes.
    pub capability_profile: ClusterExecutionCapabilityProfile,
    /// Served-artifact digest that must be executable on every selected shard node.
    pub served_artifact_digest: String,
    /// Explicit model-eligibility flags for tensor sharding.
    pub model_eligibility: TensorShardedModelEligibility,
    /// Number of tensor shards to place across the cluster.
    pub shard_count: usize,
    /// Estimated bytes transferred per token for tensor collectives.
    pub collective_bytes_per_token: u64,
    /// Minimum free memory each shard node must expose, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_free_memory_bytes_per_shard: Option<u64>,
    /// Whether peer-copy staging is allowed on shard nodes.
    pub allow_copy_staging: bool,
    /// Whether pull-based staging is allowed on shard nodes.
    pub allow_pull_staging: bool,
    /// Stable policy digests constraining the tensor-sharded decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_digests: Vec<ClusterPolicyDigest>,
    /// Optional pre-sharded manifest that must match the realized topology.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharded_model_manifest: Option<ShardedModelManifest>,
}

impl TensorShardedExecutionRequest {
    /// Creates one tensor-sharded execution request for the first CUDA lane.
    #[must_use]
    pub fn new(
        scheduler_node_id: NodeId,
        served_artifact_digest: impl Into<String>,
        model_eligibility: TensorShardedModelEligibility,
        shard_count: usize,
    ) -> Self {
        Self {
            scheduler_node_id,
            requested_backend: String::from("cuda"),
            capability_profile: default_tensor_sharded_capability_profile(),
            served_artifact_digest: served_artifact_digest.into(),
            model_eligibility,
            shard_count,
            collective_bytes_per_token: 0,
            minimum_free_memory_bytes_per_shard: None,
            allow_copy_staging: false,
            allow_pull_staging: false,
            policy_digests: Vec::new(),
            sharded_model_manifest: None,
        }
    }

    /// Overrides the requested backend. Non-CUDA values are explicitly refused.
    #[must_use]
    pub fn with_requested_backend(mut self, requested_backend: impl Into<String>) -> Self {
        self.requested_backend = requested_backend.into();
        self.capability_profile =
            ClusterExecutionCapabilityProfile::new(self.requested_backend.clone());
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

    /// Attaches an explicit tensor collective byte estimate.
    #[must_use]
    pub const fn with_collective_bytes_per_token(
        mut self,
        collective_bytes_per_token: u64,
    ) -> Self {
        self.collective_bytes_per_token = collective_bytes_per_token;
        self
    }

    /// Attaches a per-shard minimum free-memory requirement.
    #[must_use]
    pub const fn with_minimum_free_memory_bytes_per_shard(
        mut self,
        minimum_free_memory_bytes_per_shard: u64,
    ) -> Self {
        self.minimum_free_memory_bytes_per_shard = Some(minimum_free_memory_bytes_per_shard);
        self
    }

    /// Overrides whether copy or pull staging is allowed.
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

    /// Attaches one pre-sharded model manifest that must match the realized topology.
    #[must_use]
    pub fn with_sharded_model_manifest(
        mut self,
        sharded_model_manifest: ShardedModelManifest,
    ) -> Self {
        self.sharded_model_manifest = Some(sharded_model_manifest);
        self
    }

    fn whole_request_scheduling_request(
        &self,
        excluded_node_ids: BTreeSet<NodeId>,
    ) -> WholeRequestSchedulingRequest {
        let mut request = WholeRequestSchedulingRequest::new(
            self.scheduler_node_id.clone(),
            self.requested_backend.clone(),
        )
        .with_capability_profile(self.capability_profile.clone())
        .with_served_artifact_digest(self.served_artifact_digest.clone())
        .requiring_accelerator()
        .with_staging_policy(self.allow_copy_staging, self.allow_pull_staging)
        .excluding_nodes(excluded_node_ids);
        if let Some(minimum_free_memory_bytes_per_shard) = self.minimum_free_memory_bytes_per_shard
        {
            request = request.with_minimum_free_memory_bytes(minimum_free_memory_bytes_per_shard);
        }
        for policy_digest in &self.policy_digests {
            request = request.with_policy_digest(policy_digest.clone());
        }
        request
    }
}

/// Stable failure code for tensor-sharded execution planning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TensorShardedSchedulingFailureCode {
    /// The request asked for a backend outside the first truthful CUDA scope.
    UnsupportedBackend,
    /// The backend does not satisfy the required communication class for tensor sharding.
    CommunicationClassIneligible,
    /// The model or artifact is ineligible for tensor sharding.
    ModelIneligible,
    /// The requested tensor geometry cannot be partitioned honestly.
    InvalidTensorGeometry,
    /// The cluster lacks enough eligible remote shard nodes.
    InsufficientShardNodes,
    /// One required pair of shard nodes lacks an authoritative mesh link fact.
    MeshLinkMissing,
    /// One required pair of shard nodes is connected, but the transport policy is not satisfied.
    MeshLinkUnsuitable,
    /// The supplied pre-sharded manifest was missing, invalid, or incompatible.
    ManifestInvalid,
    /// Whole-request candidate selection failed before tensor planning could proceed.
    SchedulingFailure,
}

/// Machine-checkable failure for tensor-sharded execution planning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TensorShardedSchedulingFailure {
    /// Stable failure code.
    pub code: TensorShardedSchedulingFailureCode,
    /// Plain-language failure detail.
    pub detail: String,
    /// Cluster identity used for the failed decision.
    pub cluster_id: ClusterId,
    /// Node that attempted the sharded decision.
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
    /// Tensor axis that was requested for sharding.
    pub tensor_axis: usize,
    /// Logical tensor-axis size.
    pub axis_size: usize,
    /// Requested shard count.
    pub shard_count: usize,
    /// Explicit model-eligibility facts used for the refusal.
    pub model_eligibility: TensorShardedModelEligibility,
    /// Policy digests constraining the failed decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_digests: Vec<ClusterPolicyDigest>,
    /// Explicit backend communication-class eligibility for the failed path.
    pub communication_eligibility: ClusterCommunicationEligibility,
    /// Nodes already selected before the failure occurred.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_node_ids: Vec<NodeId>,
    /// Nested whole-request scheduling failure, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_failure: Option<Box<WholeRequestSchedulingFailure>>,
}

/// Successful tensor-sharded cluster execution plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TensorShardedClusterSchedule {
    /// Cluster identity used for the decision.
    pub cluster_id: ClusterId,
    /// Node that performed the sharded placement decision.
    pub scheduler_node_id: NodeId,
    /// Runtime backend selected for the sharded lane.
    pub runtime_backend: String,
    /// Explicit model-eligibility facts for the realized tensor-sharded lane.
    pub model_eligibility: TensorShardedModelEligibility,
    /// Ordered shard-node assignment for the selected plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shard_node_ids: Vec<NodeId>,
    /// Explicit tensor-sharded topology emitted by the planner.
    pub execution_topology: ExecutionTopologyPlan,
    /// Explicit tensor collectives for the plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shard_handoffs: Vec<ClusterShardHandoff>,
    /// Cluster execution evidence for capability and receipt surfaces.
    pub cluster_execution: ClusterExecutionContext,
}

/// Plans one truthful homogeneous tensor-sharded CUDA execution lane.
pub fn schedule_tensor_sharded_execution(
    state: &ClusterState,
    request: &TensorShardedExecutionRequest,
    policy: &TensorShardedTransportPolicy,
) -> Result<TensorShardedClusterSchedule, Box<TensorShardedSchedulingFailure>> {
    let cluster_state_digest = state.stable_digest();
    let topology_digest = state.topology_digest();
    let artifact_residency_digest = Some(state.artifact_residency_digest());
    let communication_eligibility =
        tensor_collective_communication_eligibility(&request.capability_profile);

    if !communication_eligibility.eligible {
        return Err(Box::new(tensor_sharded_failure(
            TensorShardedSchedulingFailureCode::CommunicationClassIneligible,
            communication_eligibility.detail.clone().unwrap_or_else(|| {
                format!(
                    "backend `{}` does not satisfy tensor-sharded communication eligibility",
                    request.requested_backend
                )
            }),
            state,
            request,
            &cluster_state_digest,
            &topology_digest,
            artifact_residency_digest.clone(),
            communication_eligibility.clone(),
            Vec::new(),
            None,
        )));
    }

    if !request.model_eligibility.supports_tensor_sharding {
        return Err(Box::new(tensor_sharded_failure(
            TensorShardedSchedulingFailureCode::ModelIneligible,
            request
                .model_eligibility
                .detail
                .clone()
                .unwrap_or_else(|| String::from("model explicitly refuses tensor sharding")),
            state,
            request,
            &cluster_state_digest,
            &topology_digest,
            artifact_residency_digest.clone(),
            communication_eligibility.clone(),
            Vec::new(),
            None,
        )));
    }

    let tensor_ranges = match split_tensor_axis(
        request.model_eligibility.axis_size,
        request.shard_count,
        request.model_eligibility.minimum_partition_size,
    ) {
        Some(tensor_ranges) => tensor_ranges,
        None => {
            return Err(Box::new(tensor_sharded_failure(
                TensorShardedSchedulingFailureCode::InvalidTensorGeometry,
                format!(
                    "cannot divide tensor axis {} of size {} across {} shards with minimum span {}",
                    request.model_eligibility.axis,
                    request.model_eligibility.axis_size,
                    request.shard_count,
                    request.model_eligibility.minimum_partition_size
                ),
                state,
                request,
                &cluster_state_digest,
                &topology_digest,
                artifact_residency_digest.clone(),
                communication_eligibility.clone(),
                Vec::new(),
                None,
            )));
        }
    };

    let mut shard_schedules: Vec<crate::WholeRequestClusterSchedule> =
        Vec::with_capacity(request.shard_count);
    let mut selected_node_ids = Vec::with_capacity(request.shard_count);
    let mut globally_excluded = BTreeSet::new();

    for shard_index in 0..request.shard_count {
        let mut attempt_excluded = globally_excluded.clone();
        let mut last_mesh_failure = None;

        loop {
            let scheduling_request =
                request.whole_request_scheduling_request(attempt_excluded.clone());
            let schedule = match schedule_remote_whole_request(state, &scheduling_request) {
                Ok(schedule) => schedule,
                Err(scheduler_failure) => {
                    if let Some((code, detail)) = last_mesh_failure {
                        return Err(Box::new(tensor_sharded_failure(
                            code,
                            detail,
                            state,
                            request,
                            &cluster_state_digest,
                            &topology_digest,
                            artifact_residency_digest.clone(),
                            communication_eligibility.clone(),
                            selected_node_ids,
                            Some(scheduler_failure),
                        )));
                    }
                    let failure_code = match scheduler_failure.code {
                        WholeRequestSchedulingFailureCode::CommunicationClassIneligible => {
                            TensorShardedSchedulingFailureCode::CommunicationClassIneligible
                        }
                        WholeRequestSchedulingFailureCode::NoEligibleRemoteNode => {
                            TensorShardedSchedulingFailureCode::InsufficientShardNodes
                        }
                        WholeRequestSchedulingFailureCode::SchedulerNodeUnknown
                        | WholeRequestSchedulingFailureCode::SchedulerNodeNotReady => {
                            TensorShardedSchedulingFailureCode::SchedulingFailure
                        }
                    };
                    return Err(Box::new(tensor_sharded_failure(
                        failure_code,
                        format!(
                            "unable to place tensor shard {} of {} for `{}`",
                            shard_index + 1,
                            request.shard_count,
                            request.served_artifact_digest
                        ),
                        state,
                        request,
                        &cluster_state_digest,
                        &topology_digest,
                        artifact_residency_digest.clone(),
                        communication_eligibility.clone(),
                        selected_node_ids,
                        Some(scheduler_failure),
                    )));
                }
            };

            match validate_mesh_links(
                state,
                &selected_node_ids,
                &schedule.selected_node_id,
                policy,
            ) {
                Ok(()) => {}
                Err((code, detail)) => {
                    attempt_excluded.insert(schedule.selected_node_id.clone());
                    last_mesh_failure = Some((code, detail));
                    continue;
                }
            }

            globally_excluded.insert(schedule.selected_node_id.clone());
            selected_node_ids.push(schedule.selected_node_id.clone());
            shard_schedules.push(schedule);
            break;
        }
    }

    let devices = shard_schedules
        .iter()
        .map(|schedule| schedule.selected_device.clone())
        .collect::<Vec<_>>();
    let execution_topology = ExecutionTopologyPlan::tensor_sharded(
        request.requested_backend.clone(),
        request.model_eligibility.axis,
        devices
            .iter()
            .cloned()
            .zip(tensor_ranges.iter().copied())
            .map(|(device, (start, end))| (device, start, end))
            .collect(),
    );
    let sharded_model_manifest_digest = request
        .sharded_model_manifest
        .as_ref()
        .map(ShardedModelManifest::stable_digest);
    if let Some(manifest) = &request.sharded_model_manifest {
        validate_sharded_manifest_for_request(
            manifest,
            &request.served_artifact_digest,
            &execution_topology,
        )
        .map_err(|error| {
            Box::new(tensor_sharded_failure(
                TensorShardedSchedulingFailureCode::ManifestInvalid,
                format!(
                    "sharded manifest `{}` is incompatible with tensor-sharded request `{}`: {error}",
                    manifest.manifest_id, request.served_artifact_digest
                ),
                state,
                request,
                &cluster_state_digest,
                &topology_digest,
                artifact_residency_digest.clone(),
                communication_eligibility.clone(),
                selected_node_ids.clone(),
                None,
            ))
        })?;
    }
    let selected_nodes = shard_schedules
        .iter()
        .zip(devices.iter())
        .map(|(schedule, device)| {
            schedule
                .cluster_execution
                .selected_nodes
                .first()
                .cloned()
                .unwrap_or_else(|| {
                    RuntimeClusterSelectedNode::new(
                        schedule.selected_node_id.as_str(),
                        request.requested_backend.clone(),
                    )
                    .with_device_inventory(device.clone())
                    .with_stable_device_id(device.stable_device_id.clone())
                    .with_served_artifact_digest(request.served_artifact_digest.clone())
                })
        })
        .collect::<Vec<_>>();
    let shard_handoffs = build_tensor_collectives(
        state,
        &selected_node_ids,
        &tensor_ranges,
        request.model_eligibility.axis,
        request.collective_bytes_per_token,
    )?;

    let mut cluster_execution = ClusterExecutionContext::new(
        state.cluster_id().as_str(),
        cluster_state_digest.clone(),
        topology_digest.clone(),
        request.scheduler_node_id.as_str(),
        cluster_transport_for_sharded_path(&shard_schedules, &shard_handoffs),
        ClusterExecutionDisposition::Sharded,
    )
    .with_communication_eligibility(communication_eligibility)
    .with_execution_topology(execution_topology.clone())
    .with_selected_nodes(selected_nodes)
    .with_shard_handoffs(shard_handoffs.clone());
    if let Some(artifact_residency_digest) = artifact_residency_digest.clone() {
        cluster_execution =
            cluster_execution.with_artifact_residency_digest(artifact_residency_digest);
    }
    if let Some(sharded_model_manifest_digest) = sharded_model_manifest_digest {
        cluster_execution =
            cluster_execution.with_sharded_model_manifest_digest(sharded_model_manifest_digest);
    }
    cluster_execution = cluster_execution.with_command_provenance(merged_command_provenance(
        shard_schedules
            .iter()
            .map(|schedule| &schedule.cluster_execution),
    ));
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
        ClusterPolicyDigestKind::Sharding,
        policy.stable_digest(),
    ));

    Ok(TensorShardedClusterSchedule {
        cluster_id: state.cluster_id().clone(),
        scheduler_node_id: request.scheduler_node_id.clone(),
        runtime_backend: request.requested_backend.clone(),
        model_eligibility: request.model_eligibility.clone(),
        shard_node_ids: selected_node_ids,
        execution_topology,
        shard_handoffs,
        cluster_execution,
    })
}

fn split_tensor_axis(
    axis_size: usize,
    shard_count: usize,
    minimum_partition_size: usize,
) -> Option<Vec<(usize, usize)>> {
    if shard_count < 2 || axis_size < shard_count || axis_size == 0 {
        return None;
    }
    if !axis_size.is_multiple_of(shard_count) {
        return None;
    }
    let partition_span = axis_size / shard_count;
    if partition_span < minimum_partition_size {
        return None;
    }
    Some(
        (0..shard_count)
            .map(|index| {
                let start = index * partition_span;
                let end = start + partition_span;
                (start, end)
            })
            .collect(),
    )
}

fn merged_command_provenance<'a, I>(
    cluster_executions: I,
) -> Vec<psionic_runtime::ClusterCommandProvenanceEvidence>
where
    I: IntoIterator<Item = &'a ClusterExecutionContext>,
{
    let mut merged = Vec::new();
    for cluster_execution in cluster_executions {
        for provenance in &cluster_execution.command_provenance {
            if !merged.contains(provenance) {
                merged.push(provenance.clone());
            }
        }
    }
    merged
}

fn validate_mesh_links(
    state: &ClusterState,
    selected_node_ids: &[NodeId],
    candidate_node_id: &NodeId,
    policy: &TensorShardedTransportPolicy,
) -> Result<(), (TensorShardedSchedulingFailureCode, String)> {
    for selected_node_id in selected_node_ids {
        validate_mesh_link_pair(state, selected_node_id, candidate_node_id, policy)?;
    }
    Ok(())
}

fn validate_mesh_link_pair(
    state: &ClusterState,
    left_node_id: &NodeId,
    right_node_id: &NodeId,
    policy: &TensorShardedTransportPolicy,
) -> Result<(), (TensorShardedSchedulingFailureCode, String)> {
    let key = ClusterLinkKey::new(left_node_id.clone(), right_node_id.clone());
    let Some(link) = state.links().get(&key) else {
        return Err((
            TensorShardedSchedulingFailureCode::MeshLinkMissing,
            format!(
                "authoritative state has no mesh link fact between tensor shard nodes `{}` and `{}`",
                left_node_id.as_str(),
                right_node_id.as_str()
            ),
        ));
    };
    if !matches!(link.status, ClusterLinkStatus::Healthy) {
        return Err((
            TensorShardedSchedulingFailureCode::MeshLinkUnsuitable,
            format!(
                "mesh link between tensor shard nodes `{}` and `{}` is `{}`",
                left_node_id.as_str(),
                right_node_id.as_str(),
                link_status_name(link.status)
            ),
        ));
    }
    if !matches!(link.stability, ClusterStabilityPosture::Stable) {
        return Err((
            TensorShardedSchedulingFailureCode::MeshLinkUnsuitable,
            format!(
                "mesh link between tensor shard nodes `{}` and `{}` is only `{}`",
                left_node_id.as_str(),
                right_node_id.as_str(),
                stability_name(link.stability)
            ),
        ));
    }
    if link.transport != policy.required_transport {
        return Err((
            TensorShardedSchedulingFailureCode::MeshLinkUnsuitable,
            format!(
                "mesh link between tensor shard nodes `{}` and `{}` uses `{}` instead of required `{}`",
                left_node_id.as_str(),
                right_node_id.as_str(),
                transport_name(link.transport),
                transport_name(policy.required_transport)
            ),
        ));
    }
    if link.link_class != policy.required_link_class {
        return Err((
            TensorShardedSchedulingFailureCode::MeshLinkUnsuitable,
            format!(
                "mesh link between tensor shard nodes `{}` and `{}` is classified as `{}` instead of required `{}`",
                left_node_id.as_str(),
                right_node_id.as_str(),
                link_class_name(link.link_class),
                link_class_name(policy.required_link_class)
            ),
        ));
    }
    if let Some(minimum_bandwidth_mbps) = policy.minimum_bandwidth_mbps {
        if link.bandwidth_mbps.unwrap_or(0) < minimum_bandwidth_mbps {
            return Err((
                TensorShardedSchedulingFailureCode::MeshLinkUnsuitable,
                format!(
                    "mesh link between tensor shard nodes `{}` and `{}` exposes {} Mbps, below required {} Mbps",
                    left_node_id.as_str(),
                    right_node_id.as_str(),
                    link.bandwidth_mbps.unwrap_or(0),
                    minimum_bandwidth_mbps
                ),
            ));
        }
    }
    if let Some(maximum_latency_us) = policy.maximum_latency_us {
        if link.latency_us.unwrap_or(u64::MAX) > maximum_latency_us {
            return Err((
                TensorShardedSchedulingFailureCode::MeshLinkUnsuitable,
                format!(
                    "mesh link between tensor shard nodes `{}` and `{}` exposes {} us latency, above allowed {} us",
                    left_node_id.as_str(),
                    right_node_id.as_str(),
                    link.latency_us.unwrap_or(u64::MAX),
                    maximum_latency_us
                ),
            ));
        }
    }
    Ok(())
}

fn build_tensor_collectives(
    state: &ClusterState,
    shard_node_ids: &[NodeId],
    tensor_ranges: &[(usize, usize)],
    tensor_axis: usize,
    collective_bytes_per_token: u64,
) -> Result<Vec<ClusterShardHandoff>, Box<TensorShardedSchedulingFailure>> {
    let mut shard_handoffs = Vec::new();
    for left_index in 0..shard_node_ids.len() {
        for right_index in (left_index + 1)..shard_node_ids.len() {
            let left_node_id = &shard_node_ids[left_index];
            let right_node_id = &shard_node_ids[right_index];
            let link = state
                .links()
                .get(&ClusterLinkKey::new(left_node_id.clone(), right_node_id.clone()))
                .ok_or_else(|| {
                    Box::new(TensorShardedSchedulingFailure {
                        code: TensorShardedSchedulingFailureCode::MeshLinkMissing,
                        detail: format!(
                            "authoritative state lost the tensor mesh link fact between `{}` and `{}` during collective construction",
                            left_node_id.as_str(),
                            right_node_id.as_str()
                        ),
                        cluster_id: state.cluster_id().clone(),
                        scheduler_node_id: NodeId::new("unknown"),
                        requested_backend: String::from("cuda"),
                        cluster_state_digest: state.stable_digest(),
                        topology_digest: state.topology_digest(),
                        artifact_residency_digest: Some(state.artifact_residency_digest()),
                        tensor_axis,
                        axis_size: tensor_ranges.last().map_or(0, |(_, end)| *end),
                        shard_count: shard_node_ids.len(),
                        model_eligibility: TensorShardedModelEligibility::new(
                            tensor_axis,
                            tensor_ranges.last().map_or(0, |(_, end)| *end),
                        ),
                        policy_digests: Vec::new(),
                        communication_eligibility:
                            tensor_collective_communication_eligibility(
                                &default_tensor_sharded_capability_profile(),
                            ),
                        selected_node_ids: shard_node_ids.to_vec(),
                        scheduler_failure: None,
                    })
                })?;
            let transport = runtime_transport_class(link.transport);
            let (range_start, range_end) = tensor_ranges[left_index];
            shard_handoffs.push(
                ClusterShardHandoff::new(
                    left_index,
                    right_index,
                    left_node_id.as_str(),
                    right_node_id.as_str(),
                    ClusterShardHandoffKind::TensorCollective,
                    transport,
                    0,
                    collective_bytes_per_token,
                )
                .with_tensor_partition(tensor_axis, range_start, range_end)
                .with_detail(format!(
                    "synchronize tensor shard [{}..{}) on axis {} between `{}` and `{}`",
                    range_start,
                    range_end,
                    tensor_axis,
                    left_node_id.as_str(),
                    right_node_id.as_str()
                )),
            );
        }
    }
    Ok(shard_handoffs)
}

fn validate_sharded_manifest_for_request(
    manifest: &ShardedModelManifest,
    served_artifact_digest: &str,
    execution_topology: &ExecutionTopologyPlan,
) -> Result<(), ShardedModelManifestError> {
    if manifest.served_artifact.served_artifact_digest != served_artifact_digest {
        return Err(ShardedModelManifestError::ServedArtifactDigestMismatch {
            manifest_served_artifact_digest: manifest
                .served_artifact
                .served_artifact_digest
                .clone(),
            expected_served_artifact_digest: served_artifact_digest.to_owned(),
        });
    }
    manifest.validate_against_topology(execution_topology)
}

fn cluster_transport_for_sharded_path(
    shard_schedules: &[crate::WholeRequestClusterSchedule],
    shard_handoffs: &[ClusterShardHandoff],
) -> RuntimeClusterTransportClass {
    let mut transports = shard_schedules
        .iter()
        .map(|schedule| schedule.cluster_execution.transport)
        .chain(shard_handoffs.iter().map(|handoff| handoff.transport));
    let Some(first_transport) = transports.next() else {
        return RuntimeClusterTransportClass::Mixed;
    };
    if transports.all(|transport| transport == first_transport) {
        first_transport
    } else {
        RuntimeClusterTransportClass::Mixed
    }
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

#[allow(clippy::too_many_arguments)]
fn tensor_sharded_failure(
    code: TensorShardedSchedulingFailureCode,
    detail: String,
    state: &ClusterState,
    request: &TensorShardedExecutionRequest,
    cluster_state_digest: &str,
    topology_digest: &str,
    artifact_residency_digest: Option<String>,
    communication_eligibility: ClusterCommunicationEligibility,
    selected_node_ids: Vec<NodeId>,
    scheduler_failure: Option<Box<WholeRequestSchedulingFailure>>,
) -> TensorShardedSchedulingFailure {
    TensorShardedSchedulingFailure {
        code,
        detail,
        cluster_id: state.cluster_id().clone(),
        scheduler_node_id: request.scheduler_node_id.clone(),
        requested_backend: request.requested_backend.clone(),
        cluster_state_digest: cluster_state_digest.to_owned(),
        topology_digest: topology_digest.to_owned(),
        artifact_residency_digest,
        tensor_axis: request.model_eligibility.axis,
        axis_size: request.model_eligibility.axis_size,
        shard_count: request.shard_count,
        model_eligibility: request.model_eligibility.clone(),
        policy_digests: request.policy_digests.clone(),
        communication_eligibility,
        selected_node_ids,
        scheduler_failure,
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

const fn stability_name(stability: ClusterStabilityPosture) -> &'static str {
    match stability {
        ClusterStabilityPosture::Stable => "stable",
        ClusterStabilityPosture::Flaky => "flaky",
        ClusterStabilityPosture::Unstable => "unstable",
    }
}

const fn transport_name(transport: ClusterTransportClass) -> &'static str {
    match transport {
        ClusterTransportClass::LoopbackUdp => "loopback_udp",
        ClusterTransportClass::LanUdp => "lan_udp",
        ClusterTransportClass::Tcp => "tcp",
        ClusterTransportClass::Rdma => "rdma",
        ClusterTransportClass::Unknown => "unknown",
    }
}

const fn link_class_name(link_class: ClusterLinkClass) -> &'static str {
    match link_class {
        ClusterLinkClass::Loopback => "loopback",
        ClusterLinkClass::Ethernet => "ethernet",
        ClusterLinkClass::Wifi => "wifi",
        ClusterLinkClass::Thunderbolt => "thunderbolt",
        ClusterLinkClass::Rdma => "rdma",
        ClusterLinkClass::Unknown => "unknown",
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic_in_result_fn)]
mod tests {
    use std::io::Error;

    use psionic_runtime::{
        ClusterCommunicationClass, ClusterExecutionCapabilityProfile, ClusterPolicyDigest,
        ClusterPolicyDigestKind, ExecutionPartition, ExecutionTopologyKind, ServedArtifactIdentity,
        ShardedModelArtifactRef, ShardedModelLayoutKind, ShardedModelManifest,
    };

    use crate::{
        AdmissionToken, ClusterArtifactReference, ClusterArtifactResidencyKey,
        ClusterArtifactResidencyRecord, ClusterArtifactResidencyStatus,
        ClusterBackendReadinessStatus, ClusterLink, ClusterLinkClass, ClusterLinkStatus,
        ClusterMembershipRecord, ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity,
        ClusterNodeTelemetry, ClusterSnapshot, ClusterState, ClusterTransportClass, NodeEpoch,
        NodeRole,
    };

    use super::{
        TensorShardedExecutionRequest, TensorShardedModelEligibility,
        TensorShardedSchedulingFailureCode, TensorShardedTransportPolicy,
        schedule_tensor_sharded_execution,
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

    fn ready_cuda_telemetry(node_id: &str, free_memory_bytes: u64) -> ClusterNodeTelemetry {
        ClusterNodeTelemetry::new(crate::NodeId::new(node_id))
            .with_memory(Some(64 * 1024 * 1024 * 1024), Some(free_memory_bytes))
            .with_cpu_logical_cores(16)
            .with_accelerator_count(1)
            .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready)
    }

    fn scheduler_link(right: &str) -> ClusterLink {
        ClusterLink::new(
            crate::NodeId::new("scheduler"),
            crate::NodeId::new(right),
            ClusterTransportClass::LanUdp,
            ClusterLinkStatus::Healthy,
        )
        .with_link_class(ClusterLinkClass::Ethernet)
        .with_latency_us(500)
        .with_bandwidth_mbps(25_000)
    }

    fn mesh_link(left: &str, right: &str) -> ClusterLink {
        ClusterLink::new(
            crate::NodeId::new(left),
            crate::NodeId::new(right),
            ClusterTransportClass::Rdma,
            ClusterLinkStatus::Healthy,
        )
        .with_link_class(ClusterLinkClass::Rdma)
        .with_latency_us(120)
        .with_bandwidth_mbps(100_000)
    }

    fn metal_cluster_blocked_capability_profile() -> ClusterExecutionCapabilityProfile {
        ClusterExecutionCapabilityProfile::new("metal").with_detail(
            "backend `metal` remains refused for cluster execution until the Metal roadmap queue `#3286` -> `#3285` -> `#3269` -> `#3262` closes",
        )
    }

    fn sample_snapshot() -> ClusterSnapshot {
        let cluster_id = sample_cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships.insert(
            crate::NodeId::new("scheduler"),
            ready_membership(&cluster_id, "scheduler", NodeRole::Mixed),
        );
        for worker in ["worker-a", "worker-b", "worker-c"] {
            snapshot.memberships.insert(
                crate::NodeId::new(worker),
                ready_membership(&cluster_id, worker, NodeRole::ExecutorOnly),
            );
        }
        snapshot.telemetry.insert(
            crate::NodeId::new("worker-a"),
            ready_cuda_telemetry("worker-a", 48 * 1024 * 1024 * 1024),
        );
        snapshot.telemetry.insert(
            crate::NodeId::new("worker-b"),
            ready_cuda_telemetry("worker-b", 40 * 1024 * 1024 * 1024),
        );
        snapshot.telemetry.insert(
            crate::NodeId::new("worker-c"),
            ready_cuda_telemetry("worker-c", 32 * 1024 * 1024 * 1024),
        );
        for worker in ["worker-a", "worker-b", "worker-c"] {
            snapshot.links.insert(
                crate::ClusterLinkKey::new(
                    crate::NodeId::new("scheduler"),
                    crate::NodeId::new(worker),
                ),
                scheduler_link(worker),
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
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("worker-a"),
                crate::NodeId::new("worker-b"),
            ),
            mesh_link("worker-a", "worker-b"),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("worker-a"),
                crate::NodeId::new("worker-c"),
            ),
            mesh_link("worker-a", "worker-c"),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("worker-b"),
                crate::NodeId::new("worker-c"),
            ),
            mesh_link("worker-b", "worker-c"),
        );
        snapshot
    }

    fn sample_served_artifact_identity(served_artifact_digest: &str) -> ServedArtifactIdentity {
        serde_json::from_value(serde_json::json!({
            "model_id": "fixture-word-decoder-v0",
            "model_revision": "v0",
            "weight_bundle_digest": "bundle-digest",
            "served_artifact_digest": served_artifact_digest,
            "model_blob_digest": null,
            "tokenizer_digest": "tokenizer-digest",
            "chat_template_digest": "template-digest",
            "generation_defaults_digest": "defaults-digest",
            "weight_format": "gguf",
            "quantization_family": "ggml_q4_0",
            "backend": {
                "effective_backend": "cuda",
                "toolchain_version": "cuda@0.1.0",
                "compiled_backend_features": [],
                "probe_state": "compiled_only",
                "probed_backend_features": []
            }
        }))
        .expect("served artifact identity fixture should decode")
    }

    fn sample_tensor_sharded_manifest(served_artifact_digest: &str) -> ShardedModelManifest {
        ShardedModelManifest::new(
            "tensor-manifest",
            sample_served_artifact_identity(served_artifact_digest),
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
        ))
    }

    #[test]
    fn tensor_sharded_scheduler_builds_two_shard_cuda_plan()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = ClusterState::from_snapshot(sample_snapshot());
        let policy = TensorShardedTransportPolicy::cuda_default();
        let request = TensorShardedExecutionRequest::new(
            crate::NodeId::new("scheduler"),
            "artifact-1",
            TensorShardedModelEligibility::new(1, 64)
                .with_minimum_partition_size(16)
                .with_detail("hidden size is divisible across the requested shard count"),
            2,
        )
        .with_collective_bytes_per_token(16_384)
        .with_minimum_free_memory_bytes_per_shard(16 * 1024 * 1024 * 1024)
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Placement,
            "tensor-placement-digest",
        ))
        .with_sharded_model_manifest(sample_tensor_sharded_manifest("artifact-1"));

        let schedule =
            schedule_tensor_sharded_execution(&state, &request, &policy).map_err(|err| {
                fixture_error(&format!("tensor-sharded schedule should succeed: {err:?}"))
            })?;
        let expected_manifest_digest = sample_tensor_sharded_manifest("artifact-1").stable_digest();

        assert_eq!(schedule.runtime_backend, "cuda");
        assert_eq!(schedule.shard_node_ids.len(), 2);
        assert_eq!(schedule.shard_node_ids[0], crate::NodeId::new("worker-a"));
        assert_eq!(schedule.shard_node_ids[1], crate::NodeId::new("worker-b"));
        assert_eq!(
            schedule.execution_topology.kind,
            ExecutionTopologyKind::TensorSharded
        );
        assert_eq!(schedule.execution_topology.assignments.len(), 2);
        assert_eq!(schedule.shard_handoffs.len(), 1);
        assert_eq!(
            schedule.shard_handoffs[0].kind,
            psionic_runtime::ClusterShardHandoffKind::TensorCollective
        );
        assert_eq!(schedule.shard_handoffs[0].tensor_axis, Some(1));
        assert_eq!(schedule.shard_handoffs[0].tensor_range_start, Some(0));
        assert_eq!(schedule.shard_handoffs[0].tensor_range_end, Some(32));
        assert!(
            schedule
                .cluster_execution
                .policy_digests
                .iter()
                .any(|digest| digest.kind == ClusterPolicyDigestKind::Sharding)
        );
        assert_eq!(
            schedule
                .cluster_execution
                .communication_eligibility
                .as_ref()
                .map(|eligibility| eligibility.required_class),
            Some(ClusterCommunicationClass::TensorCollectiveMesh)
        );
        assert!(
            schedule
                .cluster_execution
                .communication_eligibility
                .as_ref()
                .and_then(|eligibility| eligibility.capability_profile_digest.as_deref())
                .is_some()
        );
        assert_eq!(
            schedule
                .cluster_execution
                .sharded_model_manifest_digest
                .as_deref(),
            Some(expected_manifest_digest.as_str())
        );
        Ok(())
    }

    #[test]
    fn tensor_sharded_scheduler_refuses_metal_backend_explicitly()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = ClusterState::from_snapshot(sample_snapshot());
        let request = TensorShardedExecutionRequest::new(
            crate::NodeId::new("scheduler"),
            "artifact-1",
            TensorShardedModelEligibility::new(1, 64).with_minimum_partition_size(16),
            2,
        )
        .with_requested_backend("metal")
        .with_capability_profile(metal_cluster_blocked_capability_profile());

        let failure = schedule_tensor_sharded_execution(
            &state,
            &request,
            &TensorShardedTransportPolicy::cuda_default(),
        )
        .expect_err("metal tensor sharding should be refused");

        assert_eq!(
            failure.code,
            TensorShardedSchedulingFailureCode::CommunicationClassIneligible
        );
        assert_eq!(
            failure.communication_eligibility.required_class,
            ClusterCommunicationClass::TensorCollectiveMesh
        );
        Ok(())
    }

    #[test]
    fn tensor_sharded_scheduler_refuses_model_ineligibility()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = ClusterState::from_snapshot(sample_snapshot());
        let request = TensorShardedExecutionRequest::new(
            crate::NodeId::new("scheduler"),
            "artifact-1",
            TensorShardedModelEligibility::new(1, 64)
                .with_minimum_partition_size(16)
                .refused("model metadata does not permit tensor sharding"),
            2,
        );

        let failure = schedule_tensor_sharded_execution(
            &state,
            &request,
            &TensorShardedTransportPolicy::cuda_default(),
        )
        .expect_err("model ineligibility should be refused");

        assert_eq!(
            failure.code,
            TensorShardedSchedulingFailureCode::ModelIneligible
        );
        Ok(())
    }

    #[test]
    fn tensor_sharded_scheduler_refuses_invalid_tensor_geometry()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = ClusterState::from_snapshot(sample_snapshot());
        let request = TensorShardedExecutionRequest::new(
            crate::NodeId::new("scheduler"),
            "artifact-1",
            TensorShardedModelEligibility::new(1, 63).with_minimum_partition_size(16),
            2,
        );

        let failure = schedule_tensor_sharded_execution(
            &state,
            &request,
            &TensorShardedTransportPolicy::cuda_default(),
        )
        .expect_err("non-divisible tensor axis should be refused");

        assert_eq!(
            failure.code,
            TensorShardedSchedulingFailureCode::InvalidTensorGeometry
        );
        Ok(())
    }

    #[test]
    fn tensor_sharded_scheduler_refuses_unsuitable_mesh_transport()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        for (left, right) in [
            ("worker-a", "worker-b"),
            ("worker-a", "worker-c"),
            ("worker-b", "worker-c"),
        ] {
            snapshot.links.insert(
                crate::ClusterLinkKey::new(crate::NodeId::new(left), crate::NodeId::new(right)),
                ClusterLink::new(
                    crate::NodeId::new(left),
                    crate::NodeId::new(right),
                    ClusterTransportClass::Tcp,
                    ClusterLinkStatus::Healthy,
                )
                .with_link_class(ClusterLinkClass::Ethernet)
                .with_latency_us(900)
                .with_bandwidth_mbps(20_000),
            );
        }
        let state = ClusterState::from_snapshot(snapshot);
        let request = TensorShardedExecutionRequest::new(
            crate::NodeId::new("scheduler"),
            "artifact-1",
            TensorShardedModelEligibility::new(1, 64).with_minimum_partition_size(16),
            2,
        );

        let failure = schedule_tensor_sharded_execution(
            &state,
            &request,
            &TensorShardedTransportPolicy::cuda_default(),
        )
        .expect_err("non-rdma mesh transport should be refused");

        assert_eq!(
            failure.code,
            TensorShardedSchedulingFailureCode::MeshLinkUnsuitable
        );
        Ok(())
    }
}
