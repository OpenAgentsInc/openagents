use std::collections::BTreeSet;

use psionic_runtime::{
    CacheAction, ClusterCacheCapability, ClusterCacheScope, ClusterCacheUsage,
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
    ClusterId, ClusterLink, ClusterLinkClass, ClusterLinkKey, ClusterLinkStatus,
    ClusterStabilityPosture, ClusterState, ClusterTransportClass, NodeId,
    WholeRequestSchedulingFailure, WholeRequestSchedulingFailureCode,
    WholeRequestSchedulingRequest, layer_shard_handoff_communication_eligibility,
    schedule_remote_whole_request,
};

/// Policy controlling the first truthful layer-sharded cluster lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerShardedExecutionPolicy {
    /// Whether sharded handoffs require stream-capable inter-node transport.
    pub require_stream_handoff: bool,
    /// Minimum acceptable inter-node bandwidth for shard handoff, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_handoff_bandwidth_mbps: Option<u64>,
    /// Whether degraded or flaky inter-shard links remain eligible.
    pub allow_degraded_links: bool,
}

impl LayerShardedExecutionPolicy {
    /// Conservative default policy for the first homogeneous CUDA sharded lane.
    #[must_use]
    pub const fn cuda_default() -> Self {
        Self {
            require_stream_handoff: true,
            minimum_handoff_bandwidth_mbps: Some(10_000),
            allow_degraded_links: false,
        }
    }

    /// Returns a stable digest for the sharding policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"layer_sharded_policy|");
        hasher.update(if self.require_stream_handoff {
            b"stream".as_slice()
        } else {
            b"any".as_slice()
        });
        hasher.update(b"|");
        hasher.update(
            self.minimum_handoff_bandwidth_mbps
                .map_or_else(String::new, |value| value.to_string()),
        );
        hasher.update(b"|");
        hasher.update(if self.allow_degraded_links {
            b"allow_degraded".as_slice()
        } else {
            b"healthy_only".as_slice()
        });
        hex::encode(hasher.finalize())
    }
}

impl Default for LayerShardedExecutionPolicy {
    fn default() -> Self {
        Self::cuda_default()
    }
}

fn default_layer_sharded_capability_profile() -> ClusterExecutionCapabilityProfile {
    ClusterExecutionCapabilityProfile::new("cuda")
        .with_supported_lanes(vec![
            ClusterExecutionLane::RemoteWholeRequest,
            ClusterExecutionLane::LayerSharded,
        ])
        .with_clustered_cache_capability(
            ClusterCacheCapability::new(
                ClusterExecutionLane::LayerSharded,
                ClusterCacheScope::StageLocal,
                ClusterCacheScope::StageLocal,
            )
            .invalidates_on_topology_change()
            .with_detail(
                "layer-sharded prefix and KV reuse are only truthful when stage ownership stays pinned to the same shard topology",
            ),
        )
        .with_detail(
            "backend `cuda` declares whole-request dispatch plus layer-sharded cluster handoff support under explicit transport policy",
        )
}

/// Request for one homogeneous layer-sharded CUDA execution plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerShardedExecutionRequest {
    /// Node performing the sharded placement decision.
    pub scheduler_node_id: NodeId,
    /// Runtime backend requested for the lane. The first truthful path is CUDA-only.
    pub requested_backend: String,
    /// Declared capability profile for the requested backend and clustered lanes.
    pub capability_profile: ClusterExecutionCapabilityProfile,
    /// Served-artifact digest that must be executable on every selected shard node.
    pub served_artifact_digest: String,
    /// Total number of model layers in the served artifact.
    pub total_layers: usize,
    /// Number of shards to place across the cluster.
    pub shard_count: usize,
    /// Minimum free memory each shard node must expose, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_free_memory_bytes_per_shard: Option<u64>,
    /// Estimated activation bytes handed off per token at each shard boundary.
    pub activation_bytes_per_token: u64,
    /// Estimated KV-cache bytes handed off per token at each shard boundary.
    pub kv_bytes_per_token: u64,
    /// Whether peer-copy staging is allowed on shard nodes.
    pub allow_copy_staging: bool,
    /// Whether pull-based staging is allowed on shard nodes.
    pub allow_pull_staging: bool,
    /// Stable policy digests constraining the sharded decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_digests: Vec<ClusterPolicyDigest>,
    /// Optional pre-sharded manifest that must match the realized topology.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharded_model_manifest: Option<ShardedModelManifest>,
}

impl LayerShardedExecutionRequest {
    /// Creates one layer-sharded execution request for the first CUDA lane.
    #[must_use]
    pub fn new(
        scheduler_node_id: NodeId,
        served_artifact_digest: impl Into<String>,
        total_layers: usize,
        shard_count: usize,
    ) -> Self {
        Self {
            scheduler_node_id,
            requested_backend: String::from("cuda"),
            capability_profile: default_layer_sharded_capability_profile(),
            served_artifact_digest: served_artifact_digest.into(),
            total_layers,
            shard_count,
            minimum_free_memory_bytes_per_shard: None,
            activation_bytes_per_token: 0,
            kv_bytes_per_token: 0,
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

    /// Attaches a per-shard minimum free-memory requirement.
    #[must_use]
    pub const fn with_minimum_free_memory_bytes_per_shard(
        mut self,
        minimum_free_memory_bytes_per_shard: u64,
    ) -> Self {
        self.minimum_free_memory_bytes_per_shard = Some(minimum_free_memory_bytes_per_shard);
        self
    }

    /// Attaches explicit activation and KV handoff cost estimates.
    #[must_use]
    pub const fn with_handoff_bytes_per_token(
        mut self,
        activation_bytes_per_token: u64,
        kv_bytes_per_token: u64,
    ) -> Self {
        self.activation_bytes_per_token = activation_bytes_per_token;
        self.kv_bytes_per_token = kv_bytes_per_token;
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

/// Stable failure code for layer-sharded execution planning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerShardedSchedulingFailureCode {
    /// The request asked for a backend outside the first truthful CUDA scope.
    UnsupportedBackend,
    /// The backend does not satisfy the required communication class for sharded execution.
    CommunicationClassIneligible,
    /// The requested layer or shard geometry cannot be planned honestly.
    InvalidShardGeometry,
    /// The cluster lacks enough eligible remote shard nodes.
    InsufficientShardNodes,
    /// The selected shard pair lacks an authoritative handoff link fact.
    HandoffLinkMissing,
    /// The selected shard pair is connected, but the link is not honest enough for sharding.
    HandoffLinkUnsuitable,
    /// The supplied pre-sharded manifest was missing, invalid, or incompatible.
    ManifestInvalid,
    /// Whole-request candidate selection failed before shard planning could proceed.
    SchedulingFailure,
}

/// Machine-checkable failure for layer-sharded execution planning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerShardedSchedulingFailure {
    /// Stable failure code.
    pub code: LayerShardedSchedulingFailureCode,
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
    /// Requested layer count.
    pub total_layers: usize,
    /// Requested shard count.
    pub shard_count: usize,
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

/// Successful layer-sharded cluster execution plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerShardedClusterSchedule {
    /// Cluster identity used for the decision.
    pub cluster_id: ClusterId,
    /// Node that performed the sharded placement decision.
    pub scheduler_node_id: NodeId,
    /// Runtime backend selected for the sharded lane.
    pub runtime_backend: String,
    /// Total model layers partitioned across the plan.
    pub total_layers: usize,
    /// Ordered shard-node assignment for the selected plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shard_node_ids: Vec<NodeId>,
    /// Explicit layer-sharded topology emitted by the planner.
    pub execution_topology: ExecutionTopologyPlan,
    /// Explicit cross-node handoffs for the plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shard_handoffs: Vec<ClusterShardHandoff>,
    /// Cluster execution evidence for capability and receipt surfaces.
    pub cluster_execution: ClusterExecutionContext,
}

/// Plans one truthful homogeneous layer-sharded CUDA execution lane.
pub fn schedule_layer_sharded_execution(
    state: &ClusterState,
    request: &LayerShardedExecutionRequest,
    policy: &LayerShardedExecutionPolicy,
) -> Result<LayerShardedClusterSchedule, Box<LayerShardedSchedulingFailure>> {
    let cluster_state_digest = state.stable_digest();
    let topology_digest = state.topology_digest();
    let artifact_residency_digest = Some(state.artifact_residency_digest());
    let communication_eligibility =
        layer_shard_handoff_communication_eligibility(&request.capability_profile);

    if !communication_eligibility.eligible {
        return Err(Box::new(layer_sharded_failure(
            LayerShardedSchedulingFailureCode::CommunicationClassIneligible,
            communication_eligibility.detail.clone().unwrap_or_else(|| {
                format!(
                    "backend `{}` does not satisfy layer-sharded communication eligibility",
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

    let layer_ranges = match split_layers(request.total_layers, request.shard_count) {
        Some(layer_ranges) => layer_ranges,
        None => {
            return Err(Box::new(layer_sharded_failure(
                LayerShardedSchedulingFailureCode::InvalidShardGeometry,
                format!(
                    "cannot divide {} layers across {} shards",
                    request.total_layers, request.shard_count
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
        let mut last_link_failure = None;

        loop {
            let scheduling_request =
                request.whole_request_scheduling_request(attempt_excluded.clone());
            let schedule = match schedule_remote_whole_request(state, &scheduling_request) {
                Ok(schedule) => schedule,
                Err(scheduler_failure) => {
                    if let Some((code, detail)) = last_link_failure {
                        return Err(Box::new(layer_sharded_failure(
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
                            LayerShardedSchedulingFailureCode::CommunicationClassIneligible
                        }
                        WholeRequestSchedulingFailureCode::NoEligibleRemoteNode
                            if shard_index > 0 =>
                        {
                            LayerShardedSchedulingFailureCode::InsufficientShardNodes
                        }
                        WholeRequestSchedulingFailureCode::NoEligibleRemoteNode => {
                            LayerShardedSchedulingFailureCode::InsufficientShardNodes
                        }
                        WholeRequestSchedulingFailureCode::SchedulerNodeUnknown
                        | WholeRequestSchedulingFailureCode::SchedulerNodeNotReady => {
                            LayerShardedSchedulingFailureCode::SchedulingFailure
                        }
                    };
                    return Err(Box::new(layer_sharded_failure(
                        failure_code,
                        format!(
                            "unable to place shard {} of {} for `{}`",
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

            if let Some(previous_schedule) = shard_schedules.last() {
                match validate_handoff_link(
                    state,
                    &previous_schedule.selected_node_id,
                    &schedule.selected_node_id,
                    policy,
                ) {
                    Ok(_) => {}
                    Err((code, detail)) => {
                        attempt_excluded.insert(schedule.selected_node_id.clone());
                        last_link_failure = Some((code, detail));
                        continue;
                    }
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
    let execution_topology = ExecutionTopologyPlan::layer_sharded(
        request.requested_backend.clone(),
        devices
            .iter()
            .cloned()
            .zip(layer_ranges.iter().copied())
            .map(|(device, (start_layer, end_layer))| (device, start_layer, end_layer))
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
            Box::new(layer_sharded_failure(
                LayerShardedSchedulingFailureCode::ManifestInvalid,
                format!(
                    "sharded manifest `{}` is incompatible with layer-sharded request `{}`: {error}",
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
    let shard_handoffs = build_shard_handoffs(
        state,
        &selected_node_ids,
        &layer_ranges,
        request.activation_bytes_per_token,
        request.kv_bytes_per_token,
    )?;
    let mut degraded_reasons = shard_schedules
        .iter()
        .filter_map(|schedule| schedule.cluster_execution.degraded_reason.clone())
        .collect::<Vec<_>>();
    if policy.allow_degraded_links {
        degraded_reasons.extend(layer_sharded_degraded_link_details(
            state,
            &selected_node_ids,
        ));
    }
    degraded_reasons.sort();
    degraded_reasons.dedup();

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
    cluster_execution = cluster_execution.with_clustered_cache_usage(
        ClusterCacheUsage::new(
            ClusterExecutionLane::LayerSharded,
            ClusterCacheScope::StageLocal,
            ClusterCacheScope::StageLocal,
            CacheAction::Bypass,
            CacheAction::Bypass,
        )
        .with_detail(
            "layer-sharded execution cannot promise cluster-wide prefix or KV reuse outside one stable shard topology",
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
        ClusterPolicyDigestKind::Sharding,
        policy.stable_digest(),
    ));
    if !degraded_reasons.is_empty() {
        cluster_execution = cluster_execution.with_degraded_reason(degraded_reasons.join("; "));
    }

    Ok(LayerShardedClusterSchedule {
        cluster_id: state.cluster_id().clone(),
        scheduler_node_id: request.scheduler_node_id.clone(),
        runtime_backend: request.requested_backend.clone(),
        total_layers: request.total_layers,
        shard_node_ids: selected_node_ids,
        execution_topology,
        shard_handoffs,
        cluster_execution,
    })
}

fn split_layers(total_layers: usize, shard_count: usize) -> Option<Vec<(usize, usize)>> {
    if shard_count < 2 || total_layers < shard_count || total_layers == 0 {
        return None;
    }
    let base_layers = total_layers / shard_count;
    let extra_layers = total_layers % shard_count;
    let mut start_layer = 0usize;
    let mut layer_ranges = Vec::with_capacity(shard_count);
    for shard_index in 0..shard_count {
        let layer_count = base_layers + usize::from(shard_index < extra_layers);
        let end_layer = start_layer + layer_count;
        layer_ranges.push((start_layer, end_layer));
        start_layer = end_layer;
    }
    Some(layer_ranges)
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

fn validate_handoff_link<'a>(
    state: &'a ClusterState,
    left_node_id: &NodeId,
    right_node_id: &NodeId,
    policy: &LayerShardedExecutionPolicy,
) -> Result<&'a ClusterLink, (LayerShardedSchedulingFailureCode, String)> {
    let key = ClusterLinkKey::new(left_node_id.clone(), right_node_id.clone());
    let Some(link) = state.links().get(&key) else {
        return Err((
            LayerShardedSchedulingFailureCode::HandoffLinkMissing,
            format!(
                "authoritative state has no handoff link fact between shard nodes `{}` and `{}`",
                left_node_id.as_str(),
                right_node_id.as_str()
            ),
        ));
    };

    if !(matches!(link.status, ClusterLinkStatus::Healthy)
        || policy.allow_degraded_links && link.status == ClusterLinkStatus::Degraded)
    {
        return Err((
            LayerShardedSchedulingFailureCode::HandoffLinkUnsuitable,
            format!(
                "link between shard nodes `{}` and `{}` is `{}` and not honest enough for layer-sharded handoff",
                left_node_id.as_str(),
                right_node_id.as_str(),
                link_status_name(link.status)
            ),
        ));
    }
    if matches!(link.stability, ClusterStabilityPosture::Unstable)
        || (!policy.allow_degraded_links && link.stability == ClusterStabilityPosture::Flaky)
    {
        return Err((
            LayerShardedSchedulingFailureCode::HandoffLinkUnsuitable,
            format!(
                "link between shard nodes `{}` and `{}` is only `{}`",
                left_node_id.as_str(),
                right_node_id.as_str(),
                stability_name(link.stability)
            ),
        ));
    }
    if policy.require_stream_handoff
        && !matches!(
            link.transport,
            ClusterTransportClass::Tcp | ClusterTransportClass::Rdma
        )
    {
        return Err((
            LayerShardedSchedulingFailureCode::HandoffLinkUnsuitable,
            format!(
                "link between shard nodes `{}` and `{}` uses non-stream transport `{}`",
                left_node_id.as_str(),
                right_node_id.as_str(),
                transport_name(link.transport)
            ),
        ));
    }
    if policy.require_stream_handoff
        && matches!(
            link.link_class,
            ClusterLinkClass::Wifi | ClusterLinkClass::Unknown
        )
    {
        return Err((
            LayerShardedSchedulingFailureCode::HandoffLinkUnsuitable,
            format!(
                "link between shard nodes `{}` and `{}` is only classified as `{}`",
                left_node_id.as_str(),
                right_node_id.as_str(),
                link_class_name(link.link_class)
            ),
        ));
    }
    if let Some(minimum_handoff_bandwidth_mbps) = policy.minimum_handoff_bandwidth_mbps {
        if link.bandwidth_mbps.unwrap_or(0) < minimum_handoff_bandwidth_mbps {
            return Err((
                LayerShardedSchedulingFailureCode::HandoffLinkUnsuitable,
                format!(
                    "link between shard nodes `{}` and `{}` exposes {} Mbps, below required {} Mbps",
                    left_node_id.as_str(),
                    right_node_id.as_str(),
                    link.bandwidth_mbps.unwrap_or(0),
                    minimum_handoff_bandwidth_mbps
                ),
            ));
        }
    }
    Ok(link)
}

fn build_shard_handoffs(
    state: &ClusterState,
    shard_node_ids: &[NodeId],
    layer_ranges: &[(usize, usize)],
    activation_bytes_per_token: u64,
    kv_bytes_per_token: u64,
) -> Result<Vec<ClusterShardHandoff>, Box<LayerShardedSchedulingFailure>> {
    let mut shard_handoffs = Vec::new();
    for (index, ((_, end_layer), node_pair)) in layer_ranges
        .iter()
        .zip(shard_node_ids.windows(2))
        .enumerate()
    {
        let link = state
            .links()
            .get(&ClusterLinkKey::new(
                node_pair[0].clone(),
                node_pair[1].clone(),
            ))
            .ok_or_else(|| {
                Box::new(LayerShardedSchedulingFailure {
                    code: LayerShardedSchedulingFailureCode::HandoffLinkMissing,
                    detail: format!(
                        "authoritative state lost the handoff link fact between `{}` and `{}` during handoff construction",
                        node_pair[0].as_str(),
                        node_pair[1].as_str()
                    ),
                    cluster_id: state.cluster_id().clone(),
                    scheduler_node_id: NodeId::new("unknown"),
                    requested_backend: String::from("cuda"),
                    cluster_state_digest: state.stable_digest(),
                    topology_digest: state.topology_digest(),
                    artifact_residency_digest: Some(state.artifact_residency_digest()),
                    total_layers: layer_ranges.last().map_or(0, |(_, end_layer)| *end_layer),
                    shard_count: shard_node_ids.len(),
                    policy_digests: Vec::new(),
                    communication_eligibility: layer_shard_handoff_communication_eligibility(
                        &default_layer_sharded_capability_profile(),
                    ),
                    selected_node_ids: shard_node_ids.to_vec(),
                    scheduler_failure: None,
                })
            })?;
        let transport = runtime_transport_class(link.transport);
        shard_handoffs.push(
            ClusterShardHandoff::new(
                index,
                index + 1,
                node_pair[0].as_str(),
                node_pair[1].as_str(),
                ClusterShardHandoffKind::Activation,
                transport,
                *end_layer,
                activation_bytes_per_token,
            )
            .with_detail(format!(
                "forward activations across layer boundary {} from `{}` to `{}`",
                end_layer,
                node_pair[0].as_str(),
                node_pair[1].as_str()
            )),
        );
        shard_handoffs.push(
            ClusterShardHandoff::new(
                index,
                index + 1,
                node_pair[0].as_str(),
                node_pair[1].as_str(),
                ClusterShardHandoffKind::KvCache,
                transport,
                *end_layer,
                kv_bytes_per_token,
            )
            .with_detail(format!(
                "forward KV cache across layer boundary {} from `{}` to `{}`",
                end_layer,
                node_pair[0].as_str(),
                node_pair[1].as_str()
            )),
        );
    }
    Ok(shard_handoffs)
}

fn layer_sharded_degraded_link_details(
    state: &ClusterState,
    shard_node_ids: &[NodeId],
) -> Vec<String> {
    let mut details = Vec::new();
    for node_pair in shard_node_ids.windows(2) {
        let Some(link) = state.links().get(&ClusterLinkKey::new(
            node_pair[0].clone(),
            node_pair[1].clone(),
        )) else {
            continue;
        };
        if link.status == ClusterLinkStatus::Degraded {
            details.push(format!(
                "layer-sharded handoff link `{}` -> `{}` is only in degraded status",
                node_pair[0].as_str(),
                node_pair[1].as_str()
            ));
        }
        if link.stability == ClusterStabilityPosture::Flaky {
            details.push(format!(
                "layer-sharded handoff link `{}` -> `{}` is only marked flaky",
                node_pair[0].as_str(),
                node_pair[1].as_str()
            ));
        }
    }
    details
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
fn layer_sharded_failure(
    code: LayerShardedSchedulingFailureCode,
    detail: String,
    state: &ClusterState,
    request: &LayerShardedExecutionRequest,
    cluster_state_digest: &str,
    topology_digest: &str,
    artifact_residency_digest: Option<String>,
    communication_eligibility: ClusterCommunicationEligibility,
    selected_node_ids: Vec<NodeId>,
    scheduler_failure: Option<Box<WholeRequestSchedulingFailure>>,
) -> LayerShardedSchedulingFailure {
    LayerShardedSchedulingFailure {
        code,
        detail,
        cluster_id: state.cluster_id().clone(),
        scheduler_node_id: request.scheduler_node_id.clone(),
        requested_backend: request.requested_backend.clone(),
        cluster_state_digest: cluster_state_digest.to_owned(),
        topology_digest: topology_digest.to_owned(),
        artifact_residency_digest,
        total_layers: request.total_layers,
        shard_count: request.shard_count,
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
#[allow(
    clippy::bool_assert_comparison,
    clippy::expect_used,
    clippy::panic_in_result_fn
)]
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
        ClusterNodeTelemetry, ClusterSnapshot, ClusterStabilityPosture, ClusterState,
        ClusterTransportClass, NodeEpoch, NodeRole,
    };

    use super::{
        LayerShardedExecutionPolicy, LayerShardedExecutionRequest,
        LayerShardedSchedulingFailureCode, schedule_layer_sharded_execution,
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

    fn shard_link(left: &str, right: &str) -> ClusterLink {
        ClusterLink::new(
            crate::NodeId::new(left),
            crate::NodeId::new(right),
            ClusterTransportClass::Tcp,
            ClusterLinkStatus::Healthy,
        )
        .with_link_class(ClusterLinkClass::Ethernet)
        .with_latency_us(150)
        .with_bandwidth_mbps(40_000)
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
            shard_link("worker-a", "worker-b"),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("worker-b"),
                crate::NodeId::new("worker-c"),
            ),
            shard_link("worker-b", "worker-c"),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("worker-a"),
                crate::NodeId::new("worker-c"),
            ),
            shard_link("worker-a", "worker-c").with_bandwidth_mbps(20_000),
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

    fn sample_layer_sharded_manifest(
        served_artifact_digest: &str,
        second_end_layer: usize,
    ) -> ShardedModelManifest {
        ShardedModelManifest::new(
            "layer-manifest",
            sample_served_artifact_identity(served_artifact_digest),
            ShardedModelLayoutKind::LayerSharded,
        )
        .with_shard(ShardedModelArtifactRef::new(
            0,
            "decoder.layers0_20",
            "layer-digest-0",
            ExecutionPartition::LayerRange {
                start_layer: 0,
                end_layer: 20,
            },
        ))
        .with_shard(ShardedModelArtifactRef::new(
            1,
            "decoder.layers20_40",
            "layer-digest-1",
            ExecutionPartition::LayerRange {
                start_layer: 20,
                end_layer: second_end_layer,
            },
        ))
    }

    #[test]
    fn layer_sharded_scheduler_builds_two_shard_cuda_plan() -> Result<(), Box<dyn std::error::Error>>
    {
        let state = ClusterState::from_snapshot(sample_snapshot());
        let policy = LayerShardedExecutionPolicy::cuda_default();
        let request =
            LayerShardedExecutionRequest::new(crate::NodeId::new("scheduler"), "artifact-1", 40, 2)
                .with_minimum_free_memory_bytes_per_shard(16 * 1024 * 1024 * 1024)
                .with_handoff_bytes_per_token(8192, 4096)
                .with_policy_digest(ClusterPolicyDigest::new(
                    ClusterPolicyDigestKind::Placement,
                    "layer-placement-digest",
                ))
                .with_sharded_model_manifest(sample_layer_sharded_manifest("artifact-1", 40));

        let schedule =
            schedule_layer_sharded_execution(&state, &request, &policy).map_err(|err| {
                fixture_error(&format!("layer-sharded schedule should succeed: {err:?}"))
            })?;
        let expected_manifest_digest =
            sample_layer_sharded_manifest("artifact-1", 40).stable_digest();

        assert_eq!(schedule.runtime_backend, "cuda");
        assert_eq!(schedule.shard_node_ids.len(), 2);
        assert_eq!(schedule.shard_node_ids[0], crate::NodeId::new("worker-a"));
        assert_eq!(schedule.shard_node_ids[1], crate::NodeId::new("worker-b"));
        assert_eq!(
            schedule.execution_topology.kind,
            ExecutionTopologyKind::LayerSharded
        );
        assert_eq!(schedule.execution_topology.assignments.len(), 2);
        assert_eq!(
            schedule.cluster_execution.disposition,
            psionic_runtime::ClusterExecutionDisposition::Sharded
        );
        assert_eq!(schedule.shard_handoffs.len(), 2);
        assert_eq!(
            schedule.shard_handoffs[0].kind,
            psionic_runtime::ClusterShardHandoffKind::Activation
        );
        assert_eq!(schedule.shard_handoffs[0].layer_boundary, 20);
        assert_eq!(schedule.shard_handoffs[0].estimated_bytes_per_token, 8192);
        assert_eq!(
            schedule
                .cluster_execution
                .clustered_cache_usage
                .as_ref()
                .map(|usage| (usage.prefix_scope, usage.prefix_action)),
            Some((
                psionic_runtime::ClusterCacheScope::StageLocal,
                psionic_runtime::CacheAction::Bypass,
            ))
        );
        assert_eq!(
            schedule
                .cluster_execution
                .policy_digests
                .iter()
                .any(|digest| digest.kind == ClusterPolicyDigestKind::Sharding),
            true
        );
        assert_eq!(
            schedule
                .cluster_execution
                .communication_eligibility
                .as_ref()
                .map(|eligibility| eligibility.required_class),
            Some(ClusterCommunicationClass::LayerShardHandoff)
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
    fn layer_sharded_scheduler_refuses_non_cuda_backend() -> Result<(), Box<dyn std::error::Error>>
    {
        let state = ClusterState::from_snapshot(sample_snapshot());
        let request =
            LayerShardedExecutionRequest::new(crate::NodeId::new("scheduler"), "artifact-1", 40, 2)
                .with_requested_backend("metal")
                .with_capability_profile(metal_cluster_blocked_capability_profile());

        let failure = schedule_layer_sharded_execution(
            &state,
            &request,
            &LayerShardedExecutionPolicy::cuda_default(),
        )
        .expect_err("non-cuda backend should be refused");

        assert_eq!(
            failure.code,
            LayerShardedSchedulingFailureCode::CommunicationClassIneligible
        );
        assert_eq!(
            failure.communication_eligibility.required_class,
            ClusterCommunicationClass::LayerShardHandoff
        );
        Ok(())
    }

    #[test]
    fn layer_sharded_scheduler_refuses_unsuitable_handoff_link()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("worker-a"),
                crate::NodeId::new("worker-b"),
            ),
            ClusterLink::new(
                crate::NodeId::new("worker-a"),
                crate::NodeId::new("worker-b"),
                ClusterTransportClass::LanUdp,
                ClusterLinkStatus::Healthy,
            )
            .with_link_class(ClusterLinkClass::Wifi)
            .with_latency_us(2500)
            .with_bandwidth_mbps(500),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("worker-a"),
                crate::NodeId::new("worker-c"),
            ),
            ClusterLink::new(
                crate::NodeId::new("worker-a"),
                crate::NodeId::new("worker-c"),
                ClusterTransportClass::LanUdp,
                ClusterLinkStatus::Healthy,
            )
            .with_link_class(ClusterLinkClass::Wifi)
            .with_latency_us(2600)
            .with_bandwidth_mbps(400),
        );
        snapshot.links.remove(&crate::ClusterLinkKey::new(
            crate::NodeId::new("worker-b"),
            crate::NodeId::new("worker-c"),
        ));
        let state = ClusterState::from_snapshot(snapshot);
        let request =
            LayerShardedExecutionRequest::new(crate::NodeId::new("scheduler"), "artifact-1", 40, 2);

        let failure = schedule_layer_sharded_execution(
            &state,
            &request,
            &LayerShardedExecutionPolicy::cuda_default(),
        )
        .expect_err("unsuitable handoff link should be refused");

        assert_eq!(
            failure.code,
            LayerShardedSchedulingFailureCode::HandoffLinkUnsuitable
        );
        Ok(())
    }

    #[test]
    fn layer_sharded_scheduler_refuses_when_second_shard_lacks_resident_artifact()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot.artifact_residency.insert(
            ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-b"), "artifact-1"),
            ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-b"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::CopyRequired,
            ),
        );
        snapshot.artifact_residency.insert(
            ClusterArtifactResidencyKey::new(crate::NodeId::new("worker-c"), "artifact-1"),
            ClusterArtifactResidencyRecord::new(
                crate::NodeId::new("worker-c"),
                ClusterArtifactReference::new("decoder", "artifact-1"),
                ClusterArtifactResidencyStatus::PullRequired,
            ),
        );
        let state = ClusterState::from_snapshot(snapshot);
        let request =
            LayerShardedExecutionRequest::new(crate::NodeId::new("scheduler"), "artifact-1", 40, 2);

        let failure = schedule_layer_sharded_execution(
            &state,
            &request,
            &LayerShardedExecutionPolicy::cuda_default(),
        )
        .expect_err("insufficient resident shard nodes should be refused");

        assert_eq!(
            failure.code,
            LayerShardedSchedulingFailureCode::InsufficientShardNodes
        );
        assert_eq!(
            failure
                .scheduler_failure
                .as_ref()
                .map(|failure| failure.code),
            Some(crate::WholeRequestSchedulingFailureCode::NoEligibleRemoteNode)
        );
        Ok(())
    }

    #[test]
    fn layer_sharded_scheduler_refuses_manifest_partition_mismatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = ClusterState::from_snapshot(sample_snapshot());
        let request =
            LayerShardedExecutionRequest::new(crate::NodeId::new("scheduler"), "artifact-1", 40, 2)
                .with_sharded_model_manifest(sample_layer_sharded_manifest("artifact-1", 39));

        let failure = schedule_layer_sharded_execution(
            &state,
            &request,
            &LayerShardedExecutionPolicy::cuda_default(),
        )
        .expect_err("manifest partition mismatch should be refused");

        assert_eq!(
            failure.code,
            LayerShardedSchedulingFailureCode::ManifestInvalid
        );
        Ok(())
    }

    #[test]
    fn layer_sharded_scheduler_allows_bounded_degraded_handoff_with_explicit_reason()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_snapshot();
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("worker-a"),
                crate::NodeId::new("worker-b"),
            ),
            shard_link("worker-a", "worker-b")
                .with_stability_posture(ClusterStabilityPosture::Flaky)
                .with_latency_us(200)
                .with_bandwidth_mbps(35_000),
        );
        snapshot
            .links
            .get_mut(&crate::ClusterLinkKey::new(
                crate::NodeId::new("worker-a"),
                crate::NodeId::new("worker-b"),
            ))
            .ok_or_else(|| fixture_error("worker-a/worker-b shard link"))?
            .status = ClusterLinkStatus::Degraded;

        let state = ClusterState::from_snapshot(snapshot);
        let request =
            LayerShardedExecutionRequest::new(crate::NodeId::new("scheduler"), "artifact-1", 40, 2)
                .with_minimum_free_memory_bytes_per_shard(16 * 1024 * 1024 * 1024)
                .with_handoff_bytes_per_token(8192, 4096);
        let policy = LayerShardedExecutionPolicy {
            allow_degraded_links: true,
            ..LayerShardedExecutionPolicy::cuda_default()
        };

        let schedule =
            schedule_layer_sharded_execution(&state, &request, &policy).map_err(|err| {
                fixture_error(&format!(
                    "bounded degraded layer-sharded schedule should succeed: {err:?}"
                ))
            })?;

        assert_eq!(schedule.shard_node_ids.len(), 2);
        assert!(
            schedule
                .cluster_execution
                .degraded_reason
                .as_deref()
                .is_some_and(|reason| {
                    reason.contains("layer-sharded handoff link `worker-a` -> `worker-b` is only in degraded status")
                        && reason.contains("layer-sharded handoff link `worker-a` -> `worker-b` is only marked flaky")
                })
        );
        Ok(())
    }
}
