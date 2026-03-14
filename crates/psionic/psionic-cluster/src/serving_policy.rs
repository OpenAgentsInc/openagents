use std::collections::BTreeMap;

use psionic_runtime::{
    BatchExecutionPosture, ClusterExecutionContext, ClusterExecutionLane, ClusterFallbackReason,
    ClusterFallbackStep, ClusterPolicyDigest, ClusterPolicyDigestKind, ClusterServingSemantics,
    ClusterWarmRoutePosture, ExecutionCapabilityProfile, QueueDiscipline, QueuePolicy,
    ThroughputClass,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ClusterId, ClusterState, NodeId, WholeRequestClusterSchedule, WholeRequestSchedulingFailure,
    WholeRequestSchedulingRequest, schedule_remote_whole_request,
};

/// Cluster-serving work class used for fairness and backpressure policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterServingWorkClass {
    /// Prompt or graph-prefill work.
    Prefill,
    /// Iterative token decode work.
    Decode,
}

/// How cluster serving reacts when a selected node is under pressure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterBackpressureDisposition {
    /// Admit the request into the selected node's queue.
    QueueLocally,
    /// Exclude the pressured node and retry selection elsewhere.
    Reroute,
    /// Refuse the request immediately.
    Refuse,
}

/// High-level service-health posture for one cluster node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterNodeServiceHealth {
    /// Node service lane is healthy enough for ordinary admission.
    Healthy,
    /// Node service lane is slow and should be treated as degraded.
    Slow,
    /// Node service lane is degraded enough that new work should be avoided.
    Degraded,
}

/// Explicit cancellation propagation policy for clustered serving.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterCancellationPolicy {
    /// Abort generation after the current token or step completes.
    AbortAfterCurrentToken,
}

/// Explicit cluster serving policy for queueing, fairness, and backpressure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterServingPolicy {
    /// Queue discipline used before execution starts.
    pub queue_discipline: QueueDiscipline,
    /// Maximum concurrently active requests permitted on one selected node.
    pub max_active_requests_per_node: usize,
    /// Maximum queued requests permitted on one selected node.
    pub max_queued_requests_per_node: usize,
    /// Number of decode-active requests that force new prefill to yield.
    pub decode_reserve_per_node: usize,
    /// Policy used when the selected node is already at active-request capacity.
    pub overload_backpressure: ClusterBackpressureDisposition,
    /// Policy used when the selected node is marked slow or degraded.
    pub slow_node_backpressure: ClusterBackpressureDisposition,
    /// Explicit cancellation propagation policy for the clustered lane.
    pub cancellation: ClusterCancellationPolicy,
}

impl ClusterServingPolicy {
    /// Direct-caller latency-first policy with no internal queue.
    #[must_use]
    pub const fn direct_caller_latency_first() -> Self {
        Self {
            queue_discipline: QueueDiscipline::DirectCallerBackpressure,
            max_active_requests_per_node: 1,
            max_queued_requests_per_node: 0,
            decode_reserve_per_node: 1,
            overload_backpressure: ClusterBackpressureDisposition::Reroute,
            slow_node_backpressure: ClusterBackpressureDisposition::Reroute,
            cancellation: ClusterCancellationPolicy::AbortAfterCurrentToken,
        }
    }

    /// FIFO policy that allows bounded queueing while preserving decode fairness.
    #[must_use]
    pub const fn fifo_balanced() -> Self {
        Self {
            queue_discipline: QueueDiscipline::Fifo,
            max_active_requests_per_node: 1,
            max_queued_requests_per_node: 4,
            decode_reserve_per_node: 1,
            overload_backpressure: ClusterBackpressureDisposition::QueueLocally,
            slow_node_backpressure: ClusterBackpressureDisposition::Reroute,
            cancellation: ClusterCancellationPolicy::AbortAfterCurrentToken,
        }
    }

    /// Returns the canonical runtime queue policy for this clustered serving policy.
    #[must_use]
    pub const fn queue_policy(&self) -> QueuePolicy {
        QueuePolicy {
            discipline: self.queue_discipline,
            max_active_requests: self.max_active_requests_per_node,
            max_queued_requests: self.max_queued_requests_per_node,
            per_model_serialization: true,
        }
    }

    /// Returns the canonical execution profile implied by this clustered serving policy.
    #[must_use]
    pub const fn execution_profile(&self) -> ExecutionCapabilityProfile {
        ExecutionCapabilityProfile {
            batch_posture: if self.max_active_requests_per_node > 1 {
                BatchExecutionPosture::SchedulerStaticBatch
            } else {
                BatchExecutionPosture::SingleRequestOnly
            },
            queue_policy: self.queue_policy(),
            throughput_class: match self.queue_discipline {
                QueueDiscipline::DirectCallerBackpressure => ThroughputClass::LatencyOptimized,
                QueueDiscipline::Fifo => ThroughputClass::Balanced,
            },
            prefill_decode_capability: None,
        }
    }

    /// Returns a stable digest for the serving policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(match self.queue_discipline {
            QueueDiscipline::DirectCallerBackpressure => b"direct_caller_backpressure".as_slice(),
            QueueDiscipline::Fifo => b"fifo".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(self.max_active_requests_per_node.to_string());
        hasher.update(b"|");
        hasher.update(self.max_queued_requests_per_node.to_string());
        hasher.update(b"|");
        hasher.update(self.decode_reserve_per_node.to_string());
        hasher.update(b"|");
        hasher.update(backpressure_disposition_label(self.overload_backpressure));
        hasher.update(b"|");
        hasher.update(backpressure_disposition_label(self.slow_node_backpressure));
        hasher.update(b"|");
        hasher.update(match self.cancellation {
            ClusterCancellationPolicy::AbortAfterCurrentToken => {
                b"abort_after_current_token".as_slice()
            }
        });
        hex::encode(hasher.finalize())
    }
}

impl Default for ClusterServingPolicy {
    fn default() -> Self {
        Self::direct_caller_latency_first()
    }
}

/// Explicit load and queue snapshot for one cluster node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterNodeServiceLoad {
    /// Node these service-load facts describe.
    pub node_id: NodeId,
    /// Requests currently active on the node.
    pub active_requests: usize,
    /// Requests currently queued behind active work on the node.
    pub queued_requests: usize,
    /// Active prefill-class requests on the node.
    pub prefill_active_requests: usize,
    /// Active decode-class requests on the node.
    pub decode_active_requests: usize,
    /// Explicit queue capacity surfaced by the node, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_capacity: Option<usize>,
    /// High-level service-health posture.
    pub service_health: ClusterNodeServiceHealth,
}

impl ClusterNodeServiceLoad {
    /// Creates an empty service-load snapshot for one node.
    #[must_use]
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            active_requests: 0,
            queued_requests: 0,
            prefill_active_requests: 0,
            decode_active_requests: 0,
            queue_capacity: None,
            service_health: ClusterNodeServiceHealth::Healthy,
        }
    }

    /// Attaches active-request count.
    #[must_use]
    pub const fn with_active_requests(mut self, active_requests: usize) -> Self {
        self.active_requests = active_requests;
        self
    }

    /// Attaches queue depth and optional queue capacity.
    #[must_use]
    pub const fn with_queue(
        mut self,
        queued_requests: usize,
        queue_capacity: Option<usize>,
    ) -> Self {
        self.queued_requests = queued_requests;
        self.queue_capacity = queue_capacity;
        self
    }

    /// Attaches the active class breakdown for the node.
    #[must_use]
    pub const fn with_class_load(
        mut self,
        prefill_active_requests: usize,
        decode_active_requests: usize,
    ) -> Self {
        self.prefill_active_requests = prefill_active_requests;
        self.decode_active_requests = decode_active_requests;
        self
    }

    /// Attaches an explicit service-health posture.
    #[must_use]
    pub const fn with_service_health(mut self, service_health: ClusterNodeServiceHealth) -> Self {
        self.service_health = service_health;
        self
    }
}

/// Replayable cluster-serving load snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterServingLoadSnapshot {
    /// Cluster identity these load facts belong to.
    pub cluster_id: ClusterId,
    /// Node load facts by node ID.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub nodes: BTreeMap<NodeId, ClusterNodeServiceLoad>,
}

impl ClusterServingLoadSnapshot {
    /// Creates an empty serving-load snapshot for one cluster.
    #[must_use]
    pub fn new(cluster_id: ClusterId) -> Self {
        Self {
            cluster_id,
            nodes: BTreeMap::new(),
        }
    }

    /// Inserts or replaces one node service-load snapshot.
    #[must_use]
    pub fn with_node_load(mut self, node_load: ClusterNodeServiceLoad) -> Self {
        self.nodes.insert(node_load.node_id.clone(), node_load);
        self
    }

    /// Returns a stable digest of the node load facts used for serving policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(b"|serving_load|");
        for node_load in self.nodes.values() {
            hasher.update(node_load.node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(node_load.active_requests.to_string());
            hasher.update(b"|");
            hasher.update(node_load.queued_requests.to_string());
            hasher.update(b"|");
            hasher.update(node_load.prefill_active_requests.to_string());
            hasher.update(b"|");
            hasher.update(node_load.decode_active_requests.to_string());
            hasher.update(b"|");
            hasher.update(
                node_load
                    .queue_capacity
                    .map_or(String::new(), |capacity| capacity.to_string()),
            );
            hasher.update(b"|");
            hasher.update(service_health_label(node_load.service_health));
        }
        hex::encode(hasher.finalize())
    }
}

/// Explicit clustered serving request for one schedule attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterServingRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Work class that drives fairness policy.
    pub work_class: ClusterServingWorkClass,
    /// Whether the request was cancelled before admission completed.
    pub cancellation_requested: bool,
}

impl ClusterServingRequest {
    /// Creates a clustered serving request from request ID and work class.
    #[must_use]
    pub fn new(request_id: impl Into<String>, work_class: ClusterServingWorkClass) -> Self {
        Self {
            request_id: request_id.into(),
            work_class,
            cancellation_requested: false,
        }
    }

    /// Marks the request as cancelled before admission completed.
    #[must_use]
    pub const fn cancelled(mut self) -> Self {
        self.cancellation_requested = true;
        self
    }
}

/// Stable note code for one cluster serving policy outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterServingDecisionCode {
    /// Request can execute immediately on the selected node.
    ExecuteImmediately,
    /// Request was admitted into the selected node's queue.
    QueuedByBackpressure,
    /// Prefill was forced to yield because decode work already occupied the node.
    PrefillYieldedToDecode,
    /// Request rerouted away from a saturated node.
    ReroutedForQueueBackpressure,
    /// Request rerouted away from a slow node.
    ReroutedOffSlowNode,
    /// Cancellation propagated before clustered admission completed.
    CancellationPropagated,
    /// Required load snapshot was missing.
    LoadSnapshotMissing,
    /// Request was refused under explicit serving policy.
    BackpressureRefused,
}

/// One explicit note explaining a serving-policy outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterServingDecisionNote {
    /// Stable note code.
    pub code: ClusterServingDecisionCode,
    /// Plain-language note detail.
    pub detail: String,
}

impl ClusterServingDecisionNote {
    fn new(code: ClusterServingDecisionCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

/// Final serving disposition for the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterServingDecisionDisposition {
    /// Request should execute immediately on the selected node.
    ExecuteNow,
    /// Request should wait in the selected node's queue.
    Queue,
}

/// Successful serving-policy decision for one clustered request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterServingDecision {
    /// Stable request identifier.
    pub request_id: String,
    /// Work class used for fairness policy.
    pub work_class: ClusterServingWorkClass,
    /// Final serving disposition.
    pub disposition: ClusterServingDecisionDisposition,
    /// Effective serving-policy digest used for the decision.
    pub policy_digest: String,
    /// Effective load-snapshot digest used for the decision.
    pub load_digest: String,
    /// Queue position when the request was queued.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_position: Option<usize>,
    /// Final clustered execution schedule chosen for the request.
    pub schedule: WholeRequestClusterSchedule,
    /// Explicit notes explaining the outcome.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<ClusterServingDecisionNote>,
}

/// Stable failure code for cluster serving policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterServingFailureCode {
    /// Request was cancelled before clustered admission completed.
    CancelledBeforeAdmission,
    /// The supplied load snapshot belongs to another cluster.
    LoadSnapshotClusterMismatch,
    /// The selected node had no explicit load snapshot.
    LoadSnapshotMissing,
    /// Base remote scheduling failed before serving policy could act.
    SchedulingFailed,
    /// All candidate nodes were exhausted by queue, fairness, or slow-node policy.
    NoServingCapacity,
}

/// Machine-checkable serving-policy failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterServingFailure {
    /// Stable failure code.
    pub code: ClusterServingFailureCode,
    /// Plain-language failure detail.
    pub detail: String,
    /// Cluster identity used for the failed decision.
    pub cluster_id: ClusterId,
    /// Stable request identifier.
    pub request_id: String,
    /// Work class used for the decision.
    pub work_class: ClusterServingWorkClass,
    /// Effective serving-policy digest used for the failure.
    pub policy_digest: String,
    /// Effective load-snapshot digest used for the failure.
    pub load_digest: String,
    /// Underlying whole-request scheduling failure, when one existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_failure: Option<Box<WholeRequestSchedulingFailure>>,
    /// Explicit notes explaining the failure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<ClusterServingDecisionNote>,
}

/// Plans cluster serving policy on top of truthful whole-request scheduling.
pub fn plan_cluster_serving_admission(
    state: &ClusterState,
    load_snapshot: &ClusterServingLoadSnapshot,
    policy: &ClusterServingPolicy,
    serving_request: &ClusterServingRequest,
    scheduling_request: &WholeRequestSchedulingRequest,
) -> Result<ClusterServingDecision, ClusterServingFailure> {
    let policy_digest = policy.stable_digest();
    let load_digest = load_snapshot.stable_digest();
    if load_snapshot.cluster_id != *state.cluster_id() {
        return Err(ClusterServingFailure {
            code: ClusterServingFailureCode::LoadSnapshotClusterMismatch,
            detail: format!(
                "serving load snapshot belongs to cluster `{}` but scheduling state belongs to `{}`",
                load_snapshot.cluster_id.as_str(),
                state.cluster_id().as_str()
            ),
            cluster_id: state.cluster_id().clone(),
            request_id: serving_request.request_id.clone(),
            work_class: serving_request.work_class,
            policy_digest,
            load_digest,
            scheduler_failure: None,
            notes: Vec::new(),
        });
    }
    if serving_request.cancellation_requested {
        return Err(ClusterServingFailure {
            code: ClusterServingFailureCode::CancelledBeforeAdmission,
            detail: format!(
                "request `{}` was cancelled before clustered admission completed",
                serving_request.request_id
            ),
            cluster_id: state.cluster_id().clone(),
            request_id: serving_request.request_id.clone(),
            work_class: serving_request.work_class,
            policy_digest,
            load_digest,
            scheduler_failure: None,
            notes: vec![ClusterServingDecisionNote::new(
                ClusterServingDecisionCode::CancellationPropagated,
                format!(
                    "cluster serving propagated cancellation for request `{}` under `{}`",
                    serving_request.request_id,
                    cancellation_label(policy.cancellation)
                ),
            )],
        });
    }

    let mut excluded_node_ids = scheduling_request.excluded_node_ids.clone();
    let mut notes = Vec::new();
    let mut fallback_history = Vec::new();
    let mut pending_reroute: Option<PendingReroute> = None;

    loop {
        let schedule_attempt = schedule_remote_whole_request(
            state,
            &scheduling_request
                .clone()
                .excluding_nodes(excluded_node_ids.iter().cloned()),
        );
        let schedule = match schedule_attempt {
            Ok(schedule) => schedule,
            Err(scheduler_failure) => {
                if let Some(reroute) = pending_reroute.take() {
                    notes.push(ClusterServingDecisionNote::new(
                        reroute.note_code,
                        reroute.detail,
                    ));
                }
                let code = if notes.is_empty() {
                    ClusterServingFailureCode::SchedulingFailed
                } else {
                    ClusterServingFailureCode::NoServingCapacity
                };
                let detail = if notes.is_empty() {
                    format!(
                        "whole-request scheduling failed before serving policy could admit request `{}`",
                        serving_request.request_id
                    )
                } else {
                    format!(
                        "all cluster-serving candidates were exhausted for request `{}`",
                        serving_request.request_id
                    )
                };
                return Err(ClusterServingFailure {
                    code,
                    detail,
                    cluster_id: state.cluster_id().clone(),
                    request_id: serving_request.request_id.clone(),
                    work_class: serving_request.work_class,
                    policy_digest,
                    load_digest,
                    scheduler_failure: Some(scheduler_failure),
                    notes,
                });
            }
        };

        if let Some(reroute) = pending_reroute.take() {
            fallback_history.push(
                ClusterFallbackStep::new(schedule.selected_node_id.as_str(), reroute.reason)
                    .from_node(reroute.from_node_id.as_str())
                    .with_detail(reroute.detail.clone()),
            );
            notes.push(ClusterServingDecisionNote::new(
                reroute.note_code,
                reroute.detail,
            ));
        }

        let Some(node_load) = load_snapshot.nodes.get(&schedule.selected_node_id) else {
            return Err(ClusterServingFailure {
                code: ClusterServingFailureCode::LoadSnapshotMissing,
                detail: format!(
                    "selected node `{}` has no explicit serving-load snapshot",
                    schedule.selected_node_id.as_str()
                ),
                cluster_id: state.cluster_id().clone(),
                request_id: serving_request.request_id.clone(),
                work_class: serving_request.work_class,
                policy_digest,
                load_digest,
                scheduler_failure: None,
                notes: vec![ClusterServingDecisionNote::new(
                    ClusterServingDecisionCode::LoadSnapshotMissing,
                    format!(
                        "serving policy cannot admit node `{}` without a replayable load snapshot",
                        schedule.selected_node_id.as_str()
                    ),
                )],
            });
        };

        if node_load.service_health != ClusterNodeServiceHealth::Healthy {
            if let Some(queue_position) = queue_position(policy, node_load)
                && policy.slow_node_backpressure == ClusterBackpressureDisposition::QueueLocally
            {
                let detail = format!(
                    "queued request `{}` on slow node `{}` at position {queue_position}",
                    serving_request.request_id,
                    schedule.selected_node_id.as_str()
                );
                let mut notes = notes;
                notes.push(ClusterServingDecisionNote::new(
                    ClusterServingDecisionCode::QueuedByBackpressure,
                    detail.clone(),
                ));
                let schedule = finalize_schedule(
                    schedule,
                    policy,
                    &policy_digest,
                    &fallback_history,
                    Some(&detail),
                );
                return Ok(ClusterServingDecision {
                    request_id: serving_request.request_id.clone(),
                    work_class: serving_request.work_class,
                    disposition: ClusterServingDecisionDisposition::Queue,
                    policy_digest,
                    load_digest,
                    queue_position: Some(queue_position),
                    schedule,
                    notes,
                });
            }

            match policy.slow_node_backpressure {
                ClusterBackpressureDisposition::QueueLocally => {
                    return Err(policy_refusal(
                        state.cluster_id().clone(),
                        serving_request,
                        &policy_digest,
                        &load_digest,
                        notes,
                        format!(
                            "selected node `{}` is `{}` and its queue is unavailable under the active serving policy",
                            schedule.selected_node_id.as_str(),
                            service_health_label(node_load.service_health)
                        ),
                    ));
                }
                ClusterBackpressureDisposition::Reroute => {
                    excluded_node_ids.insert(schedule.selected_node_id.clone());
                    pending_reroute = Some(PendingReroute {
                        from_node_id: schedule.selected_node_id.clone(),
                        reason: ClusterFallbackReason::SlowNodeBackpressure,
                        note_code: ClusterServingDecisionCode::ReroutedOffSlowNode,
                        detail: format!(
                            "rerouted request `{}` away from `{}` because service health was `{}`",
                            serving_request.request_id,
                            schedule.selected_node_id.as_str(),
                            service_health_label(node_load.service_health)
                        ),
                    });
                    continue;
                }
                ClusterBackpressureDisposition::Refuse => {
                    return Err(policy_refusal(
                        state.cluster_id().clone(),
                        serving_request,
                        &policy_digest,
                        &load_digest,
                        notes,
                        format!(
                            "serving policy refused slow selected node `{}` with health `{}`",
                            schedule.selected_node_id.as_str(),
                            service_health_label(node_load.service_health)
                        ),
                    ));
                }
            }
        }

        if serving_request.work_class == ClusterServingWorkClass::Prefill
            && policy.decode_reserve_per_node > 0
            && node_load.decode_active_requests >= policy.decode_reserve_per_node
        {
            if let Some(queue_position) = queue_position(policy, node_load) {
                let detail = format!(
                    "queued prefill request `{}` behind decode-active work on node `{}` at position {queue_position}",
                    serving_request.request_id,
                    schedule.selected_node_id.as_str()
                );
                let mut notes = notes;
                notes.push(ClusterServingDecisionNote::new(
                    ClusterServingDecisionCode::PrefillYieldedToDecode,
                    detail.clone(),
                ));
                let schedule = finalize_schedule(
                    schedule,
                    policy,
                    &policy_digest,
                    &fallback_history,
                    Some(&detail),
                );
                return Ok(ClusterServingDecision {
                    request_id: serving_request.request_id.clone(),
                    work_class: serving_request.work_class,
                    disposition: ClusterServingDecisionDisposition::Queue,
                    policy_digest,
                    load_digest,
                    queue_position: Some(queue_position),
                    schedule,
                    notes,
                });
            }

            match policy.overload_backpressure {
                ClusterBackpressureDisposition::QueueLocally => {
                    return Err(policy_refusal(
                        state.cluster_id().clone(),
                        serving_request,
                        &policy_digest,
                        &load_digest,
                        notes,
                        format!(
                            "prefill request `{}` yielded to decode on node `{}` but queue capacity was unavailable",
                            serving_request.request_id,
                            schedule.selected_node_id.as_str()
                        ),
                    ));
                }
                ClusterBackpressureDisposition::Reroute => {
                    excluded_node_ids.insert(schedule.selected_node_id.clone());
                    pending_reroute = Some(PendingReroute {
                        from_node_id: schedule.selected_node_id.clone(),
                        reason: ClusterFallbackReason::DecodeFairness,
                        note_code: ClusterServingDecisionCode::PrefillYieldedToDecode,
                        detail: format!(
                            "rerouted prefill request `{}` away from `{}` to preserve decode fairness",
                            serving_request.request_id,
                            schedule.selected_node_id.as_str()
                        ),
                    });
                    continue;
                }
                ClusterBackpressureDisposition::Refuse => {
                    return Err(policy_refusal(
                        state.cluster_id().clone(),
                        serving_request,
                        &policy_digest,
                        &load_digest,
                        notes,
                        format!(
                            "serving policy refused prefill request `{}` because decode fairness reserved node `{}`",
                            serving_request.request_id,
                            schedule.selected_node_id.as_str()
                        ),
                    ));
                }
            }
        }

        if node_load.active_requests >= policy.max_active_requests_per_node {
            if let Some(queue_position) = queue_position(policy, node_load)
                && policy.overload_backpressure == ClusterBackpressureDisposition::QueueLocally
            {
                let detail = format!(
                    "queued request `{}` on saturated node `{}` at position {queue_position}",
                    serving_request.request_id,
                    schedule.selected_node_id.as_str()
                );
                let mut notes = notes;
                notes.push(ClusterServingDecisionNote::new(
                    ClusterServingDecisionCode::QueuedByBackpressure,
                    detail.clone(),
                ));
                let schedule = finalize_schedule(
                    schedule,
                    policy,
                    &policy_digest,
                    &fallback_history,
                    Some(&detail),
                );
                return Ok(ClusterServingDecision {
                    request_id: serving_request.request_id.clone(),
                    work_class: serving_request.work_class,
                    disposition: ClusterServingDecisionDisposition::Queue,
                    policy_digest,
                    load_digest,
                    queue_position: Some(queue_position),
                    schedule,
                    notes,
                });
            }

            match policy.overload_backpressure {
                ClusterBackpressureDisposition::QueueLocally => {
                    return Err(policy_refusal(
                        state.cluster_id().clone(),
                        serving_request,
                        &policy_digest,
                        &load_digest,
                        notes,
                        format!(
                            "selected node `{}` is saturated and its queue is unavailable under the active serving policy",
                            schedule.selected_node_id.as_str()
                        ),
                    ));
                }
                ClusterBackpressureDisposition::Reroute => {
                    excluded_node_ids.insert(schedule.selected_node_id.clone());
                    pending_reroute = Some(PendingReroute {
                        from_node_id: schedule.selected_node_id.clone(),
                        reason: ClusterFallbackReason::QueueBackpressure,
                        note_code: ClusterServingDecisionCode::ReroutedForQueueBackpressure,
                        detail: format!(
                            "rerouted request `{}` away from saturated node `{}`",
                            serving_request.request_id,
                            schedule.selected_node_id.as_str()
                        ),
                    });
                    continue;
                }
                ClusterBackpressureDisposition::Refuse => {
                    return Err(policy_refusal(
                        state.cluster_id().clone(),
                        serving_request,
                        &policy_digest,
                        &load_digest,
                        notes,
                        format!(
                            "serving policy refused saturated selected node `{}`",
                            schedule.selected_node_id.as_str()
                        ),
                    ));
                }
            }
        }

        let mut notes = notes;
        notes.push(ClusterServingDecisionNote::new(
            ClusterServingDecisionCode::ExecuteImmediately,
            format!(
                "request `{}` can execute immediately on node `{}`",
                serving_request.request_id,
                schedule.selected_node_id.as_str()
            ),
        ));
        let schedule = finalize_schedule(schedule, policy, &policy_digest, &fallback_history, None);
        return Ok(ClusterServingDecision {
            request_id: serving_request.request_id.clone(),
            work_class: serving_request.work_class,
            disposition: ClusterServingDecisionDisposition::ExecuteNow,
            policy_digest,
            load_digest,
            queue_position: None,
            schedule,
            notes,
        });
    }
}

struct PendingReroute {
    from_node_id: NodeId,
    reason: ClusterFallbackReason,
    note_code: ClusterServingDecisionCode,
    detail: String,
}

fn finalize_schedule(
    mut schedule: WholeRequestClusterSchedule,
    policy: &ClusterServingPolicy,
    policy_digest: &str,
    fallback_history: &[ClusterFallbackStep],
    additional_degraded_reason: Option<&str>,
) -> WholeRequestClusterSchedule {
    schedule
        .cluster_execution
        .policy_digests
        .push(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Serving,
            policy_digest.to_owned(),
        ));
    schedule
        .cluster_execution
        .fallback_history
        .extend(fallback_history.iter().cloned());
    schedule.cluster_execution = schedule
        .cluster_execution
        .with_serving_semantics(
            ClusterServingSemantics::new(
                ClusterExecutionLane::RemoteWholeRequest,
                policy.execution_profile(),
                ClusterWarmRoutePosture::ReadyNodeSelection,
            )
            .with_detail(
                "cluster serving reused the canonical local execution-profile model for whole-request remote admission",
            ),
        );
    if let Some(additional_degraded_reason) = additional_degraded_reason {
        append_degraded_reason(&mut schedule.cluster_execution, additional_degraded_reason);
    }
    schedule
}

fn append_degraded_reason(
    cluster_execution: &mut ClusterExecutionContext,
    additional_degraded_reason: &str,
) {
    if let Some(existing) = &mut cluster_execution.degraded_reason {
        existing.push_str("; ");
        existing.push_str(additional_degraded_reason);
    } else {
        cluster_execution.degraded_reason = Some(additional_degraded_reason.to_owned());
    }
}

fn queue_position(
    policy: &ClusterServingPolicy,
    node_load: &ClusterNodeServiceLoad,
) -> Option<usize> {
    if policy.queue_discipline != QueueDiscipline::Fifo {
        return None;
    }
    let effective_capacity = node_load
        .queue_capacity
        .unwrap_or(policy.max_queued_requests_per_node)
        .min(policy.max_queued_requests_per_node);
    if node_load.queued_requests < effective_capacity {
        Some(node_load.queued_requests + 1)
    } else {
        None
    }
}

fn policy_refusal(
    cluster_id: ClusterId,
    serving_request: &ClusterServingRequest,
    policy_digest: &str,
    load_digest: &str,
    mut notes: Vec<ClusterServingDecisionNote>,
    detail: String,
) -> ClusterServingFailure {
    notes.push(ClusterServingDecisionNote::new(
        ClusterServingDecisionCode::BackpressureRefused,
        detail.clone(),
    ));
    ClusterServingFailure {
        code: ClusterServingFailureCode::NoServingCapacity,
        detail,
        cluster_id,
        request_id: serving_request.request_id.clone(),
        work_class: serving_request.work_class,
        policy_digest: policy_digest.to_owned(),
        load_digest: load_digest.to_owned(),
        scheduler_failure: None,
        notes,
    }
}

const fn backpressure_disposition_label(
    disposition: ClusterBackpressureDisposition,
) -> &'static [u8] {
    match disposition {
        ClusterBackpressureDisposition::QueueLocally => b"queue_locally",
        ClusterBackpressureDisposition::Reroute => b"reroute",
        ClusterBackpressureDisposition::Refuse => b"refuse",
    }
}

const fn cancellation_label(cancellation: ClusterCancellationPolicy) -> &'static str {
    match cancellation {
        ClusterCancellationPolicy::AbortAfterCurrentToken => "abort_after_current_token",
    }
}

const fn service_health_label(service_health: ClusterNodeServiceHealth) -> &'static str {
    match service_health {
        ClusterNodeServiceHealth::Healthy => "healthy",
        ClusterNodeServiceHealth::Slow => "slow",
        ClusterNodeServiceHealth::Degraded => "degraded",
    }
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use std::io::Error;

    use psionic_runtime::{
        ClusterArtifactResidencyDisposition, ClusterExecutionCapabilityProfile,
        ClusterExecutionLane, ClusterPrefillDecodeCapability, PrefillDecodeCapability,
    };

    use crate::{
        AdmissionToken, ClusterBackendReadinessStatus, ClusterLink, ClusterLinkStatus,
        ClusterMembershipRecord, ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity,
        ClusterNodeTelemetry, ClusterSnapshot, ClusterTransportClass, NodeEpoch, NodeRole,
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

    fn ready_membership(
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

    fn healthy_link(left: &str, right: &str) -> ClusterLink {
        ClusterLink::new(
            NodeId::new(left),
            NodeId::new(right),
            ClusterTransportClass::LanUdp,
            ClusterLinkStatus::Healthy,
        )
    }

    fn ready_cuda_telemetry(node_id: &str, free_memory_bytes: u64) -> ClusterNodeTelemetry {
        ClusterNodeTelemetry::new(NodeId::new(node_id))
            .with_memory(Some(64 * 1024 * 1024 * 1024), Some(free_memory_bytes))
            .with_cpu_logical_cores(16)
            .with_accelerator_count(1)
            .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready)
    }

    fn cuda_remote_dispatch_capability_profile() -> ClusterExecutionCapabilityProfile {
        ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![ClusterExecutionLane::RemoteWholeRequest])
            .with_prefill_decode_capability(ClusterPrefillDecodeCapability::new(
                ClusterExecutionLane::RemoteWholeRequest,
                PrefillDecodeCapability::colocated_split().with_detail(
                    "remote whole-request serving keeps prefill and decode split inside the selected runtime owner",
                ),
            ))
            .with_serving_semantics_capability(
                ClusterServingSemantics::new(
                    ClusterExecutionLane::RemoteWholeRequest,
                    ExecutionCapabilityProfile::single_request_latency_optimized(),
                    ClusterWarmRoutePosture::ReadyNodeSelection,
                )
                .with_detail(
                    "remote whole-request serving keeps canonical local single-request semantics while only requiring selection of one ready node",
                ),
            )
            .with_detail(
                "backend `cuda` declares whole-request remote dispatch on ready cluster nodes",
            )
    }

    fn single_worker_state() -> ClusterState {
        let cluster_id = sample_cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships.insert(
            NodeId::new("scheduler"),
            ready_membership(&cluster_id, "scheduler", NodeRole::Mixed),
        );
        snapshot.memberships.insert(
            NodeId::new("worker-a"),
            ready_membership(&cluster_id, "worker-a", NodeRole::ExecutorOnly),
        );
        snapshot.telemetry.insert(
            NodeId::new("worker-a"),
            ready_cuda_telemetry("worker-a", 32 * 1024 * 1024 * 1024),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(NodeId::new("scheduler"), NodeId::new("worker-a")),
            healthy_link("scheduler", "worker-a")
                .with_latency_us(400)
                .with_bandwidth_mbps(1000),
        );
        ClusterState::from_snapshot(snapshot)
    }

    fn two_worker_state() -> ClusterState {
        let cluster_id = sample_cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships.insert(
            NodeId::new("scheduler"),
            ready_membership(&cluster_id, "scheduler", NodeRole::Mixed),
        );
        snapshot.memberships.insert(
            NodeId::new("worker-a"),
            ready_membership(&cluster_id, "worker-a", NodeRole::ExecutorOnly),
        );
        snapshot.memberships.insert(
            NodeId::new("worker-b"),
            ready_membership(&cluster_id, "worker-b", NodeRole::ExecutorOnly),
        );
        snapshot.telemetry.insert(
            NodeId::new("worker-a"),
            ready_cuda_telemetry("worker-a", 32 * 1024 * 1024 * 1024),
        );
        snapshot.telemetry.insert(
            NodeId::new("worker-b"),
            ready_cuda_telemetry("worker-b", 16 * 1024 * 1024 * 1024),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(NodeId::new("scheduler"), NodeId::new("worker-a")),
            healthy_link("scheduler", "worker-a")
                .with_latency_us(300)
                .with_bandwidth_mbps(1000),
        );
        snapshot.links.insert(
            crate::ClusterLinkKey::new(NodeId::new("scheduler"), NodeId::new("worker-b")),
            healthy_link("scheduler", "worker-b")
                .with_latency_us(900)
                .with_bandwidth_mbps(800),
        );
        ClusterState::from_snapshot(snapshot)
    }

    fn scheduling_request() -> WholeRequestSchedulingRequest {
        WholeRequestSchedulingRequest::new(NodeId::new("scheduler"), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile())
    }

    #[test]
    fn prefill_yields_to_decode_and_queues_explicitly() -> Result<(), Box<dyn std::error::Error>> {
        let state = single_worker_state();
        let load_snapshot = ClusterServingLoadSnapshot::new(state.cluster_id().clone())
            .with_node_load(
                ClusterNodeServiceLoad::new(NodeId::new("worker-a"))
                    .with_active_requests(1)
                    .with_queue(0, Some(4))
                    .with_class_load(0, 1),
            );

        let decision = plan_cluster_serving_admission(
            &state,
            &load_snapshot,
            &ClusterServingPolicy::fifo_balanced(),
            &ClusterServingRequest::new("req-prefill", ClusterServingWorkClass::Prefill),
            &scheduling_request(),
        )
        .map_err(|err| fixture_error(&format!("prefill should queue: {err:?}")))?;

        assert_eq!(
            decision.disposition,
            ClusterServingDecisionDisposition::Queue
        );
        assert_eq!(decision.queue_position, Some(1));
        assert_eq!(decision.schedule.selected_node_id, NodeId::new("worker-a"));
        assert!(
            decision
                .notes
                .iter()
                .any(|note| { note.code == ClusterServingDecisionCode::PrefillYieldedToDecode })
        );
        assert!(
            decision
                .schedule
                .cluster_execution
                .policy_digests
                .iter()
                .any(|digest| { digest.kind == ClusterPolicyDigestKind::Serving })
        );
        assert_eq!(
            decision
                .schedule
                .cluster_execution
                .selected_nodes
                .first()
                .and_then(|node| node.artifact_residency),
            Some(ClusterArtifactResidencyDisposition::Resident)
        );
        assert_eq!(
            decision
                .schedule
                .cluster_execution
                .serving_semantics
                .as_ref()
                .map(|semantics| semantics.execution_profile.queue_policy.discipline),
            Some(QueueDiscipline::Fifo)
        );
        Ok(())
    }

    #[test]
    fn slow_node_backpressure_reroutes_and_records_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = two_worker_state();
        let load_snapshot = ClusterServingLoadSnapshot::new(state.cluster_id().clone())
            .with_node_load(
                ClusterNodeServiceLoad::new(NodeId::new("worker-a"))
                    .with_service_health(ClusterNodeServiceHealth::Slow),
            )
            .with_node_load(ClusterNodeServiceLoad::new(NodeId::new("worker-b")));

        let decision = plan_cluster_serving_admission(
            &state,
            &load_snapshot,
            &ClusterServingPolicy::direct_caller_latency_first(),
            &ClusterServingRequest::new("req-decode", ClusterServingWorkClass::Decode),
            &scheduling_request(),
        )
        .map_err(|err| fixture_error(&format!("slow primary should reroute: {err:?}")))?;

        assert_eq!(
            decision.disposition,
            ClusterServingDecisionDisposition::ExecuteNow
        );
        assert_eq!(decision.schedule.selected_node_id, NodeId::new("worker-b"));
        assert!(
            decision
                .notes
                .iter()
                .any(|note| { note.code == ClusterServingDecisionCode::ReroutedOffSlowNode })
        );
        assert!(
            decision
                .schedule
                .cluster_execution
                .fallback_history
                .iter()
                .any(|step| {
                    step.from_node_id.as_deref() == Some("worker-a")
                        && step.to_node_id == "worker-b"
                        && step.reason == ClusterFallbackReason::SlowNodeBackpressure
                })
        );
        assert_eq!(
            decision
                .schedule
                .cluster_execution
                .serving_semantics
                .as_ref()
                .map(|semantics| semantics.warm_route_posture),
            Some(ClusterWarmRoutePosture::ReadyNodeSelection)
        );
        Ok(())
    }

    #[test]
    fn cancellation_is_machine_checkable_before_admission() -> Result<(), Box<dyn std::error::Error>>
    {
        let state = single_worker_state();
        let load_snapshot = ClusterServingLoadSnapshot::new(state.cluster_id().clone())
            .with_node_load(ClusterNodeServiceLoad::new(NodeId::new("worker-a")));

        let failure = match plan_cluster_serving_admission(
            &state,
            &load_snapshot,
            &ClusterServingPolicy::direct_caller_latency_first(),
            &ClusterServingRequest::new("req-cancelled", ClusterServingWorkClass::Decode)
                .cancelled(),
            &scheduling_request(),
        ) {
            Ok(decision) => {
                return Err(fixture_error(&format!(
                    "expected cancellation failure, got {decision:?}"
                ))
                .into());
            }
            Err(failure) => failure,
        };

        assert_eq!(
            failure.code,
            ClusterServingFailureCode::CancelledBeforeAdmission
        );
        assert!(
            failure
                .notes
                .iter()
                .any(|note| { note.code == ClusterServingDecisionCode::CancellationPropagated })
        );
        Ok(())
    }

    #[test]
    fn saturated_cluster_returns_explicit_backpressure_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = single_worker_state();
        let load_snapshot = ClusterServingLoadSnapshot::new(state.cluster_id().clone())
            .with_node_load(
                ClusterNodeServiceLoad::new(NodeId::new("worker-a"))
                    .with_active_requests(1)
                    .with_queue(0, Some(0))
                    .with_class_load(0, 0),
            );

        let failure = match plan_cluster_serving_admission(
            &state,
            &load_snapshot,
            &ClusterServingPolicy::direct_caller_latency_first(),
            &ClusterServingRequest::new("req-overloaded", ClusterServingWorkClass::Decode),
            &scheduling_request(),
        ) {
            Ok(decision) => {
                return Err(fixture_error(&format!(
                    "expected backpressure failure, got {decision:?}"
                ))
                .into());
            }
            Err(failure) => failure,
        };

        assert_eq!(failure.code, ClusterServingFailureCode::NoServingCapacity);
        assert!(
            failure.notes.iter().any(|note| {
                note.code == ClusterServingDecisionCode::ReroutedForQueueBackpressure
            })
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
}
