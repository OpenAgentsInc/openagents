use std::collections::{BTreeMap, BTreeSet};

use psionic_runtime::{
    CacheAction, CacheInvalidationTrigger, ClusterArtifactResidencyDisposition, ClusterCacheUsage,
    ClusterExecutionDisposition, ClusterExecutionLane, ClusterPolicyDigest,
    ClusterPolicyDigestKind, ClusterReplicaNode, ClusterReplicaRoutingDisposition,
    ClusterReplicaWarmState, ClusterSelectedNode as RuntimeClusterSelectedNode,
    ClusterServingSemantics, ClusterWarmRoutePosture, DeviceInventoryQualifiers,
    ExecutionTopologyPlan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ClusterId, ClusterLeadershipLeasePolicy, ClusterLeadershipLeaseStatus, ClusterLeaseTick,
    ClusterReplicaHostElectionReason, ClusterReplicaHostElectionRecord, ClusterServingDecision,
    ClusterServingFailure, ClusterServingLoadSnapshot, ClusterServingPolicy, ClusterServingRequest,
    ClusterState, ClusterTerm, NodeId, WholeRequestSchedulingRequest,
    plan_cluster_serving_admission, replica_routing_communication_eligibility,
};

/// Runtime contract required to execute one expert-family replica lane honestly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterReplicaLaneExpertRuntimeContract {
    /// Psionic already has a native family contract for the expert lane.
    GptOssNativeMoe,
    /// The lane requires later family-specific placement and topology truth.
    FamilySpecificPlacement,
}

/// Explicit expert-family topology truth attached to one replica lane.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterReplicaLaneExpertTopologyRequirement {
    /// Stable model-family label for the lane.
    pub family: String,
    /// Raw model-architecture label when one differs from family.
    pub architecture: String,
    /// Total declared expert count.
    pub expert_count: usize,
    /// Routed active-expert count per token when the lane declares it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_expert_count: Option<usize>,
    /// Expert feed-forward width when the lane declares it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expert_feed_forward_length: Option<usize>,
    /// Runtime contract required to execute the lane honestly.
    pub runtime_contract: ClusterReplicaLaneExpertRuntimeContract,
}

impl ClusterReplicaLaneExpertTopologyRequirement {
    /// Creates one explicit expert-family topology requirement.
    #[must_use]
    pub fn new(
        family: impl Into<String>,
        architecture: impl Into<String>,
        expert_count: usize,
        runtime_contract: ClusterReplicaLaneExpertRuntimeContract,
    ) -> Self {
        Self {
            family: family.into(),
            architecture: architecture.into(),
            expert_count,
            active_expert_count: None,
            expert_feed_forward_length: None,
            runtime_contract,
        }
    }

    /// Attaches routed active-expert count per token.
    #[must_use]
    pub fn with_active_expert_count(mut self, active_expert_count: usize) -> Self {
        self.active_expert_count = Some(active_expert_count);
        self
    }

    /// Attaches expert feed-forward width.
    #[must_use]
    pub fn with_expert_feed_forward_length(mut self, expert_feed_forward_length: usize) -> Self {
        self.expert_feed_forward_length = Some(expert_feed_forward_length);
        self
    }
}

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
    /// Honest expert-family topology truth when the lane cannot be described as
    /// a simple dense replica.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expert_topology_requirement: Option<ClusterReplicaLaneExpertTopologyRequirement>,
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
            expert_topology_requirement: None,
        }
    }

    /// Attaches the sharded-model manifest digest backing the replica lane.
    #[must_use]
    pub fn with_sharded_model_manifest_digest(mut self, digest: impl Into<String>) -> Self {
        self.sharded_model_manifest_digest = Some(digest.into());
        self
    }

    /// Attaches explicit expert-family topology truth for the lane.
    #[must_use]
    pub fn with_expert_topology_requirement(
        mut self,
        requirement: ClusterReplicaLaneExpertTopologyRequirement,
    ) -> Self {
        self.expert_topology_requirement = Some(requirement);
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
        hasher.update(b"|");
        if let Some(requirement) = &self.lane.expert_topology_requirement {
            hasher.update(requirement.family.as_bytes());
            hasher.update(b"|");
            hasher.update(requirement.architecture.as_bytes());
            hasher.update(b"|");
            hasher.update(requirement.expert_count.to_string());
            hasher.update(b"|");
            hasher.update(
                requirement
                    .active_expert_count
                    .map(|count| count.to_string())
                    .unwrap_or_default(),
            );
            hasher.update(b"|");
            hasher.update(
                requirement
                    .expert_feed_forward_length
                    .map(|count| count.to_string())
                    .unwrap_or_default(),
            );
            hasher.update(b"|");
            hasher.update(format!("{:?}", requirement.runtime_contract));
        }
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
    /// Routed request count carried safely by one warm replica before scale-out.
    pub requests_per_target_warm_replica: usize,
    /// Peak active requests tolerated on one warm replica before scale-out.
    pub active_requests_per_target_warm_replica: usize,
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
            requests_per_target_warm_replica: 4,
            active_requests_per_target_warm_replica: 2,
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
        hasher.update(self.requests_per_target_warm_replica.to_string());
        hasher.update(b"|");
        hasher.update(self.active_requests_per_target_warm_replica.to_string());
        hasher.update(b"|");
        hasher.update(self.idle_keepalive_seconds.to_string());
        hasher.update(b"|");
        hasher.update(self.unload_after_idle_seconds.to_string());
        hex::encode(hasher.finalize())
    }

    /// Computes the next explicit warm-capacity target from observed demand.
    #[must_use]
    pub fn rebalance_for_demand(
        &self,
        demand: Option<&ClusterReplicaDemandSnapshot>,
        current_warm_replicas: usize,
        observed_at_ms: u64,
    ) -> ClusterReplicaDemandRebalanceDecision {
        let steady_target = self
            .target_warm_replicas
            .clamp(self.min_warm_replicas, self.max_warm_replicas);
        let zero_denominator_guard = |value: usize| value.max(1);

        let (product_id, model_id, route_alias) = demand
            .map(|snapshot| {
                (
                    snapshot.product_id.clone(),
                    snapshot.model_id.clone(),
                    snapshot.route_alias.clone(),
                )
            })
            .unwrap_or_else(|| {
                (
                    String::from("psionic.unscoped"),
                    String::from("unknown"),
                    None,
                )
            });

        let (target_warm_replicas, reason, detail) = match demand {
            Some(snapshot) if !snapshot.is_expired_at(observed_at_ms) => {
                let request_target = snapshot.request_count.div_ceil(zero_denominator_guard(
                    self.requests_per_target_warm_replica,
                ));
                let active_target =
                    snapshot
                        .peak_selected_active_requests
                        .div_ceil(zero_denominator_guard(
                            self.active_requests_per_target_warm_replica,
                        ));
                let demanded_target = steady_target
                    .max(request_target.max(active_target))
                    .min(self.max_warm_replicas);
                if demanded_target > steady_target {
                    (
                        demanded_target,
                        ClusterReplicaDemandRebalanceReason::HotDemandScaleOut,
                        format!(
                            "fresh demand window recorded {} requests and peak {} active requests for model `{}`",
                            snapshot.request_count,
                            snapshot.peak_selected_active_requests,
                            snapshot.model_id
                        ),
                    )
                } else {
                    (
                        steady_target,
                        ClusterReplicaDemandRebalanceReason::SteadyState,
                        format!(
                            "fresh demand window stays within steady target {} for model `{}`",
                            steady_target, snapshot.model_id
                        ),
                    )
                }
            }
            Some(snapshot) => {
                let idle_ms = observed_at_ms.saturating_sub(snapshot.last_observed_at_ms);
                if current_warm_replicas > steady_target
                    && idle_ms >= self.unload_after_idle_seconds.saturating_mul(1_000)
                {
                    (
                        steady_target,
                        ClusterReplicaDemandRebalanceReason::IdleUnload,
                        format!(
                            "demand window for model `{}` expired at {} and remained idle for {} ms",
                            snapshot.model_id, snapshot.expires_at_ms, idle_ms
                        ),
                    )
                } else if current_warm_replicas > steady_target {
                    (
                        current_warm_replicas,
                        ClusterReplicaDemandRebalanceReason::IdleKeepalive,
                        format!(
                            "model `{}` is past fresh demand but still inside unload grace after {} ms of idleness",
                            snapshot.model_id, idle_ms
                        ),
                    )
                } else {
                    (
                        steady_target,
                        ClusterReplicaDemandRebalanceReason::SteadyState,
                        format!(
                            "expired demand for model `{}` does not require a warm-capacity change",
                            snapshot.model_id
                        ),
                    )
                }
            }
            None => {
                if current_warm_replicas > steady_target {
                    (
                        steady_target,
                        ClusterReplicaDemandRebalanceReason::IdleUnload,
                        format!(
                            "no fresh demand snapshot exists, so warm capacity returns to steady target {}",
                            steady_target
                        ),
                    )
                } else {
                    (
                        steady_target,
                        ClusterReplicaDemandRebalanceReason::SteadyState,
                        String::from("no demand snapshot exists for this model"),
                    )
                }
            }
        };

        ClusterReplicaDemandRebalanceDecision {
            product_id,
            model_id,
            route_alias,
            current_warm_replicas,
            target_warm_replicas,
            promote_replicas: target_warm_replicas.saturating_sub(current_warm_replicas),
            unload_replicas: current_warm_replicas.saturating_sub(target_warm_replicas),
            reason,
            detail,
        }
    }
}

impl Default for ClusterReplicaLifecyclePolicy {
    fn default() -> Self {
        Self::replicated_lane()
    }
}

/// Demand snapshot fed into the replica lifecycle policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterReplicaDemandSnapshot {
    /// Product surface publishing the demand.
    pub product_id: String,
    /// Stable model identifier.
    pub model_id: String,
    /// External alias used for the demand signal, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_alias: Option<String>,
    /// Requests observed in the current demand window.
    pub request_count: usize,
    /// Highest selected active-request count observed in the window.
    pub peak_selected_active_requests: usize,
    /// Most recent request observation in the window.
    pub last_observed_at_ms: u64,
    /// Time after which the demand window is stale.
    pub expires_at_ms: u64,
}

impl ClusterReplicaDemandSnapshot {
    /// Creates one lifecycle-consumable demand snapshot.
    #[must_use]
    pub fn new(
        product_id: impl Into<String>,
        model_id: impl Into<String>,
        route_alias: Option<String>,
        request_count: usize,
        peak_selected_active_requests: usize,
        last_observed_at_ms: u64,
        expires_at_ms: u64,
    ) -> Self {
        Self {
            product_id: product_id.into(),
            model_id: model_id.into(),
            route_alias,
            request_count,
            peak_selected_active_requests,
            last_observed_at_ms,
            expires_at_ms,
        }
    }

    /// Returns whether the demand window is stale at the observed time.
    #[must_use]
    pub const fn is_expired_at(&self, observed_at_ms: u64) -> bool {
        observed_at_ms > self.expires_at_ms
    }
}

/// Machine-checkable reason for one warm-capacity rebalance decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterReplicaDemandRebalanceReason {
    /// Current demand fits the existing steady target.
    SteadyState,
    /// Fresh demand requires extra warm capacity.
    HotDemandScaleOut,
    /// Demand expired, but replicas remain inside the configured keepalive window.
    IdleKeepalive,
    /// Demand expired and extra warm replicas should now be unloaded.
    IdleUnload,
}

/// One explicit warm-capacity rebalance decision for one model lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterReplicaDemandRebalanceDecision {
    /// Product surface publishing the demand.
    pub product_id: String,
    /// Stable model identifier the decision applies to.
    pub model_id: String,
    /// External alias used for the demand signal, when preserved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_alias: Option<String>,
    /// Current number of warm replicas observed for the model.
    pub current_warm_replicas: usize,
    /// Target number of warm replicas after the decision.
    pub target_warm_replicas: usize,
    /// Replicas that should be promoted or warmed to meet the target.
    pub promote_replicas: usize,
    /// Replicas that should be unloaded to return to the target.
    pub unload_replicas: usize,
    /// Explicit reason for the rebalance outcome.
    pub reason: ClusterReplicaDemandRebalanceReason,
    /// Plain-language detail for management surfaces.
    pub detail: String,
}

/// Stable failure code for per-lane active-host election reconciliation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterReplicaHostElectionFailureCode {
    /// Current ordered host-election record belongs to another lane.
    LaneMismatch,
    /// No warm replica exists to seed or maintain the active host.
    NoWarmReplicaHost,
    /// Active host needs promotion but no warm standby exists.
    NoWarmStandbyPromotionCandidate,
}

/// Machine-checkable failure returned when reconciling one lane host-election record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterReplicaHostElectionFailure {
    /// Stable failure code.
    pub code: ClusterReplicaHostElectionFailureCode,
    /// Replica lane that failed reconciliation.
    pub lane: ClusterReplicaLaneKey,
    /// Plain-language failure detail.
    pub detail: String,
    /// Current election term, when one was already recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_term: Option<ClusterTerm>,
    /// Current active host, when one was already recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_active_host_node_id: Option<NodeId>,
}

/// Reconciles one per-lane active-host election record from current warm replica truth.
pub fn reconcile_replica_host_election(
    replica_snapshot: &ClusterReplicaSnapshot,
    current_election: Option<&ClusterReplicaHostElectionRecord>,
    observed_tick: ClusterLeaseTick,
    lease_policy: ClusterLeadershipLeasePolicy,
) -> Result<ClusterReplicaHostElectionRecord, Box<ClusterReplicaHostElectionFailure>> {
    if let Some(current_election) = current_election
        && current_election.lane != replica_snapshot.lane
    {
        return Err(Box::new(ClusterReplicaHostElectionFailure {
            code: ClusterReplicaHostElectionFailureCode::LaneMismatch,
            lane: replica_snapshot.lane.clone(),
            detail: String::from("current host election belongs to another replica lane"),
            current_term: Some(current_election.term),
            current_active_host_node_id: Some(current_election.active_host_node_id.clone()),
        }));
    }

    let warm_replica_node_ids = replica_snapshot
        .replicas
        .values()
        .filter(|replica| replica.warm_state == ClusterReplicaWarmState::Warm)
        .map(|replica| replica.node_id.clone())
        .collect::<Vec<_>>();
    let Some(initial_active_host_node_id) = warm_replica_node_ids.first().cloned() else {
        return Err(Box::new(ClusterReplicaHostElectionFailure {
            code: ClusterReplicaHostElectionFailureCode::NoWarmReplicaHost,
            lane: replica_snapshot.lane.clone(),
            detail: String::from("replica lane has no warm node eligible to host traffic"),
            current_term: current_election.map(|election| election.term),
            current_active_host_node_id: current_election
                .map(|election| election.active_host_node_id.clone()),
        }));
    };

    let Some(current_election) = current_election else {
        return Ok(ClusterReplicaHostElectionRecord::new(
            replica_snapshot.lane.clone(),
            ClusterTerm::initial(),
            initial_active_host_node_id.clone(),
            ClusterReplicaHostElectionReason::InitialAssignment,
        )
        .with_standby_node_ids(
            warm_replica_node_ids
                .into_iter()
                .filter(|node_id| *node_id != initial_active_host_node_id)
                .collect(),
        )
        .with_lease_policy(observed_tick, lease_policy)
        .with_detail(format!(
            "assigned initial active host `{}` from current warm replica set",
            initial_active_host_node_id.as_str()
        )));
    };

    let current_replica = replica_snapshot
        .replicas
        .get(&current_election.active_host_node_id);
    let failover_reason = match current_replica {
        None => Some(ClusterReplicaHostElectionReason::ActiveHostMissing),
        Some(replica) => match replica.warm_state {
            ClusterReplicaWarmState::Warm => {
                match current_election.lease_status_at(observed_tick) {
                    ClusterLeadershipLeaseStatus::Active { .. } => None,
                    ClusterLeadershipLeaseStatus::Stale { .. }
                        if warm_replica_node_ids
                            .iter()
                            .any(|node_id| *node_id != current_election.active_host_node_id) =>
                    {
                        Some(ClusterReplicaHostElectionReason::LeaseExpired)
                    }
                    ClusterLeadershipLeaseStatus::Stale { .. } => None,
                }
            }
            ClusterReplicaWarmState::Draining => {
                Some(ClusterReplicaHostElectionReason::ActiveHostDraining)
            }
            ClusterReplicaWarmState::Refused => {
                Some(ClusterReplicaHostElectionReason::ActiveHostRefused)
            }
            ClusterReplicaWarmState::Cold | ClusterReplicaWarmState::Warming => {
                Some(ClusterReplicaHostElectionReason::ActiveHostNotWarm)
            }
        },
    };

    if let Some(failover_reason) = failover_reason {
        let promotion_candidate = warm_replica_node_ids
            .iter()
            .find(|node_id| **node_id != current_election.active_host_node_id)
            .cloned();
        let Some(next_active_host_node_id) = promotion_candidate else {
            return Err(Box::new(ClusterReplicaHostElectionFailure {
                code: ClusterReplicaHostElectionFailureCode::NoWarmStandbyPromotionCandidate,
                lane: replica_snapshot.lane.clone(),
                detail: format!(
                    "lane `{}` could not promote a standby away from active host `{}`",
                    replica_snapshot.lane.model_id,
                    current_election.active_host_node_id.as_str()
                ),
                current_term: Some(current_election.term),
                current_active_host_node_id: Some(current_election.active_host_node_id.clone()),
            }));
        };
        return Ok(ClusterReplicaHostElectionRecord::new(
            replica_snapshot.lane.clone(),
            current_election.term.next(),
            next_active_host_node_id.clone(),
            failover_reason,
        )
        .with_standby_node_ids(
            warm_replica_node_ids
                .into_iter()
                .filter(|node_id| *node_id != next_active_host_node_id)
                .collect(),
        )
        .with_promoted_from_node_id(current_election.active_host_node_id.clone())
        .with_lease_policy(observed_tick, lease_policy)
        .with_detail(format!(
            "promoted standby `{}` after `{}` stopped qualifying as active host",
            next_active_host_node_id.as_str(),
            current_election.active_host_node_id.as_str()
        )));
    }

    Ok(current_election
        .clone()
        .with_standby_node_ids(
            warm_replica_node_ids
                .into_iter()
                .filter(|node_id| *node_id != current_election.active_host_node_id)
                .collect(),
        )
        .renewed_at(observed_tick, lease_policy))
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
    /// Ordered host-election truth points at one active host that is not currently warm.
    ActiveHostUnavailable,
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

    let elected_host = state.replica_host_election(&replica_snapshot.lane);
    let routeable_replica_nodes = if let Some(elected_host) = elected_host {
        if !warm_replica_nodes.contains(&elected_host.active_host_node_id) {
            return Err(Box::new(ClusterReplicatedServingFailure {
                code: ClusterReplicatedServingFailureCode::ActiveHostUnavailable,
                detail: format!(
                    "lane `{}` elected active host `{}` in term {} but that host is not currently warm",
                    replica_snapshot.lane.model_id,
                    elected_host.active_host_node_id.as_str(),
                    elected_host.term.as_u64()
                ),
                lane: replica_snapshot.lane.clone(),
                replica_state_digest,
                lifecycle_policy_digest,
                serving_failure: None,
            }));
        }
        BTreeSet::from([elected_host.active_host_node_id.clone()])
    } else {
        warm_replica_nodes.clone()
    };

    let route_request =
        restricted_replica_scheduling_request(state, scheduling_request, &routeable_replica_nodes);
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
        elected_host,
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
        .with_serving_semantics(replica_routed_serving_semantics(serving_policy))
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

fn replica_routed_serving_semantics(
    serving_policy: &ClusterServingPolicy,
) -> ClusterServingSemantics {
    ClusterServingSemantics::new(
        ClusterExecutionLane::ReplicaRouted,
        serving_policy.execution_profile(),
        ClusterWarmRoutePosture::RoutePinned,
    )
    .with_detail(
        "replica-routed serving reused the canonical local execution-profile model while pinning truthful warm reuse to the same replica identity",
    )
}

fn build_replica_nodes(
    state: &ClusterState,
    load_snapshot: &ClusterServingLoadSnapshot,
    replica_snapshot: &ClusterReplicaSnapshot,
    selected_node_id: &NodeId,
    host_election: Option<&ClusterReplicaHostElectionRecord>,
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
                ClusterReplicaRoutingDisposition::Selected => host_election.map(|election| {
                    format!(
                        "active host for lane `{}` in term {} because {:?}",
                        election.lane.model_id,
                        election.term.as_u64(),
                        election.reason
                    )
                }),
                ClusterReplicaRoutingDisposition::WarmStandby => host_election
                    .filter(|election| election.standby_node_ids.contains(&replica.node_id))
                    .map(|election| {
                        format!(
                            "warm standby `{}` retained behind active host `{}` in term {}",
                            replica.node_id.as_str(),
                            election.active_host_node_id.as_str(),
                            election.term.as_u64()
                        )
                    })
                    .or_else(|| {
                        Some(format!(
                            "warm replica `{}` is available but not selected for this request",
                            replica.node_id.as_str()
                        ))
                    }),
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
        ClusterPolicyDigestKind, ClusterPrefillDecodeCapability, ClusterServingSemantics,
        ClusterWarmRoutePosture, ExecutionCapabilityProfile, KvResidencyTier,
        PrefillDecodeCapability,
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

    fn expert_replica_lane() -> ClusterReplicaLaneKey {
        ClusterReplicaLaneKey::new(
            "psionic.text_generation",
            "gemma4:26b",
            "cuda",
            "gemma4-26b-artifact",
        )
        .with_sharded_model_manifest_digest("gemma4-26b-manifest")
        .with_expert_topology_requirement(
            ClusterReplicaLaneExpertTopologyRequirement::new(
                "gemma4",
                "gemma4",
                64,
                ClusterReplicaLaneExpertRuntimeContract::FamilySpecificPlacement,
            )
            .with_active_expert_count(4)
            .with_expert_feed_forward_length(4096),
        )
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
            .with_prefill_decode_capability(ClusterPrefillDecodeCapability::new(
                ClusterExecutionLane::RemoteWholeRequest,
                PrefillDecodeCapability::colocated_split().with_detail(
                    "remote whole-request dispatch keeps prefill and decode split inside one selected replica runtime",
                ),
            ))
            .with_prefill_decode_capability(ClusterPrefillDecodeCapability::new(
                ClusterExecutionLane::ReplicaRouted,
                PrefillDecodeCapability::colocated_split().with_detail(
                    "replica-routed serving keeps prefill and decode split on the winning warm replica rather than moving KV across replicas",
                ),
            ))
            .with_serving_semantics_capability(
                ClusterServingSemantics::new(
                    ClusterExecutionLane::ReplicaRouted,
                    ExecutionCapabilityProfile::single_request_latency_optimized(),
                    ClusterWarmRoutePosture::RoutePinned,
                )
                .with_detail(
                    "replica-routed serving keeps canonical local single-request semantics while requiring the same warm replica identity for truthful reuse",
                ),
            )
            .with_clustered_cache_capability(
                ClusterCacheCapability::new(
                    ClusterExecutionLane::ReplicaRouted,
                    ClusterCacheScope::ReplicaLocal,
                    ClusterCacheScope::ReplicaLocal,
                )
                .with_residency_tiers(vec![KvResidencyTier::Host, KvResidencyTier::Device])
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

    fn scheduling_request_for_served_artifact(
        served_artifact_digest: &str,
    ) -> WholeRequestSchedulingRequest {
        WholeRequestSchedulingRequest::new(NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_replica_routed_capability_profile())
            .with_served_artifact_digest(served_artifact_digest)
            .requiring_accelerator()
    }

    fn scheduling_request() -> WholeRequestSchedulingRequest {
        scheduling_request_for_served_artifact("artifact-1")
    }

    fn replica_host_lease_policy() -> ClusterLeadershipLeasePolicy {
        ClusterLeadershipLeasePolicy::new(4)
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
        assert_eq!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .serving_semantics
                .as_ref()
                .map(|semantics| semantics.warm_route_posture),
            Some(ClusterWarmRoutePosture::RoutePinned)
        );
        Ok(())
    }

    #[test]
    fn replicated_serving_keeps_cuda_gemma4_lane_identity_machine_legible()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = replica_state().snapshot();
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(NodeId::new("worker-a"), "gemma4-e4b-artifact"),
            ClusterArtifactResidencyRecord::new(
                NodeId::new("worker-a"),
                ClusterArtifactReference::new("decoder", "gemma4-e4b-artifact"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        snapshot.artifact_residency.insert(
            crate::ClusterArtifactResidencyKey::new(NodeId::new("worker-b"), "gemma4-e4b-artifact"),
            ClusterArtifactResidencyRecord::new(
                NodeId::new("worker-b"),
                ClusterArtifactReference::new("decoder", "gemma4-e4b-artifact"),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
        let state = ClusterState::from_snapshot(snapshot);
        let lane = ClusterReplicaLaneKey::new(
            "psionic.openai_compat",
            "gemma4:e4b",
            "cuda",
            "gemma4-e4b-artifact",
        );
        let replica_snapshot =
            ClusterReplicaSnapshot::new(state.cluster_id().clone(), lane.clone())
                .with_replica(ClusterReplicaRecord::new(
                    lane.clone(),
                    NodeId::new("worker-a"),
                    ClusterReplicaWarmState::Warm,
                ))
                .with_replica(ClusterReplicaRecord::new(
                    lane.clone(),
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
            &ClusterServingRequest::new("req-gemma4-replica-1", ClusterServingWorkClass::Decode),
            &scheduling_request_for_served_artifact("gemma4-e4b-artifact"),
        )
        .map_err(|err| {
            fixture_error(&format!(
                "gemma4 replicated serving should keep lane identity intact: {err:?}"
            ))
        })?;

        assert_eq!(decision.lane, lane);
        assert_eq!(decision.lane.model_id, "gemma4:e4b");
        assert_eq!(decision.lane.runtime_backend, "cuda");
        assert_eq!(decision.lane.served_artifact_digest, "gemma4-e4b-artifact");
        assert_eq!(
            decision.replica_state_digest,
            replica_snapshot.stable_digest()
        );
        assert_eq!(
            decision.serving_decision.schedule.selected_node_id,
            NodeId::new("worker-a")
        );
        assert_eq!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .disposition,
            ClusterExecutionDisposition::ReplicaRouted
        );
        Ok(())
    }

    #[test]
    fn replicated_serving_snapshot_digest_tracks_expert_topology_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let cluster_id = sample_cluster_id();
        let dense_snapshot =
            ClusterReplicaSnapshot::new(cluster_id.clone(), replica_lane_with_manifest())
                .with_replica(ClusterReplicaRecord::new(
                    replica_lane_with_manifest(),
                    NodeId::new("worker-a"),
                    ClusterReplicaWarmState::Warm,
                ));
        let expert_lane = expert_replica_lane();
        let expert_snapshot = ClusterReplicaSnapshot::new(cluster_id, expert_lane.clone())
            .with_replica(ClusterReplicaRecord::new(
                expert_lane.clone(),
                NodeId::new("worker-a"),
                ClusterReplicaWarmState::Warm,
            ));

        assert_ne!(
            dense_snapshot.stable_digest(),
            expert_snapshot.stable_digest()
        );
        let requirement = expert_snapshot
            .lane
            .expert_topology_requirement
            .as_ref()
            .expect("expert topology requirement");
        assert_eq!(requirement.family, "gemma4");
        assert_eq!(requirement.expert_count, 64);
        assert_eq!(requirement.active_expert_count, Some(4));
        assert_eq!(
            requirement.runtime_contract,
            ClusterReplicaLaneExpertRuntimeContract::FamilySpecificPlacement
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
        assert_eq!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .serving_semantics
                .as_ref()
                .map(|semantics| semantics.execution_profile.queue_policy.discipline),
            Some(psionic_runtime::QueueDiscipline::DirectCallerBackpressure)
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
    fn reconcile_replica_host_election_promotes_warm_standby_with_next_term_and_reason()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = replica_state();
        let initial_snapshot =
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

        let initial_election = reconcile_replica_host_election(
            &initial_snapshot,
            None,
            ClusterLeaseTick::new(10),
            replica_host_lease_policy(),
        )
        .map_err(|err| fixture_error(&format!("initial host election should succeed: {err:?}")))?;
        assert_eq!(initial_election.term, ClusterTerm::initial());
        assert_eq!(
            initial_election.active_host_node_id,
            NodeId::new("worker-a")
        );
        assert_eq!(
            initial_election.reason,
            ClusterReplicaHostElectionReason::InitialAssignment
        );

        let draining_snapshot =
            ClusterReplicaSnapshot::new(state.cluster_id().clone(), replica_lane())
                .with_replica(
                    ClusterReplicaRecord::new(
                        replica_lane(),
                        NodeId::new("worker-a"),
                        ClusterReplicaWarmState::Draining,
                    )
                    .with_detail("operator requested drain"),
                )
                .with_replica(ClusterReplicaRecord::new(
                    replica_lane(),
                    NodeId::new("worker-b"),
                    ClusterReplicaWarmState::Warm,
                ));

        let promoted_election = reconcile_replica_host_election(
            &draining_snapshot,
            Some(&initial_election),
            ClusterLeaseTick::new(11),
            replica_host_lease_policy(),
        )
        .map_err(|err| fixture_error(&format!("standby promotion should succeed: {err:?}")))?;

        assert_eq!(promoted_election.term, ClusterTerm::initial().next());
        assert_eq!(
            promoted_election.active_host_node_id,
            NodeId::new("worker-b")
        );
        assert_eq!(
            promoted_election.promoted_from_node_id,
            Some(NodeId::new("worker-a"))
        );
        assert_eq!(
            promoted_election.reason,
            ClusterReplicaHostElectionReason::ActiveHostDraining
        );
        assert!(promoted_election.standby_node_ids.is_empty());
        Ok(())
    }

    #[test]
    fn replica_lifecycle_policy_scales_target_warm_replicas_for_hot_demand() {
        let policy = ClusterReplicaLifecyclePolicy::replicated_lane();
        let demand = ClusterReplicaDemandSnapshot::new(
            "psionic.openai_compat",
            "tiny-llama",
            Some(String::from("chat-default")),
            9,
            5,
            10_000,
            70_000,
        );

        let decision = policy.rebalance_for_demand(Some(&demand), 1, 12_000);

        assert_eq!(decision.product_id, "psionic.openai_compat");
        assert_eq!(decision.model_id, "tiny-llama");
        assert_eq!(decision.target_warm_replicas, 3);
        assert_eq!(decision.promote_replicas, 2);
        assert_eq!(decision.unload_replicas, 0);
        assert_eq!(
            decision.reason,
            ClusterReplicaDemandRebalanceReason::HotDemandScaleOut
        );
    }

    #[test]
    fn replica_lifecycle_policy_unloads_after_demand_stays_idle_past_threshold() {
        let policy = ClusterReplicaLifecyclePolicy::replicated_lane();
        let demand = ClusterReplicaDemandSnapshot::new(
            "psionic.openai_compat",
            "tiny-llama",
            None,
            4,
            2,
            1_000,
            10_000,
        );

        let decision = policy.rebalance_for_demand(Some(&demand), 4, 400_000);

        assert_eq!(decision.target_warm_replicas, 2);
        assert_eq!(decision.promote_replicas, 0);
        assert_eq!(decision.unload_replicas, 2);
        assert_eq!(
            decision.reason,
            ClusterReplicaDemandRebalanceReason::IdleUnload
        );
        assert!(decision.detail.contains("expired"));
    }

    #[test]
    fn replicated_serving_respects_ordered_active_host_election()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = replica_state().snapshot();
        let lane = replica_lane();
        snapshot.replica_host_elections.insert(
            lane.clone(),
            ClusterReplicaHostElectionRecord::new(
                lane.clone(),
                ClusterTerm::initial(),
                NodeId::new("worker-b"),
                ClusterReplicaHostElectionReason::InitialAssignment,
            )
            .with_standby_node_ids(vec![NodeId::new("worker-a")])
            .with_lease_policy(ClusterLeaseTick::new(12), replica_host_lease_policy())
            .with_detail("worker-b is the elected active host"),
        );
        let state = ClusterState::from_snapshot(snapshot);
        let replica_snapshot =
            ClusterReplicaSnapshot::new(state.cluster_id().clone(), lane.clone())
                .with_replica(ClusterReplicaRecord::new(
                    lane.clone(),
                    NodeId::new("worker-a"),
                    ClusterReplicaWarmState::Warm,
                ))
                .with_replica(ClusterReplicaRecord::new(
                    lane,
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
            &ClusterServingRequest::new("req-replica-election-1", ClusterServingWorkClass::Decode),
            &scheduling_request(),
        )
        .map_err(|err| {
            fixture_error(&format!(
                "ordered host-election route should succeed: {err:?}"
            ))
        })?;

        assert_eq!(
            decision.serving_decision.schedule.selected_node_id,
            NodeId::new("worker-b")
        );
        assert!(
            decision
                .serving_decision
                .schedule
                .cluster_execution
                .replica_nodes
                .iter()
                .any(|replica| {
                    replica.node.node_id == "worker-b"
                        && replica.routing == ClusterReplicaRoutingDisposition::Selected
                        && replica
                            .detail
                            .as_deref()
                            .is_some_and(|detail| detail.contains("active host for lane"))
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
                        && replica
                            .detail
                            .as_deref()
                            .is_some_and(|detail| detail.contains("warm standby `worker-a`"))
                })
        );
        Ok(())
    }

    #[test]
    fn replicated_serving_refuses_when_elected_active_host_is_not_warm()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut snapshot = replica_state().snapshot();
        let lane = replica_lane();
        snapshot.replica_host_elections.insert(
            lane.clone(),
            ClusterReplicaHostElectionRecord::new(
                lane.clone(),
                ClusterTerm::initial(),
                NodeId::new("worker-c"),
                ClusterReplicaHostElectionReason::LeaseExpired,
            )
            .with_standby_node_ids(vec![NodeId::new("worker-a"), NodeId::new("worker-b")])
            .with_lease_policy(ClusterLeaseTick::new(12), replica_host_lease_policy())
            .with_detail("worker-c was previously active but is no longer warm"),
        );
        let state = ClusterState::from_snapshot(snapshot);
        let replica_snapshot =
            ClusterReplicaSnapshot::new(state.cluster_id().clone(), lane.clone())
                .with_replica(ClusterReplicaRecord::new(
                    lane.clone(),
                    NodeId::new("worker-a"),
                    ClusterReplicaWarmState::Warm,
                ))
                .with_replica(ClusterReplicaRecord::new(
                    lane,
                    NodeId::new("worker-b"),
                    ClusterReplicaWarmState::Warm,
                ));
        let load_snapshot = ClusterServingLoadSnapshot::new(state.cluster_id().clone())
            .with_node_load(crate::ClusterNodeServiceLoad::new(NodeId::new("worker-a")))
            .with_node_load(crate::ClusterNodeServiceLoad::new(NodeId::new("worker-b")));

        let failure = match plan_replicated_serving(
            &state,
            &load_snapshot,
            &replica_snapshot,
            &ClusterReplicaLifecyclePolicy::replicated_lane(),
            &ClusterServingPolicy::direct_caller_latency_first(),
            &ClusterServingRequest::new("req-replica-election-2", ClusterServingWorkClass::Decode),
            &scheduling_request(),
        ) {
            Ok(decision) => {
                return Err(fixture_error(&format!(
                    "expected active-host-unavailable failure, got {decision:?}"
                ))
                .into());
            }
            Err(failure) => failure,
        };

        assert_eq!(
            failure.code,
            ClusterReplicatedServingFailureCode::ActiveHostUnavailable
        );
        assert!(failure.detail.contains("worker-c"));
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
