mod support;

use std::io::Error;

use psionic_cluster::{
    ClusterArtifactResidencyStatus, ClusterCatchupPayload, ClusterEvent, ClusterEventIndex,
    ClusterHistoryError, ClusterLeaseTick, ClusterRecoveryDisposition, ClusterRecoveryReason,
    ClusterReplicaLifecyclePolicy, ClusterServingPolicy, ClusterTerm, IndexedClusterEvent,
    LayerShardedSchedulingFailureCode, TensorShardedSchedulingFailureCode, plan_replicated_serving,
    schedule_layer_sharded_execution, schedule_remote_whole_request,
    schedule_tensor_sharded_execution,
};
use psionic_runtime::{
    ClusterPolicyDigestKind, ClusterReplicaRoutingDisposition, ClusterShardHandoffKind,
    ExecutionTopologyKind,
};
use support::{
    ClusterValidationFault, ClusterValidationFixture, recovery_policy, sample_cluster_id,
    sample_recovery_log, stale_rejoin_request,
};

fn fixture_error(detail: impl Into<String>) -> Error {
    Error::other(detail.into())
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
    let fixture = ClusterValidationFixture::new();
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
