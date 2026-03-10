use psionic_cluster::{
    AdmissionToken, ClusterArtifactReference, ClusterArtifactResidencyKey,
    ClusterArtifactResidencyRecord, ClusterArtifactResidencyStatus, ClusterBackendReadinessStatus,
    ClusterCatchupRequest, ClusterCommandAuthorityScope, ClusterCommandAuthorization, ClusterEvent,
    ClusterEventIndex, ClusterEventLog, ClusterId, ClusterLeadershipLeasePolicy,
    ClusterLeadershipRecord, ClusterLeaseTick, ClusterLink, ClusterLinkClass, ClusterLinkKey,
    ClusterLinkStatus, ClusterMembershipRecord, ClusterMembershipStatus, ClusterNamespace,
    ClusterNodeIdentity, ClusterNodeServiceHealth, ClusterNodeServiceLoad, ClusterNodeTelemetry,
    ClusterRecoveryPolicy, ClusterReplicaLaneKey, ClusterReplicaRecord, ClusterReplicaSnapshot,
    ClusterServingLoadSnapshot, ClusterServingRequest, ClusterServingWorkClass, ClusterSnapshot,
    ClusterStabilityPosture, ClusterState, ClusterTerm, ClusterTransportClass,
    LayerShardedExecutionPolicy, LayerShardedExecutionRequest, NodeEpoch, NodeId, NodeRole,
    TensorShardedExecutionRequest, TensorShardedModelEligibility, TensorShardedTransportPolicy,
    WholeRequestSchedulingRequest,
};
use psionic_runtime::{
    ClusterExecutionCapabilityProfile, ClusterExecutionLane, ClusterReplicaWarmState,
};

pub const ARTIFACT_DIGEST: &str = "artifact-1";
pub const CLUSTER_NAMESPACE: &str = "cluster-lan";
pub const CLUSTER_SECRET: &str = "cluster-secret";
pub const REPLICA_PRODUCT_ID: &str = "fixture.decoder";
pub const REPLICA_MODEL_ID: &str = "fixture-decoder";
pub const REPLICA_RUNTIME_BACKEND: &str = "cuda";

#[must_use]
pub fn cuda_remote_dispatch_capability_profile() -> ClusterExecutionCapabilityProfile {
    ClusterExecutionCapabilityProfile::new("cuda")
        .with_supported_lanes(vec![ClusterExecutionLane::RemoteWholeRequest])
        .with_detail("backend `cuda` declares whole-request remote dispatch on ready cluster nodes")
}

#[allow(dead_code)]
#[must_use]
pub fn cuda_replica_routed_capability_profile() -> ClusterExecutionCapabilityProfile {
    ClusterExecutionCapabilityProfile::new("cuda")
        .with_supported_lanes(vec![
            ClusterExecutionLane::RemoteWholeRequest,
            ClusterExecutionLane::ReplicaRouted,
        ])
        .with_detail(
            "backend `cuda` declares whole-request dispatch plus replica routing across warm lanes",
        )
}

#[allow(dead_code)]
#[must_use]
pub fn metal_cluster_blocked_capability_profile() -> ClusterExecutionCapabilityProfile {
    ClusterExecutionCapabilityProfile::new("metal").with_detail(
        "backend `metal` remains refused for cluster execution until the Metal roadmap queue `#3286` -> `#3285` -> `#3269` -> `#3262` closes",
    )
}

pub struct ClusterValidationFixture {
    pub cluster_id: ClusterId,
    pub snapshot: ClusterSnapshot,
}

#[allow(dead_code)]
pub enum ClusterValidationFault {
    ArtifactStatus {
        node_id: &'static str,
        status: ClusterArtifactResidencyStatus,
    },
    BackendDegraded {
        node_id: &'static str,
    },
    LowFreeMemory {
        node_id: &'static str,
        free_memory_bytes: u64,
    },
    SchedulerLinkDegraded {
        node_id: &'static str,
    },
    MeshLinkUnsuitable {
        left: &'static str,
        right: &'static str,
    },
    RemoveMeshLink {
        left: &'static str,
        right: &'static str,
    },
}

impl ClusterValidationFixture {
    #[must_use]
    pub fn new() -> Self {
        let cluster_id = sample_cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships.insert(
            NodeId::new("scheduler"),
            ready_membership(&cluster_id, "scheduler", NodeRole::Mixed),
        );
        for worker in ["worker-a", "worker-b", "worker-c"] {
            snapshot.memberships.insert(
                NodeId::new(worker),
                ready_membership(&cluster_id, worker, NodeRole::ExecutorOnly),
            );
            snapshot.telemetry.insert(
                NodeId::new(worker),
                ready_cuda_telemetry(worker, worker_free_memory_bytes(worker)),
            );
            snapshot
                .links
                .insert(link_key("scheduler", worker), scheduler_link(worker));
            snapshot.artifact_residency.insert(
                ClusterArtifactResidencyKey::new(NodeId::new(worker), ARTIFACT_DIGEST),
                ClusterArtifactResidencyRecord::new(
                    NodeId::new(worker),
                    ClusterArtifactReference::new("decoder", ARTIFACT_DIGEST),
                    ClusterArtifactResidencyStatus::Resident,
                ),
            );
        }
        for (left, right) in [
            ("worker-a", "worker-b"),
            ("worker-a", "worker-c"),
            ("worker-b", "worker-c"),
        ] {
            snapshot
                .links
                .insert(link_key(left, right), mesh_link(left, right));
        }
        Self {
            cluster_id,
            snapshot,
        }
    }

    #[must_use]
    pub fn state(&self) -> ClusterState {
        ClusterState::from_snapshot(self.snapshot.clone())
    }

    #[must_use]
    pub fn whole_request(&self) -> WholeRequestSchedulingRequest {
        WholeRequestSchedulingRequest::new(NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile())
            .with_served_artifact_digest(ARTIFACT_DIGEST)
            .with_minimum_free_memory_bytes(16 * 1024 * 1024 * 1024)
            .requiring_accelerator()
    }

    #[must_use]
    pub fn layer_request(&self) -> LayerShardedExecutionRequest {
        LayerShardedExecutionRequest::new(NodeId::new("scheduler"), ARTIFACT_DIGEST, 40, 2)
            .with_minimum_free_memory_bytes_per_shard(16 * 1024 * 1024 * 1024)
            .with_handoff_bytes_per_token(8_192, 4_096)
    }

    #[must_use]
    pub fn layer_policy(&self) -> LayerShardedExecutionPolicy {
        LayerShardedExecutionPolicy::cuda_default()
    }

    #[must_use]
    pub fn tensor_request(&self) -> TensorShardedExecutionRequest {
        TensorShardedExecutionRequest::new(
            NodeId::new("scheduler"),
            ARTIFACT_DIGEST,
            TensorShardedModelEligibility::new(1, 64).with_minimum_partition_size(16),
            2,
        )
        .with_collective_bytes_per_token(16_384)
        .with_minimum_free_memory_bytes_per_shard(16 * 1024 * 1024 * 1024)
    }

    #[must_use]
    pub fn tensor_policy(&self) -> TensorShardedTransportPolicy {
        TensorShardedTransportPolicy::cuda_default()
    }

    #[must_use]
    pub fn replica_lane(&self) -> ClusterReplicaLaneKey {
        ClusterReplicaLaneKey::new(
            REPLICA_PRODUCT_ID,
            REPLICA_MODEL_ID,
            REPLICA_RUNTIME_BACKEND,
            ARTIFACT_DIGEST,
        )
    }

    #[must_use]
    pub fn replica_snapshot_with_warm_nodes(&self, node_ids: &[&str]) -> ClusterReplicaSnapshot {
        let lane = self.replica_lane();
        node_ids.iter().fold(
            ClusterReplicaSnapshot::new(self.cluster_id.clone(), lane.clone()),
            |snapshot, node_id| {
                snapshot.with_replica(ClusterReplicaRecord::new(
                    lane.clone(),
                    NodeId::new(*node_id),
                    ClusterReplicaWarmState::Warm,
                ))
            },
        )
    }

    #[must_use]
    pub fn load_snapshot_with_slow_nodes(&self, slow_nodes: &[&str]) -> ClusterServingLoadSnapshot {
        ["worker-a", "worker-b", "worker-c"].into_iter().fold(
            ClusterServingLoadSnapshot::new(self.cluster_id.clone()),
            |snapshot, node_id| {
                let load = if slow_nodes.contains(&node_id) {
                    ClusterNodeServiceLoad::new(NodeId::new(node_id))
                        .with_service_health(ClusterNodeServiceHealth::Slow)
                } else {
                    ClusterNodeServiceLoad::new(NodeId::new(node_id))
                };
                snapshot.with_node_load(load)
            },
        )
    }

    #[must_use]
    pub fn serving_request(&self, request_id: &str) -> ClusterServingRequest {
        ClusterServingRequest::new(request_id, ClusterServingWorkClass::Decode)
    }

    #[allow(dead_code)]
    pub fn set_leadership(
        &mut self,
        leader_id: &str,
        term: ClusterTerm,
        committed_event_index: u64,
        heartbeat_tick: u64,
    ) {
        self.snapshot.leadership = Some(
            ClusterLeadershipRecord::new(
                term,
                NodeId::new(leader_id),
                cluster_event_index(committed_event_index),
            )
            .with_lease_policy(
                ClusterLeaseTick::new(heartbeat_tick),
                ClusterLeadershipLeasePolicy::new(4),
            ),
        );
    }

    #[allow(dead_code)]
    pub fn seed_command_provenance(&mut self) {
        self.snapshot.membership_provenance.insert(
            NodeId::new("scheduler"),
            sample_command_authorization(
                "scheduler",
                NodeRole::Mixed,
                ClusterCommandAuthorityScope::SelfNode,
                "scheduler-membership-command",
            ),
        );
        for worker in ["worker-a", "worker-b", "worker-c"] {
            self.snapshot.membership_provenance.insert(
                NodeId::new(worker),
                sample_command_authorization(
                    worker,
                    NodeRole::ExecutorOnly,
                    ClusterCommandAuthorityScope::SelfNode,
                    format!("{worker}-membership-command"),
                ),
            );
            self.snapshot.artifact_residency_provenance.insert(
                ClusterArtifactResidencyKey::new(NodeId::new(worker), ARTIFACT_DIGEST),
                sample_command_authorization(
                    worker,
                    NodeRole::ExecutorOnly,
                    ClusterCommandAuthorityScope::SelfNode,
                    format!("{worker}-artifact-command"),
                ),
            );
        }
        self.snapshot.leadership_provenance = Some(sample_command_authorization(
            "scheduler",
            NodeRole::Mixed,
            ClusterCommandAuthorityScope::ProposedLeader,
            "leadership-command",
        ));
    }

    #[allow(dead_code)]
    pub fn inject(&mut self, fault: ClusterValidationFault) {
        match fault {
            ClusterValidationFault::ArtifactStatus { node_id, status } => {
                self.snapshot.artifact_residency.insert(
                    ClusterArtifactResidencyKey::new(NodeId::new(node_id), ARTIFACT_DIGEST),
                    ClusterArtifactResidencyRecord::new(
                        NodeId::new(node_id),
                        ClusterArtifactReference::new("decoder", ARTIFACT_DIGEST),
                        status,
                    ),
                );
            }
            ClusterValidationFault::BackendDegraded { node_id } => {
                self.snapshot.telemetry.insert(
                    NodeId::new(node_id),
                    ready_cuda_telemetry(node_id, worker_free_memory_bytes(node_id))
                        .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Degraded)
                        .with_stability_posture(ClusterStabilityPosture::Flaky),
                );
            }
            ClusterValidationFault::LowFreeMemory {
                node_id,
                free_memory_bytes,
            } => {
                self.snapshot.telemetry.insert(
                    NodeId::new(node_id),
                    ready_cuda_telemetry(node_id, free_memory_bytes),
                );
            }
            ClusterValidationFault::SchedulerLinkDegraded { node_id } => {
                self.snapshot.links.insert(
                    link_key("scheduler", node_id),
                    ClusterLink::new(
                        NodeId::new("scheduler"),
                        NodeId::new(node_id),
                        ClusterTransportClass::LanUdp,
                        ClusterLinkStatus::Degraded,
                    )
                    .with_link_class(ClusterLinkClass::Wifi)
                    .with_latency_us(900)
                    .with_bandwidth_mbps(250)
                    .with_stability_posture(ClusterStabilityPosture::Flaky),
                );
            }
            ClusterValidationFault::MeshLinkUnsuitable { left, right } => {
                self.snapshot.links.insert(
                    link_key(left, right),
                    ClusterLink::new(
                        NodeId::new(left),
                        NodeId::new(right),
                        ClusterTransportClass::LanUdp,
                        ClusterLinkStatus::Healthy,
                    )
                    .with_link_class(ClusterLinkClass::Wifi)
                    .with_latency_us(2_500)
                    .with_bandwidth_mbps(500),
                );
            }
            ClusterValidationFault::RemoveMeshLink { left, right } => {
                self.snapshot.links.remove(&link_key(left, right));
            }
        }
    }
}

#[must_use]
pub fn sample_cluster_id() -> ClusterId {
    ClusterId::new(
        &ClusterNamespace::new(CLUSTER_NAMESPACE),
        &AdmissionToken::new(CLUSTER_SECRET),
    )
}

#[must_use]
pub fn ready_membership(
    cluster_id: &ClusterId,
    node_id: &str,
    role: NodeRole,
) -> ClusterMembershipRecord {
    ClusterMembershipRecord::new(
        ClusterNodeIdentity {
            cluster_id: cluster_id.clone(),
            node_id: NodeId::new(node_id),
            node_epoch: NodeEpoch::initial(),
            role,
            auth_public_key: String::new(),
            attestation: None,
        },
        None,
        ClusterMembershipStatus::Ready,
    )
}

#[must_use]
pub fn ready_cuda_telemetry(node_id: &str, free_memory_bytes: u64) -> ClusterNodeTelemetry {
    ClusterNodeTelemetry::new(NodeId::new(node_id))
        .with_memory(Some(64 * 1024 * 1024 * 1024), Some(free_memory_bytes))
        .with_cpu_logical_cores(16)
        .with_accelerator_count(1)
        .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready)
}

#[must_use]
pub fn scheduler_link(right: &str) -> ClusterLink {
    ClusterLink::new(
        NodeId::new("scheduler"),
        NodeId::new(right),
        ClusterTransportClass::LanUdp,
        ClusterLinkStatus::Healthy,
    )
    .with_link_class(ClusterLinkClass::Ethernet)
    .with_latency_us(500)
    .with_bandwidth_mbps(25_000)
}

#[must_use]
pub fn mesh_link(left: &str, right: &str) -> ClusterLink {
    ClusterLink::new(
        NodeId::new(left),
        NodeId::new(right),
        ClusterTransportClass::Rdma,
        ClusterLinkStatus::Healthy,
    )
    .with_link_class(ClusterLinkClass::Rdma)
    .with_latency_us(120)
    .with_bandwidth_mbps(100_000)
}

#[must_use]
pub fn link_key(left: &str, right: &str) -> ClusterLinkKey {
    ClusterLinkKey::new(NodeId::new(left), NodeId::new(right))
}

#[must_use]
pub fn worker_free_memory_bytes(worker: &str) -> u64 {
    match worker {
        "worker-a" => 48 * 1024 * 1024 * 1024,
        "worker-b" => 40 * 1024 * 1024 * 1024,
        "worker-c" => 32 * 1024 * 1024 * 1024,
        _ => 24 * 1024 * 1024 * 1024,
    }
}

#[allow(dead_code)]
#[must_use]
pub fn cluster_event_index(raw: u64) -> ClusterEventIndex {
    let mut index = ClusterEventIndex::initial();
    for _ in 1..raw.max(1) {
        index = index.next();
    }
    index
}

#[must_use]
pub fn sample_recovery_log() -> ClusterEventLog {
    let cluster_id = sample_cluster_id();
    let mut log = ClusterEventLog::new(cluster_id.clone());
    let _ = log.append_event(ClusterEvent::MembershipReconciled {
        membership: ready_membership(&cluster_id, "scheduler", NodeRole::Mixed),
    });
    let _ = log.append_event(ClusterEvent::MembershipReconciled {
        membership: ready_membership(&cluster_id, "worker-a", NodeRole::ExecutorOnly),
    });
    let _ = log.append_event(ClusterEvent::NodeTelemetryReconciled {
        telemetry: ready_cuda_telemetry("worker-a", worker_free_memory_bytes("worker-a")),
    });
    let _ = log.append_event(ClusterEvent::ConnectionReconciled {
        link: scheduler_link("worker-a"),
    });
    let _ = log.append_event(ClusterEvent::ArtifactResidencyReconciled {
        residency: ClusterArtifactResidencyRecord::new(
            NodeId::new("worker-a"),
            ClusterArtifactReference::new("decoder", ARTIFACT_DIGEST),
            ClusterArtifactResidencyStatus::Resident,
        ),
    });
    log
}

#[must_use]
pub fn stale_rejoin_request(cluster_id: &ClusterId) -> ClusterCatchupRequest {
    ClusterCatchupRequest::new(
        cluster_id.clone(),
        NodeId::new("worker-rejoin"),
        Some(ClusterEventIndex::initial()),
        psionic_cluster::ClusterSchemaVersion::initial(),
        16,
    )
}

#[must_use]
pub fn recovery_policy() -> ClusterRecoveryPolicy {
    ClusterRecoveryPolicy {
        max_events_per_response: 16,
        retain_tail_events: 2,
        full_resync_gap_threshold: 32,
    }
}

fn sample_command_authorization(
    submitter_node_id: &str,
    submitter_role: NodeRole,
    authority_scope: ClusterCommandAuthorityScope,
    command_digest: impl Into<String>,
) -> ClusterCommandAuthorization {
    ClusterCommandAuthorization {
        command_digest: command_digest.into(),
        authorization_policy_digest: String::from("command-authorization-policy"),
        authority_scope,
        submitter_node_id: NodeId::new(submitter_node_id),
        submitter_role,
        submitter_membership_status: ClusterMembershipStatus::Ready,
        coordinator_authority: None,
    }
}
