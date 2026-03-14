use std::collections::BTreeSet;

use psionic_runtime::{
    CacheAction, ClusterCacheCapability, ClusterCacheScope, ClusterCacheUsage,
    ClusterCommitAuthorityEvidence, ClusterCommunicationEligibility,
    ClusterExecutionCapabilityProfile, ClusterExecutionContext, ClusterExecutionDisposition,
    ClusterExecutionLane, ClusterPipelineStage, ClusterPipelineStageRole, ClusterPolicyDigest,
    ClusterPolicyDigestKind, ClusterSelectedNode as RuntimeClusterSelectedNode,
    ClusterShardHandoff, ClusterShardHandoffKind,
    ClusterTransportClass as RuntimeClusterTransportClass, ExecutionTopologyPlan,
    ShardedModelManifest, ShardedModelManifestError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ClusterId, ClusterLink, ClusterLinkKey, ClusterLinkStatus, ClusterStabilityPosture,
    ClusterState, ClusterTransportClass, NodeId, WholeRequestSchedulingFailure,
    WholeRequestSchedulingFailureCode, WholeRequestSchedulingRequest,
    pipeline_stage_handoff_communication_eligibility, schedule_remote_whole_request,
};

/// Policy controlling truthful public-network pipeline-parallel execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineShardedExecutionPolicy {
    /// Whether stage links must use public-network-capable stream transport.
    pub require_public_stream_transport: bool,
    /// Minimum acceptable stage-link bandwidth, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_stage_bandwidth_mbps: Option<u64>,
    /// Maximum allowed stage-link latency before the plan becomes untruthful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_stage_handoff_latency_ms: Option<u64>,
    /// Latency above which the plan remains admissible but must degrade explicitly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded_decode_handoff_latency_ms: Option<u64>,
    /// Whether degraded or flaky stage links remain eligible.
    pub allow_degraded_links: bool,
}

impl PipelineShardedExecutionPolicy {
    /// Conservative public-network default for the first truthful pipeline lane.
    #[must_use]
    pub const fn public_network_default() -> Self {
        Self {
            require_public_stream_transport: true,
            minimum_stage_bandwidth_mbps: Some(1_000),
            maximum_stage_handoff_latency_ms: Some(120),
            degraded_decode_handoff_latency_ms: Some(40),
            allow_degraded_links: true,
        }
    }

    /// Returns a stable digest for the pipeline policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"pipeline_sharded_policy|");
        hasher.update(if self.require_public_stream_transport {
            b"public_stream".as_slice()
        } else {
            b"any_stream".as_slice()
        });
        hasher.update(b"|");
        hasher.update(
            self.minimum_stage_bandwidth_mbps
                .map_or_else(String::new, |value| value.to_string()),
        );
        hasher.update(b"|");
        hasher.update(
            self.maximum_stage_handoff_latency_ms
                .map_or_else(String::new, |value| value.to_string()),
        );
        hasher.update(b"|");
        hasher.update(
            self.degraded_decode_handoff_latency_ms
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

impl Default for PipelineShardedExecutionPolicy {
    fn default() -> Self {
        Self::public_network_default()
    }
}

fn default_pipeline_sharded_capability_profile() -> ClusterExecutionCapabilityProfile {
    ClusterExecutionCapabilityProfile::new("cuda")
        .with_supported_lanes(vec![
            ClusterExecutionLane::RemoteWholeRequest,
            ClusterExecutionLane::PipelineSharded,
        ])
        .with_clustered_cache_capability(
            ClusterCacheCapability::new(
                ClusterExecutionLane::PipelineSharded,
                ClusterCacheScope::StageLocal,
                ClusterCacheScope::StageLocal,
            )
            .invalidates_on_topology_change()
            .with_detail(
                "pipeline-parallel prefix and KV reuse are only truthful while the same ordered stage topology remains pinned",
            ),
        )
        .with_detail(
            "backend `cuda` declares whole-request dispatch plus public-network pipeline-parallel stage handoff support under explicit timing policy",
        )
}

/// Request for one truthful public-network pipeline-parallel execution plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineShardedExecutionRequest {
    /// Node performing the pipeline placement decision.
    pub scheduler_node_id: NodeId,
    /// Runtime backend requested for the lane. The first truthful path is CUDA-only.
    pub requested_backend: String,
    /// Declared capability profile for the requested backend and clustered lanes.
    pub capability_profile: ClusterExecutionCapabilityProfile,
    /// Served-artifact digest that must be executable on every selected stage node.
    pub served_artifact_digest: String,
    /// Total number of model layers in the served artifact.
    pub total_layers: usize,
    /// Number of pipeline stages to place across the cluster.
    pub stage_count: usize,
    /// Minimum free memory each stage node must expose, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_free_memory_bytes_per_stage: Option<u64>,
    /// Estimated activation bytes forwarded per token at each stage boundary.
    pub activation_bytes_per_token: u64,
    /// Estimated KV bytes forwarded per token at each stage boundary.
    pub kv_bytes_per_token: u64,
    /// Estimated startup cost before one stage becomes warm enough to serve.
    pub stage_startup_cost_ms: u64,
    /// Estimated prefill latency per layer for one stage.
    pub prefill_latency_per_layer_ms: u64,
    /// Estimated decode latency per layer for one stage.
    pub decode_latency_per_layer_ms: u64,
    /// Whether peer-copy staging is allowed on stage nodes.
    pub allow_copy_staging: bool,
    /// Whether pull-based staging is allowed on stage nodes.
    pub allow_pull_staging: bool,
    /// Stable policy digests constraining the decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_digests: Vec<ClusterPolicyDigest>,
    /// Optional pre-sharded manifest that must match the realized stage partitions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharded_model_manifest: Option<ShardedModelManifest>,
}

impl PipelineShardedExecutionRequest {
    /// Creates one pipeline-sharded execution request for the first CUDA lane.
    #[must_use]
    pub fn new(
        scheduler_node_id: NodeId,
        served_artifact_digest: impl Into<String>,
        total_layers: usize,
        stage_count: usize,
    ) -> Self {
        Self {
            scheduler_node_id,
            requested_backend: String::from("cuda"),
            capability_profile: default_pipeline_sharded_capability_profile(),
            served_artifact_digest: served_artifact_digest.into(),
            total_layers,
            stage_count,
            minimum_free_memory_bytes_per_stage: None,
            activation_bytes_per_token: 0,
            kv_bytes_per_token: 0,
            stage_startup_cost_ms: 25,
            prefill_latency_per_layer_ms: 2,
            decode_latency_per_layer_ms: 1,
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

    /// Attaches a per-stage minimum free-memory requirement.
    #[must_use]
    pub const fn with_minimum_free_memory_bytes_per_stage(
        mut self,
        minimum_free_memory_bytes_per_stage: u64,
    ) -> Self {
        self.minimum_free_memory_bytes_per_stage = Some(minimum_free_memory_bytes_per_stage);
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

    /// Overrides the stage-local startup and per-layer timing estimates.
    #[must_use]
    pub const fn with_stage_timing_costs(
        mut self,
        stage_startup_cost_ms: u64,
        prefill_latency_per_layer_ms: u64,
        decode_latency_per_layer_ms: u64,
    ) -> Self {
        self.stage_startup_cost_ms = stage_startup_cost_ms;
        self.prefill_latency_per_layer_ms = prefill_latency_per_layer_ms;
        self.decode_latency_per_layer_ms = decode_latency_per_layer_ms;
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

    /// Attaches one pre-sharded model manifest that must match the realized stage partitions.
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
        if let Some(minimum_free_memory_bytes_per_stage) = self.minimum_free_memory_bytes_per_stage
        {
            request = request.with_minimum_free_memory_bytes(minimum_free_memory_bytes_per_stage);
        }
        for policy_digest in &self.policy_digests {
            request = request.with_policy_digest(policy_digest.clone());
        }
        request
    }
}

/// Stable failure code for pipeline-parallel execution planning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineShardedSchedulingFailureCode {
    /// The request asked for a backend outside the first truthful CUDA scope.
    UnsupportedBackend,
    /// The backend does not satisfy the required communication class for pipelining.
    CommunicationClassIneligible,
    /// The requested layer or stage geometry cannot be planned honestly.
    InvalidStageGeometry,
    /// The cluster lacks enough eligible remote stage nodes.
    InsufficientStageNodes,
    /// The selected stage pair lacks an authoritative handoff link fact.
    StageLinkMissing,
    /// The selected stage pair is connected, but the link is not honest enough for pipelining.
    StageLinkUnsuitable,
    /// The selected stage pair exceeds the current timing envelope.
    TimingEnvelopeExceeded,
    /// The supplied pre-sharded manifest was invalid or incompatible.
    ManifestInvalid,
    /// Whole-request candidate selection failed before stage planning could proceed.
    SchedulingFailure,
}

/// Machine-checkable failure for pipeline-parallel execution planning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineShardedSchedulingFailure {
    /// Stable failure code.
    pub code: PipelineShardedSchedulingFailureCode,
    /// Plain-language failure detail.
    pub detail: String,
    /// Cluster identity used for the failed decision.
    pub cluster_id: ClusterId,
    /// Node that attempted the pipeline decision.
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
    /// Requested stage count.
    pub stage_count: usize,
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

/// Successful public-network pipeline-parallel cluster execution plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineShardedClusterSchedule {
    /// Cluster identity used for the decision.
    pub cluster_id: ClusterId,
    /// Node that performed the pipeline placement decision.
    pub scheduler_node_id: NodeId,
    /// Runtime backend selected for the pipeline lane.
    pub runtime_backend: String,
    /// Total model layers partitioned across the plan.
    pub total_layers: usize,
    /// Ordered stage-node assignment for the selected plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stage_node_ids: Vec<NodeId>,
    /// Explicit pipeline-sharded topology emitted by the planner.
    pub execution_topology: ExecutionTopologyPlan,
    /// Explicit pipeline stage timing and transport facts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pipeline_stages: Vec<ClusterPipelineStage>,
    /// Explicit cross-stage activation and KV handoffs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shard_handoffs: Vec<ClusterShardHandoff>,
    /// Cluster execution evidence for capability and receipt surfaces.
    pub cluster_execution: ClusterExecutionContext,
}

/// Plans one truthful public-network pipeline-parallel execution lane.
pub fn schedule_pipeline_sharded_execution(
    state: &ClusterState,
    request: &PipelineShardedExecutionRequest,
    policy: &PipelineShardedExecutionPolicy,
) -> Result<PipelineShardedClusterSchedule, Box<PipelineShardedSchedulingFailure>> {
    let cluster_state_digest = state.stable_digest();
    let topology_digest = state.topology_digest();
    let artifact_residency_digest = Some(state.artifact_residency_digest());
    let communication_eligibility =
        pipeline_stage_handoff_communication_eligibility(&request.capability_profile);

    if request.requested_backend != "cuda" {
        return Err(Box::new(pipeline_sharded_failure(
            PipelineShardedSchedulingFailureCode::UnsupportedBackend,
            format!(
                "backend `{}` is outside the first truthful public-network pipeline scope",
                request.requested_backend
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

    if !communication_eligibility.eligible {
        return Err(Box::new(pipeline_sharded_failure(
            PipelineShardedSchedulingFailureCode::CommunicationClassIneligible,
            communication_eligibility.detail.clone().unwrap_or_else(|| {
                format!(
                    "backend `{}` does not satisfy pipeline-parallel communication eligibility",
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

    let stage_ranges = match split_layers(request.total_layers, request.stage_count) {
        Some(stage_ranges) => stage_ranges,
        None => {
            return Err(Box::new(pipeline_sharded_failure(
                PipelineShardedSchedulingFailureCode::InvalidStageGeometry,
                format!(
                    "cannot divide {} layers across {} ordered stages",
                    request.total_layers, request.stage_count
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

    let mut stage_schedules: Vec<crate::WholeRequestClusterSchedule> =
        Vec::with_capacity(request.stage_count);
    let mut selected_node_ids = Vec::with_capacity(request.stage_count);
    let mut globally_excluded = BTreeSet::new();

    for stage_index in 0..request.stage_count {
        let mut attempt_excluded = globally_excluded.clone();
        let mut last_stage_link_failure = None;

        loop {
            let scheduling_request =
                request.whole_request_scheduling_request(attempt_excluded.clone());
            let schedule = match schedule_remote_whole_request(state, &scheduling_request) {
                Ok(schedule) => schedule,
                Err(scheduler_failure) => {
                    if let Some((code, detail)) = last_stage_link_failure {
                        return Err(Box::new(pipeline_sharded_failure(
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
                            PipelineShardedSchedulingFailureCode::CommunicationClassIneligible
                        }
                        WholeRequestSchedulingFailureCode::NoEligibleRemoteNode => {
                            PipelineShardedSchedulingFailureCode::InsufficientStageNodes
                        }
                        WholeRequestSchedulingFailureCode::SchedulerNodeUnknown
                        | WholeRequestSchedulingFailureCode::SchedulerNodeNotReady => {
                            PipelineShardedSchedulingFailureCode::SchedulingFailure
                        }
                    };
                    return Err(Box::new(pipeline_sharded_failure(
                        failure_code,
                        format!(
                            "unable to place pipeline stage {} of {} for `{}`",
                            stage_index + 1,
                            request.stage_count,
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

            if let Some(previous_schedule) = stage_schedules.last() {
                match validate_stage_link(
                    state,
                    &previous_schedule.selected_node_id,
                    &schedule.selected_node_id,
                    policy,
                ) {
                    Ok(_) => {}
                    Err((code, detail)) => {
                        attempt_excluded.insert(schedule.selected_node_id.clone());
                        last_stage_link_failure = Some((code, detail));
                        continue;
                    }
                }
            }

            globally_excluded.insert(schedule.selected_node_id.clone());
            selected_node_ids.push(schedule.selected_node_id.clone());
            stage_schedules.push(schedule);
            break;
        }
    }

    let devices = stage_schedules
        .iter()
        .map(|schedule| schedule.selected_device.clone())
        .collect::<Vec<_>>();
    let execution_topology = ExecutionTopologyPlan::pipeline_sharded(
        request.requested_backend.clone(),
        devices
            .iter()
            .cloned()
            .zip(stage_ranges.iter().copied())
            .map(|(device, (start_layer, end_layer))| (device, start_layer, end_layer))
            .collect(),
    );
    let manifest_topology = ExecutionTopologyPlan::layer_sharded(
        request.requested_backend.clone(),
        devices
            .iter()
            .cloned()
            .zip(stage_ranges.iter().copied())
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
            &manifest_topology,
        )
        .map_err(|error| {
            Box::new(pipeline_sharded_failure(
                PipelineShardedSchedulingFailureCode::ManifestInvalid,
                format!(
                    "sharded manifest `{}` is incompatible with pipeline request `{}`: {error}",
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

    let selected_nodes = stage_schedules
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
    let shard_handoffs = build_pipeline_handoffs(
        state,
        &selected_node_ids,
        &stage_ranges,
        request.activation_bytes_per_token,
        request.kv_bytes_per_token,
    )?;
    let pipeline_stages = build_pipeline_stages(state, &selected_node_ids, &stage_ranges, request)?;
    let degraded_reasons = pipeline_degraded_reasons(state, &selected_node_ids, policy);

    let mut cluster_execution = ClusterExecutionContext::new(
        state.cluster_id().as_str(),
        cluster_state_digest.clone(),
        topology_digest.clone(),
        request.scheduler_node_id.as_str(),
        cluster_transport_for_pipeline_path(&stage_schedules, &shard_handoffs),
        ClusterExecutionDisposition::Sharded,
    )
    .with_communication_eligibility(communication_eligibility)
    .with_execution_topology(execution_topology.clone())
    .with_selected_nodes(selected_nodes)
    .with_shard_handoffs(shard_handoffs.clone())
    .with_pipeline_stages(pipeline_stages.clone());
    if let Some(artifact_residency_digest) = artifact_residency_digest.clone() {
        cluster_execution =
            cluster_execution.with_artifact_residency_digest(artifact_residency_digest);
    }
    if let Some(sharded_model_manifest_digest) = sharded_model_manifest_digest {
        cluster_execution =
            cluster_execution.with_sharded_model_manifest_digest(sharded_model_manifest_digest);
    }
    cluster_execution = cluster_execution.with_command_provenance(merged_command_provenance(
        stage_schedules
            .iter()
            .map(|schedule| &schedule.cluster_execution),
    ));
    cluster_execution = cluster_execution.with_clustered_cache_usage(
        ClusterCacheUsage::new(
            ClusterExecutionLane::PipelineSharded,
            ClusterCacheScope::StageLocal,
            ClusterCacheScope::StageLocal,
            CacheAction::Bypass,
            CacheAction::Bypass,
        )
        .with_detail(
            "pipeline-parallel execution cannot promise cluster-wide prefix or KV reuse outside one fixed stage topology",
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

    Ok(PipelineShardedClusterSchedule {
        cluster_id: state.cluster_id().clone(),
        scheduler_node_id: request.scheduler_node_id.clone(),
        runtime_backend: request.requested_backend.clone(),
        total_layers: request.total_layers,
        stage_node_ids: selected_node_ids,
        execution_topology,
        pipeline_stages,
        shard_handoffs,
        cluster_execution,
    })
}

fn split_layers(total_layers: usize, stage_count: usize) -> Option<Vec<(usize, usize)>> {
    if stage_count < 2 || total_layers < stage_count || total_layers == 0 {
        return None;
    }
    let base_layers = total_layers / stage_count;
    let extra_layers = total_layers % stage_count;
    let mut start_layer = 0usize;
    let mut stage_ranges = Vec::with_capacity(stage_count);
    for stage_index in 0..stage_count {
        let layer_count = base_layers + usize::from(stage_index < extra_layers);
        let end_layer = start_layer + layer_count;
        stage_ranges.push((start_layer, end_layer));
        start_layer = end_layer;
    }
    Some(stage_ranges)
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

fn validate_stage_link<'a>(
    state: &'a ClusterState,
    left_node_id: &NodeId,
    right_node_id: &NodeId,
    policy: &PipelineShardedExecutionPolicy,
) -> Result<&'a ClusterLink, (PipelineShardedSchedulingFailureCode, String)> {
    let key = ClusterLinkKey::new(left_node_id.clone(), right_node_id.clone());
    let Some(link) = state.links().get(&key) else {
        return Err((
            PipelineShardedSchedulingFailureCode::StageLinkMissing,
            format!(
                "authoritative state has no pipeline stage link fact between `{}` and `{}`",
                left_node_id.as_str(),
                right_node_id.as_str()
            ),
        ));
    };

    if !(matches!(link.status, ClusterLinkStatus::Healthy)
        || policy.allow_degraded_links && link.status == ClusterLinkStatus::Degraded)
    {
        return Err((
            PipelineShardedSchedulingFailureCode::StageLinkUnsuitable,
            format!(
                "pipeline stage link `{}` -> `{}` is `{}` and not honest enough for pipelining",
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
            PipelineShardedSchedulingFailureCode::StageLinkUnsuitable,
            format!(
                "pipeline stage link `{}` -> `{}` is only `{}`",
                left_node_id.as_str(),
                right_node_id.as_str(),
                stability_name(link.stability)
            ),
        ));
    }
    if policy.require_public_stream_transport && link.transport != ClusterTransportClass::Tcp {
        return Err((
            PipelineShardedSchedulingFailureCode::StageLinkUnsuitable,
            format!(
                "pipeline stage link `{}` -> `{}` uses non-public stream transport `{}`",
                left_node_id.as_str(),
                right_node_id.as_str(),
                transport_name(link.transport)
            ),
        ));
    }
    if let Some(minimum_stage_bandwidth_mbps) = policy.minimum_stage_bandwidth_mbps {
        if link.bandwidth_mbps.unwrap_or(0) < minimum_stage_bandwidth_mbps {
            return Err((
                PipelineShardedSchedulingFailureCode::StageLinkUnsuitable,
                format!(
                    "pipeline stage link `{}` -> `{}` exposes {} Mbps, below required {} Mbps",
                    left_node_id.as_str(),
                    right_node_id.as_str(),
                    link.bandwidth_mbps.unwrap_or(0),
                    minimum_stage_bandwidth_mbps
                ),
            ));
        }
    }
    if let Some(maximum_stage_handoff_latency_ms) = policy.maximum_stage_handoff_latency_ms {
        let handoff_latency_ms = latency_ms(link);
        if handoff_latency_ms > maximum_stage_handoff_latency_ms {
            return Err((
                PipelineShardedSchedulingFailureCode::TimingEnvelopeExceeded,
                format!(
                    "pipeline stage link `{}` -> `{}` exposes {} ms latency, above allowed {} ms",
                    left_node_id.as_str(),
                    right_node_id.as_str(),
                    handoff_latency_ms,
                    maximum_stage_handoff_latency_ms
                ),
            ));
        }
    }
    Ok(link)
}

fn build_pipeline_handoffs(
    state: &ClusterState,
    stage_node_ids: &[NodeId],
    stage_ranges: &[(usize, usize)],
    activation_bytes_per_token: u64,
    kv_bytes_per_token: u64,
) -> Result<Vec<ClusterShardHandoff>, Box<PipelineShardedSchedulingFailure>> {
    let mut shard_handoffs = Vec::new();
    for (index, ((_, end_layer), node_pair)) in stage_ranges
        .iter()
        .zip(stage_node_ids.windows(2))
        .enumerate()
    {
        let link = stage_link(state, &node_pair[0], &node_pair[1])?;
        let transport = runtime_transport_class(link.transport);
        let handoff_latency_ms = latency_ms(link);
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
                "pipeline activation handoff across layer boundary {} from `{}` to `{}` over {} ms public-network transport",
                end_layer,
                node_pair[0].as_str(),
                node_pair[1].as_str(),
                handoff_latency_ms
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
                "pipeline KV handoff across layer boundary {} from `{}` to `{}` over {} ms public-network transport",
                end_layer,
                node_pair[0].as_str(),
                node_pair[1].as_str(),
                handoff_latency_ms
            )),
        );
    }
    Ok(shard_handoffs)
}

fn build_pipeline_stages(
    state: &ClusterState,
    stage_node_ids: &[NodeId],
    stage_ranges: &[(usize, usize)],
    request: &PipelineShardedExecutionRequest,
) -> Result<Vec<ClusterPipelineStage>, Box<PipelineShardedSchedulingFailure>> {
    let mut pipeline_stages = Vec::with_capacity(stage_node_ids.len());
    for (stage_index, (node_id, (start_layer, end_layer))) in stage_node_ids
        .iter()
        .zip(stage_ranges.iter().copied())
        .enumerate()
    {
        let layer_count = end_layer.saturating_sub(start_layer) as u64;
        let role = if stage_index == 0 {
            ClusterPipelineStageRole::Entry
        } else if stage_index + 1 == stage_node_ids.len() {
            ClusterPipelineStageRole::Exit
        } else {
            ClusterPipelineStageRole::Middle
        };
        let mut stage = ClusterPipelineStage::new(
            stage_index,
            node_id.as_str(),
            role,
            start_layer,
            end_layer,
            request.stage_startup_cost_ms + stage_index as u64 * 5,
            layer_count.saturating_mul(request.prefill_latency_per_layer_ms),
            layer_count.saturating_mul(request.decode_latency_per_layer_ms),
        )
        .with_detail(format!(
            "pipeline stage {} owns layers [{}..{}) on node `{}`",
            stage_index,
            start_layer,
            end_layer,
            node_id.as_str()
        ));
        if let Some(next_node_id) = stage_node_ids.get(stage_index + 1) {
            let link = stage_link(state, node_id, next_node_id)?;
            stage = stage.with_handoff(
                runtime_transport_class(link.transport),
                Some(latency_ms(link)),
                link.bandwidth_mbps,
            );
        }
        pipeline_stages.push(stage);
    }
    Ok(pipeline_stages)
}

fn pipeline_degraded_reasons(
    state: &ClusterState,
    stage_node_ids: &[NodeId],
    policy: &PipelineShardedExecutionPolicy,
) -> Vec<String> {
    let mut details = Vec::new();
    for node_pair in stage_node_ids.windows(2) {
        let Some(link) = state.links().get(&ClusterLinkKey::new(
            node_pair[0].clone(),
            node_pair[1].clone(),
        )) else {
            continue;
        };
        if link.status == ClusterLinkStatus::Degraded {
            details.push(format!(
                "pipeline stage link `{}` -> `{}` is only in degraded status",
                node_pair[0].as_str(),
                node_pair[1].as_str()
            ));
        }
        if link.stability == ClusterStabilityPosture::Flaky {
            details.push(format!(
                "pipeline stage link `{}` -> `{}` is only marked flaky",
                node_pair[0].as_str(),
                node_pair[1].as_str()
            ));
        }
        if policy
            .degraded_decode_handoff_latency_ms
            .is_some_and(|threshold| latency_ms(link) > threshold)
        {
            details.push(format!(
                "pipeline stage link `{}` -> `{}` adds {} ms handoff latency to decode",
                node_pair[0].as_str(),
                node_pair[1].as_str(),
                latency_ms(link)
            ));
        }
    }
    details.sort();
    details.dedup();
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

fn stage_link<'a>(
    state: &'a ClusterState,
    left_node_id: &NodeId,
    right_node_id: &NodeId,
) -> Result<&'a ClusterLink, Box<PipelineShardedSchedulingFailure>> {
    state
        .links()
        .get(&ClusterLinkKey::new(
            left_node_id.clone(),
            right_node_id.clone(),
        ))
        .ok_or_else(|| {
            Box::new(PipelineShardedSchedulingFailure {
                code: PipelineShardedSchedulingFailureCode::StageLinkMissing,
                detail: format!(
                    "authoritative state lost the pipeline stage link fact between `{}` and `{}` during plan construction",
                    left_node_id.as_str(),
                    right_node_id.as_str()
                ),
                cluster_id: state.cluster_id().clone(),
                scheduler_node_id: NodeId::new("unknown"),
                requested_backend: String::from("cuda"),
                cluster_state_digest: state.stable_digest(),
                topology_digest: state.topology_digest(),
                artifact_residency_digest: Some(state.artifact_residency_digest()),
                total_layers: 0,
                stage_count: 0,
                policy_digests: Vec::new(),
                communication_eligibility: pipeline_stage_handoff_communication_eligibility(
                    &default_pipeline_sharded_capability_profile(),
                ),
                selected_node_ids: vec![left_node_id.clone(), right_node_id.clone()],
                scheduler_failure: None,
            })
        })
}

fn cluster_transport_for_pipeline_path(
    stage_schedules: &[crate::WholeRequestClusterSchedule],
    shard_handoffs: &[ClusterShardHandoff],
) -> RuntimeClusterTransportClass {
    let mut transports = stage_schedules
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
        ClusterTransportClass::Tcp => RuntimeClusterTransportClass::WiderNetworkStream,
        ClusterTransportClass::Rdma => RuntimeClusterTransportClass::TrustedLanStream,
        ClusterTransportClass::Unknown => RuntimeClusterTransportClass::Mixed,
    }
}

fn latency_ms(link: &ClusterLink) -> u64 {
    link.latency_us
        .map(|latency_us| latency_us.div_ceil(1_000))
        .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
fn pipeline_sharded_failure(
    code: PipelineShardedSchedulingFailureCode,
    detail: String,
    state: &ClusterState,
    request: &PipelineShardedExecutionRequest,
    cluster_state_digest: &str,
    topology_digest: &str,
    artifact_residency_digest: Option<String>,
    communication_eligibility: ClusterCommunicationEligibility,
    selected_node_ids: Vec<NodeId>,
    scheduler_failure: Option<Box<WholeRequestSchedulingFailure>>,
) -> PipelineShardedSchedulingFailure {
    PipelineShardedSchedulingFailure {
        code,
        detail,
        cluster_id: state.cluster_id().clone(),
        scheduler_node_id: request.scheduler_node_id.clone(),
        requested_backend: request.requested_backend.clone(),
        cluster_state_digest: cluster_state_digest.to_owned(),
        topology_digest: topology_digest.to_owned(),
        artifact_residency_digest,
        total_layers: request.total_layers,
        stage_count: request.stage_count,
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

#[cfg(test)]
#[allow(
    clippy::bool_assert_comparison,
    clippy::expect_used,
    clippy::panic_in_result_fn
)]
mod tests {
    use std::io::Error;

    use psionic_runtime::{
        ClusterCommunicationClass, ClusterPipelineStageRole, ExecutionPartition,
        ExecutionTopologyKind, ShardedModelArtifactRef, ShardedModelLayoutKind,
        ShardedModelManifest,
    };

    use crate::{
        AdmissionToken, ClusterArtifactReference, ClusterArtifactResidencyKey,
        ClusterArtifactResidencyRecord, ClusterArtifactResidencyStatus,
        ClusterBackendReadinessStatus, ClusterLink, ClusterLinkStatus, ClusterMembershipRecord,
        ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry,
        ClusterSnapshot, ClusterState, ClusterTransportClass, NodeEpoch, NodeRole,
    };

    use super::{
        PipelineShardedExecutionPolicy, PipelineShardedExecutionRequest,
        PipelineShardedSchedulingFailureCode, schedule_pipeline_sharded_execution,
    };

    fn fixture_error(detail: &str) -> Error {
        Error::other(detail.to_owned())
    }

    fn sample_cluster_id() -> crate::ClusterId {
        crate::ClusterId::new(
            &ClusterNamespace::new("cluster-public"),
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
            ClusterTransportClass::Tcp,
            ClusterLinkStatus::Healthy,
        )
        .with_latency_us(18_000)
        .with_bandwidth_mbps(2_500)
    }

    fn public_stage_link(left: &str, right: &str) -> ClusterLink {
        ClusterLink::new(
            crate::NodeId::new(left),
            crate::NodeId::new(right),
            ClusterTransportClass::Tcp,
            ClusterLinkStatus::Healthy,
        )
        .with_latency_us(32_000)
        .with_bandwidth_mbps(3_000)
    }

    fn sample_state() -> ClusterState {
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
            snapshot.telemetry.insert(
                crate::NodeId::new(worker),
                ready_cuda_telemetry(worker, 48 * 1024 * 1024 * 1024),
            );
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
        for (left, right) in [
            ("worker-a", "worker-b"),
            ("worker-a", "worker-c"),
            ("worker-b", "worker-c"),
        ] {
            snapshot.links.insert(
                crate::ClusterLinkKey::new(crate::NodeId::new(left), crate::NodeId::new(right)),
                public_stage_link(left, right),
            );
        }
        ClusterState::from_snapshot(snapshot)
    }

    fn sample_manifest(served_artifact_digest: &str) -> ShardedModelManifest {
        let served_artifact = serde_json::from_value(serde_json::json!({
            "model_id": "fixture-decoder",
            "model_revision": "v0",
            "weight_bundle_digest": "bundle-digest",
            "served_artifact_digest": served_artifact_digest,
            "model_blob_digest": "model-blob-digest",
            "tokenizer_digest": "tokenizer-digest",
            "chat_template_digest": "template-digest",
            "generation_defaults_digest": "defaults-digest",
            "weight_format": "gguf",
            "quantization_family": "ggml_q4_0",
            "backend": {
                "effective_backend": "cuda",
                "toolchain_version": "cuda@0.1.0",
                "compiled_backend_features": []
                ,"probe_state": "compiled_only",
                "probed_backend_features": []
            }
        }))
        .expect("served artifact fixture should decode");
        ShardedModelManifest::new(
            "pipeline-manifest",
            served_artifact,
            ShardedModelLayoutKind::LayerSharded,
        )
        .with_shard(ShardedModelArtifactRef::new(
            0,
            "pipeline-shard-0",
            "pipeline-shard-digest-0",
            ExecutionPartition::LayerRange {
                start_layer: 0,
                end_layer: 20,
            },
        ))
        .with_shard(ShardedModelArtifactRef::new(
            1,
            "pipeline-shard-1",
            "pipeline-shard-digest-1",
            ExecutionPartition::LayerRange {
                start_layer: 20,
                end_layer: 40,
            },
        ))
        .with_shard(ShardedModelArtifactRef::new(
            2,
            "pipeline-shard-2",
            "pipeline-shard-digest-2",
            ExecutionPartition::LayerRange {
                start_layer: 40,
                end_layer: 60,
            },
        ))
    }

    #[test]
    fn pipeline_sharded_scheduler_builds_public_network_plan()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = sample_state();
        let request = PipelineShardedExecutionRequest::new(
            crate::NodeId::new("scheduler"),
            "artifact-1",
            60,
            3,
        )
        .with_minimum_free_memory_bytes_per_stage(16 * 1024 * 1024 * 1024)
        .with_handoff_bytes_per_token(8_192, 4_096)
        .with_stage_timing_costs(30, 3, 1)
        .with_sharded_model_manifest(sample_manifest("artifact-1"));
        let policy = PipelineShardedExecutionPolicy::public_network_default();

        let schedule =
            schedule_pipeline_sharded_execution(&state, &request, &policy).map_err(|error| {
                fixture_error(&format!("expected public-network pipeline plan: {error:?}"))
            })?;

        assert_eq!(schedule.runtime_backend, "cuda");
        assert_eq!(schedule.stage_node_ids.len(), 3);
        assert_eq!(
            schedule.execution_topology.kind,
            ExecutionTopologyKind::PipelineSharded
        );
        assert_eq!(
            schedule.cluster_execution.transport,
            psionic_runtime::ClusterTransportClass::WiderNetworkStream
        );
        assert_eq!(schedule.pipeline_stages.len(), 3);
        assert_eq!(
            schedule.pipeline_stages[0].role,
            ClusterPipelineStageRole::Entry
        );
        assert_eq!(
            schedule.pipeline_stages[2].role,
            ClusterPipelineStageRole::Exit
        );
        assert_eq!(
            schedule.pipeline_stages[0].handoff_transport,
            Some(psionic_runtime::ClusterTransportClass::WiderNetworkStream)
        );
        assert_eq!(
            schedule.cluster_execution.pipeline_stages,
            schedule.pipeline_stages
        );
        assert_eq!(schedule.cluster_execution.shard_handoffs.len(), 4);
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
                .communication_eligibility
                .as_ref()
                .map(|eligibility| eligibility.required_class),
            Some(ClusterCommunicationClass::PipelineStageHandoff)
        );
        let expected_manifest_digest = sample_manifest("artifact-1").stable_digest();
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
    fn pipeline_sharded_scheduler_refuses_when_stage_latency_exceeds_envelope()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_state().snapshot().clone();
        for (left, right) in [
            ("worker-a", "worker-b"),
            ("worker-a", "worker-c"),
            ("worker-b", "worker-c"),
        ] {
            snapshot.links.insert(
                crate::ClusterLinkKey::new(crate::NodeId::new(left), crate::NodeId::new(right)),
                public_stage_link(left, right).with_latency_us(180_000),
            );
        }
        let state = ClusterState::from_snapshot(snapshot);
        let request = PipelineShardedExecutionRequest::new(
            crate::NodeId::new("scheduler"),
            "artifact-1",
            60,
            3,
        );
        let policy = PipelineShardedExecutionPolicy::public_network_default();

        let failure = schedule_pipeline_sharded_execution(&state, &request, &policy)
            .expect_err("expected stage latency envelope refusal");

        assert_eq!(
            failure.code,
            PipelineShardedSchedulingFailureCode::TimingEnvelopeExceeded
        );
        assert!(failure.detail.contains("above allowed"));
        Ok(())
    }

    #[test]
    fn pipeline_sharded_scheduler_records_degraded_decode_handoff_reason()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = sample_state().snapshot().clone();
        snapshot.links.insert(
            crate::ClusterLinkKey::new(
                crate::NodeId::new("worker-a"),
                crate::NodeId::new("worker-b"),
            ),
            public_stage_link("worker-a", "worker-b").with_latency_us(55_000),
        );
        let state = ClusterState::from_snapshot(snapshot);
        let request = PipelineShardedExecutionRequest::new(
            crate::NodeId::new("scheduler"),
            "artifact-1",
            60,
            3,
        );
        let policy = PipelineShardedExecutionPolicy::public_network_default();

        let schedule =
            schedule_pipeline_sharded_execution(&state, &request, &policy).map_err(|error| {
                fixture_error(&format!(
                    "expected degraded public-network pipeline plan: {error:?}"
                ))
            })?;

        assert!(
            schedule
                .cluster_execution
                .degraded_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("adds 55 ms handoff latency"))
        );
        assert_eq!(schedule.pipeline_stages[0].handoff_latency_ms, Some(55));
        Ok(())
    }
}
