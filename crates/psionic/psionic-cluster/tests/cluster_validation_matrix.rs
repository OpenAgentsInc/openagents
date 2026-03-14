#![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

mod support;

use std::io::Error;

use ed25519_dalek::SigningKey;
use psionic_cluster::{
    ClusterArtifactResidencyStatus, ClusterCatchupPayload, ClusterCommand,
    ClusterCommandAuthorizationError, ClusterCommandAuthorizationPolicy,
    ClusterCommandAuthorizationRefusalCode, ClusterDiscoveredCandidateRecord,
    ClusterDiscoveredCandidateStatus, ClusterEvent, ClusterEventIndex, ClusterHistoryError,
    ClusterIntroductionPayload, ClusterIntroductionPolicy, ClusterIntroductionSource,
    ClusterIntroductionVerificationError, ClusterLeaseTick, ClusterRecoveryDisposition,
    ClusterRecoveryReason, ClusterReplicaLifecyclePolicy, ClusterServingPolicy, ClusterTerm,
    IndexedClusterEvent, LayerShardedSchedulingFailureCode, NodeId, NodeRole,
    PipelineShardedSchedulingFailureCode, SignedClusterIntroductionEnvelope,
    TensorShardedSchedulingFailureCode, WholeRequestSchedulingFailureCode, plan_replicated_serving,
    schedule_layer_sharded_execution, schedule_pipeline_sharded_execution,
    schedule_remote_whole_request, schedule_tensor_sharded_execution,
};
use psionic_runtime::{
    ClusterAdmissionFactKind, ClusterCommunicationClass, ClusterPolicyDigestKind,
    ClusterReplicaRoutingDisposition, ClusterSettlementProvenanceInput, ClusterShardHandoffKind,
    ExecutionTopologyKind,
};
use support::{
    ClusterValidationFault, ClusterValidationFixture, metal_cluster_blocked_capability_profile,
    recovery_policy, sample_cluster_id, sample_recovery_log, stale_rejoin_request,
};

fn fixture_error(detail: impl Into<String>) -> Error {
    Error::other(detail.into())
}

fn sample_discovery_introduction_signing_key() -> SigningKey {
    SigningKey::from_bytes(&[31; 32])
}

fn sample_discovery_introduction_policy(signing_key: &SigningKey) -> ClusterIntroductionPolicy {
    ClusterIntroductionPolicy::new(
        vec![ClusterIntroductionSource::new(
            "operator-source",
            hex::encode(signing_key.verifying_key().to_bytes()),
        )],
        60_000,
    )
}

fn sample_signed_discovery_introduction(
    signing_key: &SigningKey,
    node_id: &str,
    port: u16,
) -> SignedClusterIntroductionEnvelope {
    SignedClusterIntroductionEnvelope::sign(
        ClusterIntroductionPayload::new(
            psionic_cluster::ClusterDiscoveryCandidate::new(
                sample_cluster_id(),
                psionic_cluster::ClusterNamespace::new(support::CLUSTER_NAMESPACE),
                NodeId::new(node_id),
                NodeRole::ExecutorOnly,
                format!("candidate-public-key-{port}"),
                vec![std::net::SocketAddr::from(([10, 42, 0, 1], port))],
            ),
            10_000,
            20_000,
        ),
        "operator-source",
        signing_key,
    )
}

#[test]
fn recovery_validation_installs_snapshot_after_compaction_boundary()
-> Result<(), Box<dyn std::error::Error>> {
    let mut log = sample_recovery_log();
    let policy = recovery_policy();

    assert!(log.compact(&policy)?.is_some());

    let response = log.catchup_response(&stale_rejoin_request(&sample_cluster_id()), &policy)?;

    assert_eq!(
        response.disposition,
        ClusterRecoveryDisposition::InstallSnapshot
    );
    match response.payload {
        ClusterCatchupPayload::Snapshot {
            snapshot,
            tail_events,
            reason,
        } => {
            assert!(!tail_events.is_empty());
            assert_eq!(snapshot.cluster_id, sample_cluster_id());
            assert!(matches!(
                reason,
                ClusterRecoveryReason::CompactionBoundary { .. }
            ));
        }
        payload => panic!("expected snapshot install, got {payload:?}"),
    }
    Ok(())
}

#[test]
fn scheduling_validation_covers_staging_and_degraded_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = ClusterValidationFixture::new();
    fixture.set_leadership("scheduler", ClusterTerm::initial(), 4, 10);
    fixture.seed_command_provenance();
    fixture.inject(ClusterValidationFault::ArtifactStatus {
        node_id: "worker-a",
        status: ClusterArtifactResidencyStatus::CopyRequired,
    });
    fixture.inject(ClusterValidationFault::BackendDegraded {
        node_id: "worker-a",
    });
    fixture.inject(ClusterValidationFault::SchedulerLinkDegraded {
        node_id: "worker-a",
    });
    fixture.inject(ClusterValidationFault::LowFreeMemory {
        node_id: "worker-b",
        free_memory_bytes: 8 * 1024 * 1024 * 1024,
    });
    fixture.inject(ClusterValidationFault::ArtifactStatus {
        node_id: "worker-c",
        status: ClusterArtifactResidencyStatus::Refused,
    });

    let schedule = schedule_remote_whole_request(&fixture.state(), &fixture.whole_request())
        .map_err(|err| {
            fixture_error(format!("whole-request scheduling should succeed: {err:?}"))
        })?;

    assert_eq!(schedule.selected_node_id.as_str(), "worker-a");
    assert!(schedule.selection_notes.iter().any(|note| note.code
        == psionic_cluster::WholeRequestSchedulingSelectionCode::ArtifactCopyRequired));
    assert!(
        schedule.selection_notes.iter().any(|note| note.code
            == psionic_cluster::WholeRequestSchedulingSelectionCode::BackendDegraded)
    );
    assert!(
        schedule.selection_notes.iter().any(|note| note.code
            == psionic_cluster::WholeRequestSchedulingSelectionCode::TransportDegraded)
    );
    assert_eq!(
        schedule
            .cluster_execution
            .selected_nodes
            .first()
            .and_then(|node| node.artifact_residency),
        Some(psionic_runtime::ClusterArtifactResidencyDisposition::CopyRequired)
    );
    assert!(
        schedule
            .cluster_execution
            .degraded_reason
            .as_deref()
            .is_some_and(|detail| detail.contains("requires peer-copy artifact staging"))
    );
    assert_eq!(schedule.cluster_execution.command_provenance.len(), 4);
    assert!(
        schedule
            .cluster_execution
            .command_provenance
            .iter()
            .any(|fact| fact.fact_kind == ClusterAdmissionFactKind::Leadership)
    );
    let settlement_provenance =
        ClusterSettlementProvenanceInput::from_cluster_execution(&schedule.cluster_execution)
            .ok_or_else(|| {
                fixture_error("whole-request schedule should yield settlement provenance")
            })?;
    assert_eq!(settlement_provenance.command_provenance.len(), 4);
    assert_eq!(
        settlement_provenance.coordinator_authority_digest,
        schedule
            .cluster_execution
            .commit_authority
            .as_ref()
            .map(|authority| authority.authority_digest.clone())
    );
    Ok(())
}

#[test]
fn scheduling_validation_refuses_metal_cluster_dispatch_explicitly()
-> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = ClusterValidationFixture::new();
    for (node_id, free_memory_bytes) in [
        ("worker-a", 24 * 1024 * 1024 * 1024),
        ("worker-b", 32 * 1024 * 1024 * 1024),
        ("worker-c", 40 * 1024 * 1024 * 1024),
    ] {
        fixture.snapshot.telemetry.insert(
            NodeId::new(node_id),
            psionic_cluster::ClusterNodeTelemetry::new(NodeId::new(node_id))
                .with_memory(Some(64 * 1024 * 1024 * 1024), Some(free_memory_bytes))
                .with_cpu_logical_cores(16)
                .with_accelerator_count(1)
                .with_backend_readiness(
                    "metal",
                    psionic_cluster::ClusterBackendReadinessStatus::Ready,
                ),
        );
    }
    let request =
        psionic_cluster::WholeRequestSchedulingRequest::new(NodeId::new("scheduler"), "metal")
            .with_capability_profile(metal_cluster_blocked_capability_profile());

    let failure = schedule_remote_whole_request(&fixture.state(), &request)
        .expect_err("metal cluster dispatch should remain refused");

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
            .contains("`#3286` -> `#3285` -> `#3269` -> `#3262`")
    );
    Ok(())
}

#[test]
fn discovery_validation_covers_intake_refusal_expiry_and_reconciliation()
-> Result<(), Box<dyn std::error::Error>> {
    let signing_key = sample_discovery_introduction_signing_key();
    let policy = sample_discovery_introduction_policy(&signing_key);
    let envelope = sample_signed_discovery_introduction(&signing_key, "discovery-worker", 5101);
    let candidate_record =
        ClusterDiscoveredCandidateRecord::from_signed_introduction(&envelope, &policy)?;
    assert_eq!(
        candidate_record.status,
        ClusterDiscoveredCandidateStatus::Introduced
    );

    let refusal = ClusterDiscoveredCandidateRecord::from_signed_introduction(
        &envelope,
        &ClusterIntroductionPolicy::new(
            vec![ClusterIntroductionSource::new(
                "different-source",
                hex::encode(signing_key.verifying_key().to_bytes()),
            )],
            60_000,
        ),
    )
    .expect_err("untrusted introduction source should be refused");
    assert!(matches!(
        refusal,
        ClusterIntroductionVerificationError::UntrustedSource { .. }
    ));

    let mut fixture = ClusterValidationFixture::new();
    fixture.set_leadership("scheduler", ClusterTerm::initial(), 4, 10);
    let mut state = fixture.state();
    let authorization_policy = ClusterCommandAuthorizationPolicy::default();
    let reconcile_authorization = state
        .authorize_command(
            &NodeId::new("scheduler"),
            &ClusterCommand::ReconcileDiscoveryCandidate {
                candidate: candidate_record.clone(),
            },
            &authorization_policy,
        )
        .map_err(|err| fixture_error(format!("candidate intake should authorize: {err:?}")))?;
    state.apply(
        IndexedClusterEvent::new(
            fixture.cluster_id.clone(),
            ClusterEventIndex::initial(),
            ClusterEvent::DiscoveryCandidateReconciled {
                candidate: candidate_record.clone(),
            },
        )
        .with_command_authorization(reconcile_authorization.clone()),
    )?;
    assert!(
        state
            .memberships()
            .get(&candidate_record.candidate.node_id)
            .is_none(),
        "discovery intake must not silently widen admitted membership"
    );

    let admitted_membership = support::ready_membership(
        &fixture.cluster_id,
        candidate_record.candidate.node_id.as_str(),
        NodeRole::ExecutorOnly,
    );
    let admit_authorization = state
        .authorize_command(
            &NodeId::new("scheduler"),
            &ClusterCommand::AdmitDiscoveryCandidate {
                node_id: candidate_record.candidate.node_id.clone(),
                advertised_addr: candidate_record.candidate.advertised_addrs.first().copied(),
                membership_status: admitted_membership.status,
            },
            &authorization_policy,
        )
        .map_err(|err| fixture_error(format!("candidate admission should authorize: {err:?}")))?;
    state.apply(
        IndexedClusterEvent::new(
            fixture.cluster_id.clone(),
            ClusterEventIndex::initial().next(),
            ClusterEvent::DiscoveryCandidateAdmitted {
                node_id: candidate_record.candidate.node_id.clone(),
                membership: admitted_membership.clone(),
            },
        )
        .with_command_authorization(admit_authorization.clone()),
    )?;
    assert_eq!(
        state.memberships().get(&candidate_record.candidate.node_id),
        Some(&admitted_membership)
    );
    assert_eq!(
        state
            .discovery_candidates()
            .get(&candidate_record.candidate.node_id)
            .map(|candidate| candidate.status),
        Some(ClusterDiscoveredCandidateStatus::Accepted)
    );
    assert_eq!(
        state.discovery_candidate_provenance(&candidate_record.candidate.node_id),
        Some(&admit_authorization)
    );

    let expired_candidate = candidate_record
        .clone()
        .with_status(ClusterDiscoveredCandidateStatus::Expired)
        .with_detail("introduction_expired");
    let mut expired_fixture = ClusterValidationFixture::new();
    expired_fixture.set_leadership("scheduler", ClusterTerm::initial(), 4, 10);
    let mut expired_state = expired_fixture.state();
    let expired_authorization = expired_state
        .authorize_command(
            &NodeId::new("scheduler"),
            &ClusterCommand::ReconcileDiscoveryCandidate {
                candidate: expired_candidate.clone(),
            },
            &authorization_policy,
        )
        .map_err(|err| fixture_error(format!("expired candidate should authorize: {err:?}")))?;
    expired_state.apply(
        IndexedClusterEvent::new(
            expired_fixture.cluster_id.clone(),
            ClusterEventIndex::initial(),
            ClusterEvent::DiscoveryCandidateReconciled {
                candidate: expired_candidate,
            },
        )
        .with_command_authorization(expired_authorization),
    )?;

    let expired_admission = expired_state.apply(IndexedClusterEvent::new(
        expired_fixture.cluster_id,
        ClusterEventIndex::initial().next(),
        ClusterEvent::DiscoveryCandidateAdmitted {
            node_id: candidate_record.candidate.node_id.clone(),
            membership: support::ready_membership(
                &sample_cluster_id(),
                candidate_record.candidate.node_id.as_str(),
                NodeRole::ExecutorOnly,
            ),
        },
    ));
    assert!(matches!(
        expired_admission,
        Err(ClusterHistoryError::DiscoveryCandidateAdmissionRefused {
            status: ClusterDiscoveredCandidateStatus::Expired,
            ..
        })
    ));
    Ok(())
}

#[test]
fn replication_validation_reroutes_away_from_slow_replica() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = ClusterValidationFixture::new();
    let decision = plan_replicated_serving(
        &fixture.state(),
        &fixture.load_snapshot_with_slow_nodes(&["worker-a"]),
        &fixture.replica_snapshot_with_warm_nodes(&["worker-a", "worker-b"]),
        &ClusterReplicaLifecyclePolicy::replicated_lane(),
        &ClusterServingPolicy::direct_caller_latency_first(),
        &fixture.serving_request("validation-replica-1"),
        &fixture.whole_request(),
    )
    .map_err(|err| fixture_error(format!("replicated serving should succeed: {err:?}")))?;

    assert_eq!(
        decision.serving_decision.schedule.selected_node_id.as_str(),
        "worker-b"
    );
    assert!(
        decision
            .serving_decision
            .schedule
            .cluster_execution
            .fallback_history
            .iter()
            .any(|step| {
                step.from_node_id.as_deref() == Some("worker-a") && step.to_node_id == "worker-b"
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
    Ok(())
}

#[test]
fn sharding_validation_covers_layer_and_tensor_evidence() -> Result<(), Box<dyn std::error::Error>>
{
    let mut fixture = ClusterValidationFixture::new();
    fixture.set_leadership("scheduler", ClusterTerm::initial(), 4, 10);
    fixture.seed_command_provenance();
    let layer_schedule = schedule_layer_sharded_execution(
        &fixture.state(),
        &fixture.layer_request(),
        &fixture.layer_policy(),
    )
    .map_err(|err| fixture_error(format!("layer sharding should succeed: {err:?}")))?;
    let tensor_schedule = schedule_tensor_sharded_execution(
        &fixture.state(),
        &fixture.tensor_request(),
        &fixture.tensor_policy(),
    )
    .map_err(|err| fixture_error(format!("tensor sharding should succeed: {err:?}")))?;

    assert_eq!(
        layer_schedule.execution_topology.kind,
        ExecutionTopologyKind::LayerSharded
    );
    assert_eq!(layer_schedule.shard_handoffs.len(), 2);
    assert_eq!(
        layer_schedule.shard_handoffs[0].kind,
        ClusterShardHandoffKind::Activation
    );
    assert_eq!(
        layer_schedule.shard_handoffs[1].kind,
        ClusterShardHandoffKind::KvCache
    );

    assert_eq!(
        tensor_schedule.execution_topology.kind,
        ExecutionTopologyKind::TensorSharded
    );
    assert_eq!(tensor_schedule.shard_handoffs.len(), 1);
    assert_eq!(
        tensor_schedule.shard_handoffs[0].kind,
        ClusterShardHandoffKind::TensorCollective
    );
    assert_eq!(tensor_schedule.shard_handoffs[0].tensor_axis, Some(1));
    assert_eq!(
        tensor_schedule.shard_handoffs[0].tensor_range_start,
        Some(0)
    );
    assert_eq!(tensor_schedule.shard_handoffs[0].tensor_range_end, Some(32));
    assert_eq!(layer_schedule.cluster_execution.command_provenance.len(), 6);
    assert_eq!(
        tensor_schedule.cluster_execution.command_provenance.len(),
        6
    );
    assert!(
        layer_schedule
            .cluster_execution
            .command_provenance
            .iter()
            .any(|fact| fact.fact_kind == ClusterAdmissionFactKind::Leadership)
    );
    assert!(
        tensor_schedule
            .cluster_execution
            .command_provenance
            .iter()
            .any(|fact| fact.fact_kind == ClusterAdmissionFactKind::ArtifactResidency)
    );
    Ok(())
}

#[test]
fn pipeline_validation_covers_public_network_stage_truth() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = ClusterValidationFixture::new();
    let state = fixture.public_pipeline_state();
    let schedule = schedule_pipeline_sharded_execution(
        &state,
        &fixture.pipeline_request(),
        &fixture.pipeline_policy(),
    )
    .map_err(|error| {
        fixture_error(format!(
            "expected public-network pipeline schedule: {error:?}"
        ))
    })?;

    assert_eq!(
        schedule.execution_topology.kind,
        ExecutionTopologyKind::PipelineSharded
    );
    assert_eq!(
        schedule
            .cluster_execution
            .communication_eligibility
            .as_ref()
            .map(|eligibility| eligibility.required_class),
        Some(ClusterCommunicationClass::PipelineStageHandoff)
    );
    assert_eq!(schedule.pipeline_stages.len(), 3);
    assert_eq!(
        schedule.pipeline_stages[0].handoff_transport,
        Some(psionic_runtime::ClusterTransportClass::WiderNetworkStream)
    );
    assert_eq!(
        schedule.cluster_execution.pipeline_stages,
        schedule.pipeline_stages
    );
    Ok(())
}

#[test]
fn authorization_validation_covers_allowed_and_refused_cluster_commands()
-> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = ClusterValidationFixture::new();
    fixture.set_leadership("scheduler", ClusterTerm::initial(), 4, 10);
    let state = fixture.state();
    let allowed = state
        .authorize_command(
            &NodeId::new("worker-a"),
            &ClusterCommand::ReconcileMembership {
                membership: fixture
                    .snapshot
                    .memberships
                    .get(&NodeId::new("worker-a"))
                    .cloned()
                    .ok_or_else(|| fixture_error("worker-a membership should exist"))?,
            },
            &ClusterCommandAuthorizationPolicy::default(),
        )
        .map_err(|err| {
            fixture_error(format!(
                "worker self-membership reconcile should succeed: {err:?}"
            ))
        })?;
    assert_eq!(allowed.submitter_node_id.as_str(), "worker-a");

    let refusal = state
        .authorize_command(
            &NodeId::new("worker-a"),
            &ClusterCommand::RemoveMember {
                node_id: NodeId::new("worker-c"),
                reason: String::from("validation refusal"),
            },
            &ClusterCommandAuthorizationPolicy::default(),
        )
        .expect_err("non-coordinator member removal should be refused");
    assert!(matches!(
        refusal,
        ClusterCommandAuthorizationError::Refused { refusal }
            if refusal.code == ClusterCommandAuthorizationRefusalCode::CoordinatorRequired
    ));
    Ok(())
}

#[test]
fn fault_injection_covers_mesh_refusal_for_sharded_paths() -> Result<(), Box<dyn std::error::Error>>
{
    let mut layer_fixture = ClusterValidationFixture::new();
    let mut tensor_fixture = ClusterValidationFixture::new();
    for (left, right) in [
        ("worker-a", "worker-b"),
        ("worker-a", "worker-c"),
        ("worker-b", "worker-c"),
    ] {
        layer_fixture.inject(ClusterValidationFault::MeshLinkUnsuitable { left, right });
        tensor_fixture.inject(ClusterValidationFault::MeshLinkUnsuitable { left, right });
    }
    tensor_fixture.inject(ClusterValidationFault::RemoveMeshLink {
        left: "worker-b",
        right: "worker-c",
    });

    let layer_failure = schedule_layer_sharded_execution(
        &layer_fixture.state(),
        &layer_fixture.layer_request(),
        &layer_fixture.layer_policy(),
    )
    .expect_err("unsuitable stream mesh should refuse layer sharding");
    let tensor_failure = schedule_tensor_sharded_execution(
        &tensor_fixture.state(),
        &tensor_fixture.tensor_request(),
        &tensor_fixture.tensor_policy(),
    )
    .expect_err("unsuitable mesh should refuse tensor sharding");

    assert_eq!(
        layer_failure.code,
        LayerShardedSchedulingFailureCode::HandoffLinkUnsuitable
    );
    assert_eq!(
        tensor_failure.code,
        TensorShardedSchedulingFailureCode::MeshLinkUnsuitable
    );
    let pipeline_fixture = ClusterValidationFixture::new();
    let mut pipeline_snapshot = pipeline_fixture.public_pipeline_state().snapshot();
    for (left, right) in [
        ("worker-a", "worker-b"),
        ("worker-a", "worker-c"),
        ("worker-b", "worker-c"),
    ] {
        pipeline_snapshot.links.insert(
            support::link_key(left, right),
            psionic_cluster::ClusterLink::new(
                NodeId::new(left),
                NodeId::new(right),
                psionic_cluster::ClusterTransportClass::Tcp,
                psionic_cluster::ClusterLinkStatus::Healthy,
            )
            .with_link_class(psionic_cluster::ClusterLinkClass::Ethernet)
            .with_latency_us(180_000)
            .with_bandwidth_mbps(3_000),
        );
    }
    let pipeline_state = psionic_cluster::ClusterState::from_snapshot(pipeline_snapshot);
    let pipeline_failure = schedule_pipeline_sharded_execution(
        &pipeline_state,
        &pipeline_fixture.pipeline_request(),
        &pipeline_fixture.pipeline_policy(),
    )
    .expect_err("high-latency public stage mesh should refuse pipeline sharding");
    assert_eq!(
        pipeline_failure.code,
        PipelineShardedSchedulingFailureCode::TimingEnvelopeExceeded
    );
    Ok(())
}

#[test]
fn coordinator_authority_validation_surfaces_stale_leader_and_failover_fence_rotation()
-> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = ClusterValidationFixture::new();
    fixture.set_leadership("scheduler", ClusterTerm::initial(), 4, 10);

    let stale_state = fixture.state();
    let stale_diagnostic = stale_state
        .stale_leadership_diagnostic_at(ClusterLeaseTick::new(15))
        .ok_or_else(|| fixture_error("stale leadership should surface a diagnostic"))?;
    let stale_authority = stale_state
        .commit_authority()
        .ok_or_else(|| fixture_error("stale state should still expose last authority truth"))?;
    assert_eq!(stale_diagnostic.leader_id.as_str(), "scheduler");
    assert_eq!(stale_diagnostic.term.as_u64(), 1);

    fixture.set_leadership("worker-a", ClusterTerm::initial().next(), 5, 15);
    let schedule = schedule_remote_whole_request(&fixture.state(), &fixture.whole_request())
        .map_err(|err| {
            fixture_error(format!("whole-request scheduling should succeed: {err:?}"))
        })?;
    let authority = schedule
        .cluster_execution
        .commit_authority
        .ok_or_else(|| fixture_error("cluster execution should surface commit authority"))?;

    assert_eq!(authority.coordinator_node_id, "worker-a");
    assert_eq!(authority.term, 2);
    assert_ne!(authority.fence_token, stale_authority.fence_token);
    assert!(
        schedule
            .cluster_execution
            .policy_digests
            .iter()
            .any(|digest| digest.kind == ClusterPolicyDigestKind::Authority),
        "validation schedule should carry authority digest truth"
    );
    Ok(())
}

#[test]
fn split_brain_validation_refuses_conflicting_same_term_leadership()
-> Result<(), Box<dyn std::error::Error>> {
    let cluster_id = sample_cluster_id();
    let mut state = psionic_cluster::ClusterState::new(cluster_id.clone());
    let first = IndexedClusterEvent::new(
        cluster_id.clone(),
        ClusterEventIndex::initial(),
        ClusterEvent::LeadershipReconciled {
            leadership: psionic_cluster::ClusterLeadershipRecord::new(
                ClusterTerm::initial(),
                psionic_cluster::NodeId::new("leader-alpha"),
                ClusterEventIndex::initial(),
            )
            .with_lease_policy(
                ClusterLeaseTick::new(1),
                psionic_cluster::ClusterLeadershipLeasePolicy::new(4),
            ),
        },
    );
    state.apply(first)?;

    let conflicting = state.apply(IndexedClusterEvent::new(
        cluster_id,
        ClusterEventIndex::initial().next(),
        ClusterEvent::LeadershipReconciled {
            leadership: psionic_cluster::ClusterLeadershipRecord::new(
                ClusterTerm::initial(),
                psionic_cluster::NodeId::new("leader-beta"),
                ClusterEventIndex::initial().next(),
            )
            .with_lease_policy(
                ClusterLeaseTick::new(2),
                psionic_cluster::ClusterLeadershipLeasePolicy::new(4),
            ),
        },
    ));

    assert!(
        matches!(
            conflicting,
            Err(ClusterHistoryError::SplitBrainLeadership { diagnostic })
                if diagnostic.current_leader_id.as_str() == "leader-alpha"
                    && diagnostic.attempted_leader_id.as_str() == "leader-beta"
                    && diagnostic.term.as_u64() == 1
        ),
        "validation should refuse conflicting same-term leadership"
    );
    Ok(())
}
