//! Elastic device-mesh and benchmark-gated collective planning for Psionic.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

use std::collections::BTreeSet;

use psionic_runtime::{
    ClusterCommunicationClass, ClusterShardHandoff, ClusterShardHandoffKind, ClusterTransportClass,
    RuntimeWorkClass, RuntimeWorkItem, TrainingCollectiveContext, TrainingCollectiveKind,
    TrainingCollectiveQuantization, TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind,
    TrainingDeviceMeshContext, TrainingElasticMembershipContext,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "elastic device-mesh and collective planning substrate";

/// One concrete worker participating in a device mesh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveMeshMember {
    /// Stable node identifier.
    pub node_id: String,
    /// Stable rank inside the mesh.
    pub rank: usize,
    /// Stable shard index or replica slot.
    pub shard_id: usize,
    /// Plain-language device label.
    pub device_label: String,
}

impl CollectiveMeshMember {
    /// Creates one mesh member from explicit node and rank facts.
    #[must_use]
    pub fn new(
        node_id: impl Into<String>,
        rank: usize,
        shard_id: usize,
        device_label: impl Into<String>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            rank,
            shard_id,
            device_label: device_label.into(),
        }
    }
}

/// One measured benchmark sample for a collective path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedCollectiveBenchmarkSample {
    /// Total duration in microseconds.
    pub duration_us: u64,
    /// Total wire bytes transferred.
    pub wire_bytes: u64,
    /// Maximum relative numerical error in basis points.
    pub max_relative_error_bps: u64,
}

impl QuantizedCollectiveBenchmarkSample {
    /// Creates one benchmark sample.
    #[must_use]
    pub const fn new(duration_us: u64, wire_bytes: u64, max_relative_error_bps: u64) -> Self {
        Self {
            duration_us,
            wire_bytes,
            max_relative_error_bps,
        }
    }
}

/// Benchmark comparison for one quantized collective path versus the unquantized baseline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedCollectiveBenchmark {
    /// Collective kind measured by the benchmark.
    pub kind: TrainingCollectiveKind,
    /// Quantized mode compared against the baseline.
    pub quantization: TrainingCollectiveQuantization,
    /// Baseline no-quantization sample.
    pub baseline: QuantizedCollectiveBenchmarkSample,
    /// Quantized sample.
    pub quantized: QuantizedCollectiveBenchmarkSample,
    /// Relative speedup in basis points versus baseline.
    pub speedup_bps: u64,
    /// Whether the quantized path is accepted for planning.
    pub accepted: bool,
    /// Stable digest covering the benchmark inputs and conclusion.
    pub benchmark_digest: String,
}

impl QuantizedCollectiveBenchmark {
    /// Creates one benchmark verdict from explicit baseline and quantized samples.
    #[must_use]
    pub fn new(
        kind: TrainingCollectiveKind,
        quantization: TrainingCollectiveQuantization,
        baseline: QuantizedCollectiveBenchmarkSample,
        quantized: QuantizedCollectiveBenchmarkSample,
        maximum_error_bps: u64,
        minimum_speedup_bps: u64,
    ) -> Self {
        let speedup_bps = benchmark_speedup_bps(baseline.duration_us, quantized.duration_us);
        let accepted = quantization != TrainingCollectiveQuantization::None
            && quantized.max_relative_error_bps <= maximum_error_bps
            && speedup_bps >= minimum_speedup_bps
            && quantized.wire_bytes < baseline.wire_bytes;
        let benchmark_digest = stable_benchmark_digest(
            kind,
            quantization,
            baseline,
            quantized,
            maximum_error_bps,
            minimum_speedup_bps,
            accepted,
            speedup_bps,
        );
        Self {
            kind,
            quantization,
            baseline,
            quantized,
            speedup_bps,
            accepted,
            benchmark_digest,
        }
    }
}

/// Result of observing one elastic device mesh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticDeviceMeshObservation {
    /// Runtime-visible mesh posture.
    pub mesh: TrainingDeviceMeshContext,
    /// Whether the observation changed mesh revision.
    pub changed: bool,
}

/// Planned collective execution over one elastic device mesh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveExecutionPlan {
    /// Runtime-visible collective posture.
    pub collective: TrainingCollectiveContext,
    /// Explicit shard handoffs representing the collective ring.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handoffs: Vec<ClusterShardHandoff>,
    /// Low-level runtime work item describing the collective step.
    pub work_item: RuntimeWorkItem,
}

/// Scoped sync stage inside a broader collective-cadence plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectiveSyncScope {
    /// Sync only one local subgroup of the wider mesh.
    LocalGroup,
    /// Sync the full realized mesh.
    GlobalMesh,
}

/// Mesh-wide transport feedback used by collective replanning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveTransportFeedback {
    /// Logical observation timestamp.
    pub observed_at_ms: u64,
    /// Estimated effective bandwidth available to collective traffic.
    pub estimated_bandwidth_mbps: u64,
    /// Estimated end-to-end latency for collective traffic.
    pub estimated_latency_ms: u64,
    /// Active logical streams contending for transport capacity.
    pub active_streams: u16,
    /// Stable digest over the feedback payload.
    pub feedback_digest: String,
    /// Plain-language detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl CollectiveTransportFeedback {
    /// Creates one mesh-wide transport observation for cadence planning.
    #[must_use]
    pub fn new(
        observed_at_ms: u64,
        estimated_bandwidth_mbps: u64,
        estimated_latency_ms: u64,
        active_streams: u16,
    ) -> Self {
        let feedback_digest = stable_transport_feedback_digest(
            observed_at_ms,
            estimated_bandwidth_mbps,
            estimated_latency_ms,
            active_streams,
        );
        Self {
            observed_at_ms,
            estimated_bandwidth_mbps,
            estimated_latency_ms,
            active_streams,
            feedback_digest,
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

/// Policy controlling local-versus-global sync cadence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveSyncCadencePolicy {
    /// Global sync interval in healthy transport conditions.
    pub global_interval_steps: u64,
    /// Global sync interval once transport degrades and a local fallback exists.
    pub degraded_global_interval_steps: u64,
    /// Minimum healthy bandwidth before the planner degrades cadence.
    pub bandwidth_floor_mbps: u64,
    /// Maximum healthy latency before the planner degrades cadence.
    pub latency_ceiling_ms: u64,
    /// Maximum healthy active-stream count before the planner replans.
    pub active_stream_ceiling: u16,
    /// Quantization used for local subgroup sync stages.
    pub local_quantization: TrainingCollectiveQuantization,
    /// Fallback quantization used when requested global quantization is not approved.
    pub fallback_global_quantization: TrainingCollectiveQuantization,
}

impl Default for CollectiveSyncCadencePolicy {
    fn default() -> Self {
        Self {
            global_interval_steps: 1,
            degraded_global_interval_steps: 4,
            bandwidth_floor_mbps: 800,
            latency_ceiling_ms: 8,
            active_stream_ceiling: 8,
            local_quantization: TrainingCollectiveQuantization::None,
            fallback_global_quantization: TrainingCollectiveQuantization::None,
        }
    }
}

impl CollectiveSyncCadencePolicy {
    /// Creates the default cadence policy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides the healthy global interval.
    #[must_use]
    pub fn with_global_interval_steps(mut self, global_interval_steps: u64) -> Self {
        self.global_interval_steps = global_interval_steps.max(1);
        self
    }

    /// Overrides the degraded global interval.
    #[must_use]
    pub fn with_degraded_global_interval_steps(
        mut self,
        degraded_global_interval_steps: u64,
    ) -> Self {
        self.degraded_global_interval_steps = degraded_global_interval_steps.max(1);
        self
    }

    /// Overrides the transport thresholds that trigger degraded cadence.
    #[must_use]
    pub const fn with_transport_thresholds(
        mut self,
        bandwidth_floor_mbps: u64,
        latency_ceiling_ms: u64,
        active_stream_ceiling: u16,
    ) -> Self {
        self.bandwidth_floor_mbps = bandwidth_floor_mbps;
        self.latency_ceiling_ms = latency_ceiling_ms;
        self.active_stream_ceiling = active_stream_ceiling;
        self
    }

    /// Overrides the local subgroup quantization mode.
    #[must_use]
    pub const fn with_local_quantization(
        mut self,
        local_quantization: TrainingCollectiveQuantization,
    ) -> Self {
        self.local_quantization = local_quantization;
        self
    }

    /// Overrides the fallback global quantization mode.
    #[must_use]
    pub const fn with_fallback_global_quantization(
        mut self,
        fallback_global_quantization: TrainingCollectiveQuantization,
    ) -> Self {
        self.fallback_global_quantization = fallback_global_quantization;
        self
    }
}

/// Replan reasons emitted by the cadence planner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectiveReplanTriggerKind {
    /// Mesh truth changed since the previous cadence receipt.
    MeshRevisionChanged,
    /// Observed bandwidth fell below policy floor.
    BandwidthBelowFloor,
    /// Observed latency rose above policy ceiling.
    LatencyAboveCeiling,
    /// Transport contention exceeded the policy ceiling.
    ActiveStreamsAboveCeiling,
    /// The cadence interval has elapsed and a global sync is due.
    GlobalIntervalElapsed,
    /// Requested quantization lacked an approved benchmark and was downgraded.
    QuantizationApprovalMissing,
}

/// One typed reason the planner changed or justified sync cadence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveReplanTrigger {
    /// Trigger kind.
    pub kind: CollectiveReplanTriggerKind,
    /// Machine-legible detail.
    pub detail: String,
}

/// High-level cadence class selected for one training step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectiveSyncCadenceClass {
    /// Run a full-mesh sync on this step.
    EveryStepGlobal,
    /// Run local subgroup sync only and defer the next global sync.
    LocalOnlyDeferredGlobal,
    /// Run local subgroup sync first, then a full-mesh sync.
    LocalThenGlobal,
    /// Fall back to global-only sync because no local subgroup exists.
    GlobalOnlyFallback,
}

/// Receipt describing why one sync cadence was chosen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveSyncCadenceReceipt {
    /// Step index this cadence applies to.
    pub step_index: u64,
    /// Mesh revision this cadence was derived from.
    pub mesh_revision: u64,
    /// Selected cadence class.
    pub cadence_class: CollectiveSyncCadenceClass,
    /// Global sync interval realized by the plan.
    pub global_interval_steps: u64,
    /// Next step at which a global sync should occur.
    pub next_global_step: u64,
    /// Local subgroup size used when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_group_size: Option<usize>,
    /// Global quantization realized by the plan.
    pub global_quantization: TrainingCollectiveQuantization,
    /// Local subgroup quantization realized by the plan.
    pub local_quantization: TrainingCollectiveQuantization,
    /// Whether transport was considered degraded under current policy.
    pub degraded_transport: bool,
    /// Triggers that caused or justified replanning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<CollectiveReplanTrigger>,
    /// Mesh-wide transport feedback used by the plan when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_feedback: Option<CollectiveTransportFeedback>,
    /// Stable digest over the cadence receipt.
    pub receipt_digest: String,
}

impl CollectiveSyncCadenceReceipt {
    /// Creates one cadence receipt from explicit sync-planning facts.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        step_index: u64,
        mesh_revision: u64,
        cadence_class: CollectiveSyncCadenceClass,
        global_interval_steps: u64,
        next_global_step: u64,
        local_group_size: Option<usize>,
        global_quantization: TrainingCollectiveQuantization,
        local_quantization: TrainingCollectiveQuantization,
        degraded_transport: bool,
        triggers: Vec<CollectiveReplanTrigger>,
        transport_feedback: Option<CollectiveTransportFeedback>,
    ) -> Self {
        let receipt_digest = stable_cadence_receipt_digest(
            step_index,
            mesh_revision,
            cadence_class,
            global_interval_steps,
            next_global_step,
            local_group_size,
            global_quantization,
            local_quantization,
            degraded_transport,
            triggers.as_slice(),
            transport_feedback.as_ref(),
        );
        Self {
            step_index,
            mesh_revision,
            cadence_class,
            global_interval_steps,
            next_global_step,
            local_group_size,
            global_quantization,
            local_quantization,
            degraded_transport,
            triggers,
            transport_feedback,
            receipt_digest,
        }
    }
}

/// One sync stage inside the broader cadence plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveSyncStage {
    /// Sync scope for this stage.
    pub scope: CollectiveSyncScope,
    /// Stable group index when this is a local subgroup stage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_index: Option<usize>,
    /// Explicit participants in this stage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub member_node_ids: Vec<String>,
    /// Planned collective execution for this stage.
    pub plan: CollectiveExecutionPlan,
}

/// Full sync plan for one trainer step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveSyncExecutionPlan {
    /// Cadence receipt for the selected plan.
    pub cadence_receipt: CollectiveSyncCadenceReceipt,
    /// Ordered sync stages to execute.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<CollectiveSyncStage>,
}

/// Errors returned by elastic collective planning.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CollectivePlanningError {
    /// The mesh axes do not multiply to the declared member count.
    #[error("device mesh axis product {axis_product} does not match member count {member_count}")]
    AxisProductMismatch {
        /// Product of all axis extents.
        axis_product: usize,
        /// Number of declared mesh members.
        member_count: usize,
    },
    /// One mesh member is not present in the active membership set.
    #[error("mesh member `{node_id}` is not present in the active membership set")]
    MeshMemberNotActive {
        /// Stable node identifier.
        node_id: String,
    },
    /// Collective planning requires a mesh observation first.
    #[error("collective planning requires an observed device mesh")]
    MeshNotObserved,
    /// The requested quantization has no accepted benchmark.
    #[error("quantized collective `{quantization:?}` is not benchmark-approved for `{kind:?}`")]
    QuantizationNotApproved {
        /// Collective kind.
        kind: TrainingCollectiveKind,
        /// Requested quantization mode.
        quantization: TrainingCollectiveQuantization,
    },
}

/// Stateful elastic collective planner that tracks mesh revisions and benchmark-approved quantization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticCollectivePlanner {
    /// Stable mesh identifier.
    pub mesh_id: String,
    /// Effective backend running the mesh.
    pub effective_backend: String,
    /// Communication class required by the mesh.
    pub communication_class: ClusterCommunicationClass,
    /// Explicit axes realized by the mesh.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<TrainingDeviceMeshAxis>,
    /// Transport used for collective ring handoffs.
    pub transport: ClusterTransportClass,
    /// Latest observed mesh, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_mesh: Option<TrainingDeviceMeshContext>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    members: Vec<CollectiveMeshMember>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    benchmarks: Vec<QuantizedCollectiveBenchmark>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_transport_feedback: Option<CollectiveTransportFeedback>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_sync_receipt: Option<CollectiveSyncCadenceReceipt>,
}

impl ElasticCollectivePlanner {
    /// Creates a planner for one elastic device mesh.
    #[must_use]
    pub fn new(
        mesh_id: impl Into<String>,
        effective_backend: impl Into<String>,
        communication_class: ClusterCommunicationClass,
        axes: Vec<TrainingDeviceMeshAxis>,
    ) -> Self {
        Self {
            mesh_id: mesh_id.into(),
            effective_backend: effective_backend.into(),
            communication_class,
            axes,
            transport: ClusterTransportClass::TrustedLanStream,
            latest_mesh: None,
            members: Vec::new(),
            benchmarks: Vec::new(),
            latest_transport_feedback: None,
            latest_sync_receipt: None,
        }
    }

    /// Overrides the transport used for collective handoffs.
    #[must_use]
    pub const fn with_transport(mut self, transport: ClusterTransportClass) -> Self {
        self.transport = transport;
        self
    }

    /// Records or replaces one benchmark verdict for a collective path.
    pub fn record_benchmark(&mut self, benchmark: QuantizedCollectiveBenchmark) {
        self.benchmarks.retain(|existing| {
            !(existing.kind == benchmark.kind && existing.quantization == benchmark.quantization)
        });
        self.benchmarks.push(benchmark);
    }

    /// Records the latest mesh-wide transport feedback used by sync replanning.
    pub fn observe_transport_feedback(&mut self, feedback: CollectiveTransportFeedback) {
        self.latest_transport_feedback = Some(feedback);
    }

    /// Observes one elastic device mesh and advances revision when truth changes.
    pub fn observe_mesh(
        &mut self,
        elastic_membership: TrainingElasticMembershipContext,
        members: Vec<CollectiveMeshMember>,
    ) -> Result<ElasticDeviceMeshObservation, CollectivePlanningError> {
        validate_mesh_axes(&self.axes, members.len())?;
        let active_node_ids = elastic_membership
            .active_node_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for member in &members {
            if !active_node_ids.contains(&member.node_id) {
                return Err(CollectivePlanningError::MeshMemberNotActive {
                    node_id: member.node_id.clone(),
                });
            }
        }
        let member_node_ids = sorted_member_node_ids(&members);
        let digest = stable_mesh_digest(
            self.mesh_id.as_str(),
            self.effective_backend.as_str(),
            self.communication_class,
            &self.axes,
            &elastic_membership,
            &member_node_ids,
        );
        let next_revision = self.latest_mesh.as_ref().map_or(1, |previous| {
            if stable_mesh_digest(
                previous.mesh_id.as_str(),
                previous.effective_backend.as_str(),
                previous.communication_class,
                &previous.axes,
                &previous.elastic_membership,
                &previous.member_node_ids,
            ) == digest
            {
                previous.mesh_revision
            } else {
                previous.mesh_revision.saturating_add(1)
            }
        });
        let mesh = TrainingDeviceMeshContext::new(
            self.mesh_id.clone(),
            next_revision,
            self.effective_backend.clone(),
            self.communication_class,
            elastic_membership,
            member_node_ids,
        )
        .with_axes(self.axes.clone());
        let changed = self
            .latest_mesh
            .as_ref()
            .is_none_or(|previous| previous != &mesh);
        self.members = members;
        self.latest_mesh = Some(mesh.clone());
        Ok(ElasticDeviceMeshObservation { mesh, changed })
    }

    /// Plans one collective over the latest mesh, using benchmark-approved quantization only.
    pub fn plan_collective(
        &self,
        kind: TrainingCollectiveKind,
        payload_bytes: u64,
        requested_quantization: TrainingCollectiveQuantization,
    ) -> Result<CollectiveExecutionPlan, CollectivePlanningError> {
        let mesh = self
            .latest_mesh
            .clone()
            .ok_or(CollectivePlanningError::MeshNotObserved)?;
        let benchmark = self.approved_benchmark(kind, requested_quantization)?;
        Ok(build_collective_plan(
            mesh,
            &self.members,
            kind,
            payload_bytes,
            requested_quantization,
            benchmark,
            self.transport,
            CollectiveSyncScope::GlobalMesh,
            None,
        ))
    }

    /// Plans one trainer-step sync cadence over local groups and the full mesh.
    pub fn plan_sync(
        &mut self,
        step_index: u64,
        kind: TrainingCollectiveKind,
        payload_bytes: u64,
        requested_quantization: TrainingCollectiveQuantization,
        cadence_policy: &CollectiveSyncCadencePolicy,
    ) -> Result<CollectiveSyncExecutionPlan, CollectivePlanningError> {
        let mesh = self
            .latest_mesh
            .clone()
            .ok_or(CollectivePlanningError::MeshNotObserved)?;
        let local_group_size = preferred_local_group_size(&self.axes, self.members.len());
        let mut triggers = cadence_triggers(
            self.latest_transport_feedback.as_ref(),
            cadence_policy,
            self.latest_sync_receipt.as_ref(),
            mesh.mesh_revision,
        );
        let degraded_transport = triggers.iter().any(|trigger| {
            matches!(
                trigger.kind,
                CollectiveReplanTriggerKind::BandwidthBelowFloor
                    | CollectiveReplanTriggerKind::LatencyAboveCeiling
                    | CollectiveReplanTriggerKind::ActiveStreamsAboveCeiling
            )
        });
        let global_interval_steps = if degraded_transport && local_group_size.is_some() {
            cadence_policy.degraded_global_interval_steps.max(1)
        } else {
            cadence_policy.global_interval_steps.max(1)
        };
        let has_local_groups = local_group_size.is_some();
        let global_due =
            !has_local_groups || step_index == 0 || step_index % global_interval_steps == 0;
        if global_due && global_interval_steps > 1 {
            triggers.push(CollectiveReplanTrigger {
                kind: CollectiveReplanTriggerKind::GlobalIntervalElapsed,
                detail: format!(
                    "step {step_index} reached the global sync cadence interval of {global_interval_steps}"
                ),
            });
        }

        let (global_quantization, global_benchmark) = self.sync_quantization_choice(
            kind,
            requested_quantization,
            cadence_policy,
            &mut triggers,
        );
        let next_global_step = next_global_step(step_index, global_interval_steps);
        let cadence_class = match (degraded_transport, has_local_groups, global_due) {
            (true, true, true) => CollectiveSyncCadenceClass::LocalThenGlobal,
            (true, true, false) => CollectiveSyncCadenceClass::LocalOnlyDeferredGlobal,
            (_, false, _) => CollectiveSyncCadenceClass::GlobalOnlyFallback,
            _ => CollectiveSyncCadenceClass::EveryStepGlobal,
        };

        let mut stages = Vec::new();
        if degraded_transport {
            if let Some(local_group_size) = local_group_size {
                for (group_index, group_members) in
                    self.members.chunks(local_group_size).enumerate()
                {
                    stages.push(CollectiveSyncStage {
                        scope: CollectiveSyncScope::LocalGroup,
                        group_index: Some(group_index),
                        member_node_ids: group_members
                            .iter()
                            .map(|member| member.node_id.clone())
                            .collect(),
                        plan: build_collective_plan(
                            scoped_mesh_context(&mesh, group_members),
                            group_members,
                            kind,
                            payload_bytes,
                            cadence_policy.local_quantization,
                            self.approved_benchmark(kind, cadence_policy.local_quantization)?,
                            self.transport,
                            CollectiveSyncScope::LocalGroup,
                            Some(group_index),
                        ),
                    });
                }
            }
        }
        if global_due || !has_local_groups || !degraded_transport {
            stages.push(CollectiveSyncStage {
                scope: CollectiveSyncScope::GlobalMesh,
                group_index: None,
                member_node_ids: self
                    .members
                    .iter()
                    .map(|member| member.node_id.clone())
                    .collect(),
                plan: build_collective_plan(
                    mesh.clone(),
                    &self.members,
                    kind,
                    payload_bytes,
                    global_quantization,
                    global_benchmark,
                    self.transport,
                    CollectiveSyncScope::GlobalMesh,
                    None,
                ),
            });
        }

        let receipt = CollectiveSyncCadenceReceipt::new(
            step_index,
            mesh.mesh_revision,
            cadence_class,
            global_interval_steps,
            next_global_step,
            local_group_size,
            global_quantization,
            cadence_policy.local_quantization,
            degraded_transport,
            triggers,
            self.latest_transport_feedback.clone(),
        );
        self.latest_sync_receipt = Some(receipt.clone());
        Ok(CollectiveSyncExecutionPlan {
            cadence_receipt: receipt,
            stages,
        })
    }

    fn approved_benchmark(
        &self,
        kind: TrainingCollectiveKind,
        quantization: TrainingCollectiveQuantization,
    ) -> Result<Option<QuantizedCollectiveBenchmark>, CollectivePlanningError> {
        if quantization == TrainingCollectiveQuantization::None {
            return Ok(None);
        }
        let benchmark = self
            .benchmarks
            .iter()
            .find(|benchmark| {
                benchmark.kind == kind
                    && benchmark.quantization == quantization
                    && benchmark.accepted
            })
            .cloned()
            .ok_or(CollectivePlanningError::QuantizationNotApproved { kind, quantization })?;
        Ok(Some(benchmark))
    }

    fn sync_quantization_choice(
        &self,
        kind: TrainingCollectiveKind,
        requested_quantization: TrainingCollectiveQuantization,
        cadence_policy: &CollectiveSyncCadencePolicy,
        triggers: &mut Vec<CollectiveReplanTrigger>,
    ) -> (
        TrainingCollectiveQuantization,
        Option<QuantizedCollectiveBenchmark>,
    ) {
        match self.approved_benchmark(kind, requested_quantization) {
            Ok(benchmark) => (requested_quantization, benchmark),
            Err(CollectivePlanningError::QuantizationNotApproved { .. })
                if requested_quantization != cadence_policy.fallback_global_quantization =>
            {
                triggers.push(CollectiveReplanTrigger {
                    kind: CollectiveReplanTriggerKind::QuantizationApprovalMissing,
                    detail: format!(
                        "requested {:?} quantization was not benchmark-approved for {:?}; falling back to {:?}",
                        requested_quantization,
                        kind,
                        cadence_policy.fallback_global_quantization
                    ),
                });
                let benchmark = self
                    .approved_benchmark(kind, cadence_policy.fallback_global_quantization)
                    .ok()
                    .flatten();
                (cadence_policy.fallback_global_quantization, benchmark)
            }
            _ => (TrainingCollectiveQuantization::None, None),
        }
    }
}

fn validate_mesh_axes(
    axes: &[TrainingDeviceMeshAxis],
    member_count: usize,
) -> Result<(), CollectivePlanningError> {
    let axis_product = axes
        .iter()
        .fold(1usize, |acc, axis| acc.saturating_mul(axis.extent.max(1)));
    if axis_product != member_count.max(1) {
        return Err(CollectivePlanningError::AxisProductMismatch {
            axis_product,
            member_count,
        });
    }
    Ok(())
}

fn stable_mesh_digest(
    mesh_id: &str,
    effective_backend: &str,
    communication_class: ClusterCommunicationClass,
    axes: &[TrainingDeviceMeshAxis],
    elastic_membership: &TrainingElasticMembershipContext,
    member_node_ids: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_collective_mesh|");
    hasher.update(mesh_id.as_bytes());
    hasher.update(b"|");
    hasher.update(effective_backend.as_bytes());
    hasher.update(b"|");
    hasher.update(communication_class_label(communication_class));
    hasher.update(b"|membership_epoch|");
    hasher.update(elastic_membership.membership_epoch.to_string().as_bytes());
    hasher.update(b"|cluster_state|");
    hasher.update(elastic_membership.cluster_state_digest.as_bytes());
    hasher.update(b"|topology|");
    hasher.update(elastic_membership.topology_digest.as_bytes());
    for axis in axes {
        hasher.update(b"|axis|");
        hasher.update(axis.axis_id.as_bytes());
        hasher.update(b"|");
        hasher.update(training_axis_kind_label(axis.kind));
        hasher.update(b"|");
        hasher.update(axis.extent.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(axis.collective_group_size.to_string().as_bytes());
    }
    for node_id in member_node_ids {
        hasher.update(b"|member|");
        hasher.update(node_id.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_benchmark_digest(
    kind: TrainingCollectiveKind,
    quantization: TrainingCollectiveQuantization,
    baseline: QuantizedCollectiveBenchmarkSample,
    quantized: QuantizedCollectiveBenchmarkSample,
    maximum_error_bps: u64,
    minimum_speedup_bps: u64,
    accepted: bool,
    speedup_bps: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_collective_benchmark|");
    hasher.update(training_collective_kind_label(kind));
    hasher.update(b"|");
    hasher.update(training_collective_quantization_label(quantization));
    hasher.update(b"|baseline_duration|");
    hasher.update(baseline.duration_us.to_string().as_bytes());
    hasher.update(b"|baseline_wire|");
    hasher.update(baseline.wire_bytes.to_string().as_bytes());
    hasher.update(b"|quantized_duration|");
    hasher.update(quantized.duration_us.to_string().as_bytes());
    hasher.update(b"|quantized_wire|");
    hasher.update(quantized.wire_bytes.to_string().as_bytes());
    hasher.update(b"|quantized_error|");
    hasher.update(quantized.max_relative_error_bps.to_string().as_bytes());
    hasher.update(b"|maximum_error|");
    hasher.update(maximum_error_bps.to_string().as_bytes());
    hasher.update(b"|minimum_speedup|");
    hasher.update(minimum_speedup_bps.to_string().as_bytes());
    hasher.update(b"|accepted|");
    hasher.update(if accepted {
        b"yes".as_slice()
    } else {
        b"no".as_slice()
    });
    hasher.update(b"|speedup|");
    hasher.update(speedup_bps.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn benchmark_speedup_bps(baseline_duration_us: u64, quantized_duration_us: u64) -> u64 {
    if baseline_duration_us == 0 || quantized_duration_us >= baseline_duration_us {
        return 0;
    }
    baseline_duration_us
        .saturating_sub(quantized_duration_us)
        .saturating_mul(10_000)
        / baseline_duration_us
}

fn estimate_collective_wire_bytes(
    kind: TrainingCollectiveKind,
    payload_bytes: u64,
    worker_count: usize,
    quantization: TrainingCollectiveQuantization,
) -> u64 {
    let fanout_factor = match kind {
        TrainingCollectiveKind::AllReduce => worker_count.saturating_sub(1).saturating_mul(2),
        TrainingCollectiveKind::AllGather => worker_count.saturating_sub(1),
        TrainingCollectiveKind::ReduceScatter => worker_count.saturating_sub(1),
        TrainingCollectiveKind::Broadcast => worker_count.saturating_sub(1),
    }
    .max(1) as u64;
    let quantization_divisor = match quantization {
        TrainingCollectiveQuantization::None => 1,
        TrainingCollectiveQuantization::Int8Symmetric => 4,
        TrainingCollectiveQuantization::Nf4Blockwise => 8,
    };
    payload_bytes
        .saturating_mul(fanout_factor)
        .div_ceil(quantization_divisor)
}

fn ring_handoffs(
    members: &[CollectiveMeshMember],
    transport: ClusterTransportClass,
    quantization: TrainingCollectiveQuantization,
    estimated_wire_bytes: u64,
) -> Vec<ClusterShardHandoff> {
    if members.len() < 2 {
        return Vec::new();
    }
    let bytes_per_token = estimated_wire_bytes / members.len() as u64;
    members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            let next = &members[(index + 1) % members.len()];
            ClusterShardHandoff::new(
                member.shard_id,
                next.shard_id,
                member.node_id.clone(),
                next.node_id.clone(),
                ClusterShardHandoffKind::TensorCollective,
                transport,
                0,
                bytes_per_token,
            )
            .with_detail(format!(
                "{quantization:?} tensor-collective ring handoff from rank {} to {}",
                member.rank, next.rank
            ))
        })
        .collect()
}

fn build_collective_plan(
    mesh: TrainingDeviceMeshContext,
    members: &[CollectiveMeshMember],
    kind: TrainingCollectiveKind,
    payload_bytes: u64,
    quantization: TrainingCollectiveQuantization,
    benchmark: Option<QuantizedCollectiveBenchmark>,
    transport: ClusterTransportClass,
    scope: CollectiveSyncScope,
    group_index: Option<usize>,
) -> CollectiveExecutionPlan {
    let worker_count = members.len().max(1);
    let estimated_wire_bytes =
        estimate_collective_wire_bytes(kind, payload_bytes, worker_count, quantization);
    let scope_detail = match (scope, group_index) {
        (CollectiveSyncScope::LocalGroup, Some(group_index)) => {
            format!("local group {group_index}")
        }
        _ => String::from("global mesh"),
    };
    let mut collective = TrainingCollectiveContext::new(
        mesh.clone(),
        kind,
        quantization,
        payload_bytes,
        estimated_wire_bytes,
        worker_count,
    )
    .with_detail(format!(
        "{kind:?} collective planned over {scope_detail} on mesh `{}` revision {}",
        mesh.mesh_id, mesh.mesh_revision
    ));
    if let Some(benchmark) = benchmark {
        collective = collective.with_benchmark(
            benchmark.benchmark_digest,
            benchmark.speedup_bps,
            benchmark.quantized.max_relative_error_bps,
        );
    }
    CollectiveExecutionPlan {
        handoffs: ring_handoffs(members, transport, quantization, estimated_wire_bytes),
        work_item: RuntimeWorkItem::new(
            RuntimeWorkClass::CollectiveStep,
            worker_count,
            estimated_wire_bytes,
        ),
        collective,
    }
}

fn preferred_local_group_size(
    axes: &[TrainingDeviceMeshAxis],
    member_count: usize,
) -> Option<usize> {
    axes.iter()
        .map(|axis| axis.collective_group_size.max(1))
        .filter(|group_size| *group_size > 1 && *group_size < member_count)
        .max()
}

fn scoped_mesh_context(
    mesh: &TrainingDeviceMeshContext,
    members: &[CollectiveMeshMember],
) -> TrainingDeviceMeshContext {
    let mut scoped_mesh = mesh.clone();
    scoped_mesh.member_node_ids = members
        .iter()
        .map(|member| member.node_id.clone())
        .collect::<Vec<_>>();
    scoped_mesh
}

fn cadence_triggers(
    transport_feedback: Option<&CollectiveTransportFeedback>,
    cadence_policy: &CollectiveSyncCadencePolicy,
    previous_receipt: Option<&CollectiveSyncCadenceReceipt>,
    mesh_revision: u64,
) -> Vec<CollectiveReplanTrigger> {
    let mut triggers = Vec::new();
    if let Some(previous_receipt) = previous_receipt {
        if previous_receipt.mesh_revision != mesh_revision {
            triggers.push(CollectiveReplanTrigger {
                kind: CollectiveReplanTriggerKind::MeshRevisionChanged,
                detail: format!(
                    "mesh revision changed from {} to {}",
                    previous_receipt.mesh_revision, mesh_revision
                ),
            });
        }
    }
    if let Some(transport_feedback) = transport_feedback {
        if transport_feedback.estimated_bandwidth_mbps < cadence_policy.bandwidth_floor_mbps {
            triggers.push(CollectiveReplanTrigger {
                kind: CollectiveReplanTriggerKind::BandwidthBelowFloor,
                detail: format!(
                    "observed bandwidth {} mbps is below policy floor {} mbps",
                    transport_feedback.estimated_bandwidth_mbps,
                    cadence_policy.bandwidth_floor_mbps
                ),
            });
        }
        if transport_feedback.estimated_latency_ms > cadence_policy.latency_ceiling_ms {
            triggers.push(CollectiveReplanTrigger {
                kind: CollectiveReplanTriggerKind::LatencyAboveCeiling,
                detail: format!(
                    "observed latency {} ms is above policy ceiling {} ms",
                    transport_feedback.estimated_latency_ms, cadence_policy.latency_ceiling_ms
                ),
            });
        }
        if transport_feedback.active_streams > cadence_policy.active_stream_ceiling {
            triggers.push(CollectiveReplanTrigger {
                kind: CollectiveReplanTriggerKind::ActiveStreamsAboveCeiling,
                detail: format!(
                    "active streams {} exceed policy ceiling {}",
                    transport_feedback.active_streams, cadence_policy.active_stream_ceiling
                ),
            });
        }
    }
    triggers
}

fn next_global_step(step_index: u64, global_interval_steps: u64) -> u64 {
    let global_interval_steps = global_interval_steps.max(1);
    if step_index == 0 {
        return global_interval_steps;
    }
    let remainder = step_index % global_interval_steps;
    if remainder == 0 {
        step_index.saturating_add(global_interval_steps)
    } else {
        step_index
            .saturating_sub(remainder)
            .saturating_add(global_interval_steps)
    }
}

fn sorted_member_node_ids(members: &[CollectiveMeshMember]) -> Vec<String> {
    members
        .iter()
        .map(|member| member.node_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn communication_class_label(communication_class: ClusterCommunicationClass) -> &'static [u8] {
    match communication_class {
        ClusterCommunicationClass::RemoteDispatch => b"remote_dispatch",
        ClusterCommunicationClass::ReplicaRouting => b"replica_routing",
        ClusterCommunicationClass::PipelineStageHandoff => b"pipeline_stage_handoff",
        ClusterCommunicationClass::LayerShardHandoff => b"layer_shard_handoff",
        ClusterCommunicationClass::TensorCollectiveMesh => b"tensor_collective_mesh",
    }
}

fn training_axis_kind_label(kind: TrainingDeviceMeshAxisKind) -> &'static [u8] {
    match kind {
        TrainingDeviceMeshAxisKind::DataParallel => b"data_parallel",
        TrainingDeviceMeshAxisKind::TensorParallel => b"tensor_parallel",
        TrainingDeviceMeshAxisKind::PipelineParallel => b"pipeline_parallel",
        TrainingDeviceMeshAxisKind::ExpertParallel => b"expert_parallel",
    }
}

fn training_collective_kind_label(kind: TrainingCollectiveKind) -> &'static [u8] {
    match kind {
        TrainingCollectiveKind::AllReduce => b"all_reduce",
        TrainingCollectiveKind::AllGather => b"all_gather",
        TrainingCollectiveKind::ReduceScatter => b"reduce_scatter",
        TrainingCollectiveKind::Broadcast => b"broadcast",
    }
}

fn training_collective_quantization_label(
    quantization: TrainingCollectiveQuantization,
) -> &'static [u8] {
    match quantization {
        TrainingCollectiveQuantization::None => b"none",
        TrainingCollectiveQuantization::Int8Symmetric => b"int8_symmetric",
        TrainingCollectiveQuantization::Nf4Blockwise => b"nf4_blockwise",
    }
}

fn collective_replan_trigger_label(kind: CollectiveReplanTriggerKind) -> &'static [u8] {
    match kind {
        CollectiveReplanTriggerKind::MeshRevisionChanged => b"mesh_revision_changed",
        CollectiveReplanTriggerKind::BandwidthBelowFloor => b"bandwidth_below_floor",
        CollectiveReplanTriggerKind::LatencyAboveCeiling => b"latency_above_ceiling",
        CollectiveReplanTriggerKind::ActiveStreamsAboveCeiling => b"active_streams_above_ceiling",
        CollectiveReplanTriggerKind::GlobalIntervalElapsed => b"global_interval_elapsed",
        CollectiveReplanTriggerKind::QuantizationApprovalMissing => {
            b"quantization_approval_missing"
        }
    }
}

fn collective_sync_cadence_class_label(cadence_class: CollectiveSyncCadenceClass) -> &'static [u8] {
    match cadence_class {
        CollectiveSyncCadenceClass::EveryStepGlobal => b"every_step_global",
        CollectiveSyncCadenceClass::LocalOnlyDeferredGlobal => b"local_only_deferred_global",
        CollectiveSyncCadenceClass::LocalThenGlobal => b"local_then_global",
        CollectiveSyncCadenceClass::GlobalOnlyFallback => b"global_only_fallback",
    }
}

fn stable_transport_feedback_digest(
    observed_at_ms: u64,
    estimated_bandwidth_mbps: u64,
    estimated_latency_ms: u64,
    active_streams: u16,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_collective_transport_feedback|");
    hasher.update(observed_at_ms.to_string().as_bytes());
    hasher.update(b"|bandwidth|");
    hasher.update(estimated_bandwidth_mbps.to_string().as_bytes());
    hasher.update(b"|latency|");
    hasher.update(estimated_latency_ms.to_string().as_bytes());
    hasher.update(b"|streams|");
    hasher.update(active_streams.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

#[allow(clippy::too_many_arguments)]
fn stable_cadence_receipt_digest(
    step_index: u64,
    mesh_revision: u64,
    cadence_class: CollectiveSyncCadenceClass,
    global_interval_steps: u64,
    next_global_step: u64,
    local_group_size: Option<usize>,
    global_quantization: TrainingCollectiveQuantization,
    local_quantization: TrainingCollectiveQuantization,
    degraded_transport: bool,
    triggers: &[CollectiveReplanTrigger],
    transport_feedback: Option<&CollectiveTransportFeedback>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_collective_sync_cadence_receipt|");
    hasher.update(step_index.to_string().as_bytes());
    hasher.update(b"|mesh_revision|");
    hasher.update(mesh_revision.to_string().as_bytes());
    hasher.update(b"|cadence|");
    hasher.update(collective_sync_cadence_class_label(cadence_class));
    hasher.update(b"|interval|");
    hasher.update(global_interval_steps.to_string().as_bytes());
    hasher.update(b"|next|");
    hasher.update(next_global_step.to_string().as_bytes());
    hasher.update(b"|local_group|");
    hasher.update(local_group_size.unwrap_or_default().to_string().as_bytes());
    hasher.update(b"|global_quantization|");
    hasher.update(training_collective_quantization_label(global_quantization));
    hasher.update(b"|local_quantization|");
    hasher.update(training_collective_quantization_label(local_quantization));
    hasher.update(b"|degraded|");
    hasher.update(if degraded_transport {
        b"yes".as_slice()
    } else {
        b"no".as_slice()
    });
    for trigger in triggers {
        hasher.update(b"|trigger|");
        hasher.update(collective_replan_trigger_label(trigger.kind));
        hasher.update(b"|");
        hasher.update(trigger.detail.as_bytes());
    }
    if let Some(transport_feedback) = transport_feedback {
        hasher.update(b"|feedback|");
        hasher.update(transport_feedback.feedback_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_runtime::{
        ClusterCommunicationClass, TrainingCollectiveKind, TrainingCollectiveQuantization,
        TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind, TrainingElasticMembershipContext,
    };

    use super::{
        CollectiveMeshMember, CollectivePlanningError, CollectiveReplanTriggerKind,
        CollectiveSyncCadenceClass, CollectiveSyncCadencePolicy, CollectiveSyncScope,
        CollectiveTransportFeedback, ElasticCollectivePlanner, QuantizedCollectiveBenchmark,
        QuantizedCollectiveBenchmarkSample,
    };

    fn membership(
        epoch: u64,
        active: Vec<&str>,
        joining: Vec<&str>,
    ) -> TrainingElasticMembershipContext {
        TrainingElasticMembershipContext::new(
            epoch,
            format!("cluster-state-{epoch}"),
            format!("topology-{epoch}"),
            active.into_iter().map(String::from).collect(),
        )
        .with_joining_node_ids(joining.into_iter().map(String::from).collect())
    }

    fn mesh_members(node_ids: &[&str]) -> Vec<CollectiveMeshMember> {
        node_ids
            .iter()
            .enumerate()
            .map(|(index, node_id)| {
                CollectiveMeshMember::new(*node_id, index, index, format!("cuda:{index}"))
            })
            .collect()
    }

    fn four_way_planner() -> Result<ElasticCollectivePlanner, Box<dyn std::error::Error>> {
        let mut planner = ElasticCollectivePlanner::new(
            "mesh-train",
            "cuda",
            ClusterCommunicationClass::TensorCollectiveMesh,
            vec![
                TrainingDeviceMeshAxis::new("dp", TrainingDeviceMeshAxisKind::DataParallel, 2)
                    .with_collective_group_size(2),
                TrainingDeviceMeshAxis::new("tp", TrainingDeviceMeshAxisKind::TensorParallel, 2)
                    .with_collective_group_size(2),
            ],
        );
        planner.observe_mesh(
            membership(
                1,
                vec!["worker-a", "worker-b", "worker-c", "worker-d"],
                Vec::new(),
            ),
            mesh_members(&["worker-a", "worker-b", "worker-c", "worker-d"]),
        )?;
        planner.record_benchmark(QuantizedCollectiveBenchmark::new(
            TrainingCollectiveKind::AllReduce,
            TrainingCollectiveQuantization::Int8Symmetric,
            QuantizedCollectiveBenchmarkSample::new(2_400, 32 * 1024 * 1024, 0),
            QuantizedCollectiveBenchmarkSample::new(1_200, 8 * 1024 * 1024, 55),
            100,
            1_000,
        ));
        Ok(planner)
    }

    #[test]
    fn mesh_revision_advances_when_membership_changes() -> Result<(), Box<dyn std::error::Error>> {
        let mut planner = ElasticCollectivePlanner::new(
            "mesh-train",
            "cuda",
            ClusterCommunicationClass::TensorCollectiveMesh,
            vec![
                TrainingDeviceMeshAxis::new("dp", TrainingDeviceMeshAxisKind::DataParallel, 1),
                TrainingDeviceMeshAxis::new("tp", TrainingDeviceMeshAxisKind::TensorParallel, 2),
            ],
        );

        let first = planner.observe_mesh(
            membership(1, vec!["worker-a", "worker-b"], Vec::new()),
            mesh_members(&["worker-a", "worker-b"]),
        )?;
        let second = planner.observe_mesh(
            membership(1, vec!["worker-a", "worker-b"], Vec::new()),
            mesh_members(&["worker-a", "worker-b"]),
        )?;
        let third = planner.observe_mesh(
            membership(2, vec!["worker-a", "worker-b"], vec!["worker-c"]),
            mesh_members(&["worker-a", "worker-b"]),
        )?;

        assert_eq!(first.mesh.mesh_revision, 1);
        assert!(first.changed);
        assert_eq!(second.mesh.mesh_revision, 1);
        assert!(!second.changed);
        assert_eq!(third.mesh.mesh_revision, 2);
        assert!(third.changed);
        Ok(())
    }

    #[test]
    fn planner_prefers_quantized_collective_when_benchmark_is_accepted()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut planner = ElasticCollectivePlanner::new(
            "mesh-train",
            "cuda",
            ClusterCommunicationClass::TensorCollectiveMesh,
            vec![
                TrainingDeviceMeshAxis::new("dp", TrainingDeviceMeshAxisKind::DataParallel, 1),
                TrainingDeviceMeshAxis::new("tp", TrainingDeviceMeshAxisKind::TensorParallel, 2),
            ],
        );
        planner.observe_mesh(
            membership(1, vec!["worker-a", "worker-b"], Vec::new()),
            mesh_members(&["worker-a", "worker-b"]),
        )?;
        let benchmark = QuantizedCollectiveBenchmark::new(
            TrainingCollectiveKind::AllReduce,
            TrainingCollectiveQuantization::Int8Symmetric,
            QuantizedCollectiveBenchmarkSample::new(2_400, 32 * 1024 * 1024, 0),
            QuantizedCollectiveBenchmarkSample::new(1_200, 8 * 1024 * 1024, 55),
            100,
            1_000,
        );
        assert!(benchmark.accepted);
        planner.record_benchmark(benchmark.clone());

        let plan = planner.plan_collective(
            TrainingCollectiveKind::AllReduce,
            16 * 1024 * 1024,
            TrainingCollectiveQuantization::Int8Symmetric,
        )?;

        assert_eq!(
            plan.collective.quantization,
            TrainingCollectiveQuantization::Int8Symmetric
        );
        assert_eq!(
            plan.collective.benchmark_digest.as_deref(),
            Some(benchmark.benchmark_digest.as_str())
        );
        assert!(plan.collective.estimated_wire_bytes < plan.collective.payload_bytes * 2);
        assert_eq!(
            plan.work_item.class,
            psionic_runtime::RuntimeWorkClass::CollectiveStep
        );
        assert_eq!(plan.handoffs.len(), 2);
        Ok(())
    }

    #[test]
    fn planner_refuses_unapproved_quantization() -> Result<(), Box<dyn std::error::Error>> {
        let mut planner = ElasticCollectivePlanner::new(
            "mesh-train",
            "cuda",
            ClusterCommunicationClass::TensorCollectiveMesh,
            vec![
                TrainingDeviceMeshAxis::new("dp", TrainingDeviceMeshAxisKind::DataParallel, 1),
                TrainingDeviceMeshAxis::new("tp", TrainingDeviceMeshAxisKind::TensorParallel, 2),
            ],
        );
        planner.observe_mesh(
            membership(1, vec!["worker-a", "worker-b"], Vec::new()),
            mesh_members(&["worker-a", "worker-b"]),
        )?;

        let result = planner.plan_collective(
            TrainingCollectiveKind::AllReduce,
            16 * 1024 * 1024,
            TrainingCollectiveQuantization::Int8Symmetric,
        );

        assert!(matches!(
            result,
            Err(CollectivePlanningError::QuantizationNotApproved { .. })
        ));
        Ok(())
    }

    #[test]
    fn sync_planner_defers_global_sync_when_transport_is_degraded()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut planner = four_way_planner()?;
        planner.observe_transport_feedback(
            CollectiveTransportFeedback::new(2_000, 400, 12, 2)
                .with_detail("wan bandwidth fell below the safe floor"),
        );
        let policy = CollectiveSyncCadencePolicy::new()
            .with_degraded_global_interval_steps(4)
            .with_transport_thresholds(800, 8, 8);

        let plan = planner.plan_sync(
            2,
            TrainingCollectiveKind::AllReduce,
            16 * 1024 * 1024,
            TrainingCollectiveQuantization::Int8Symmetric,
            &policy,
        )?;

        assert_eq!(
            plan.cadence_receipt.cadence_class,
            CollectiveSyncCadenceClass::LocalOnlyDeferredGlobal
        );
        assert!(plan.cadence_receipt.degraded_transport);
        assert_eq!(plan.cadence_receipt.next_global_step, 4);
        assert_eq!(plan.stages.len(), 2);
        assert!(
            plan.stages
                .iter()
                .all(|stage| stage.scope == CollectiveSyncScope::LocalGroup)
        );
        assert!(
            plan.cadence_receipt.triggers.iter().any(|trigger| {
                trigger.kind == CollectiveReplanTriggerKind::BandwidthBelowFloor
            })
        );
        Ok(())
    }

    #[test]
    fn sync_planner_runs_local_then_global_when_interval_elapses()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut planner = four_way_planner()?;
        planner.observe_transport_feedback(
            CollectiveTransportFeedback::new(2_000, 400, 12, 2)
                .with_detail("wan bandwidth fell below the safe floor"),
        );
        let policy = CollectiveSyncCadencePolicy::new()
            .with_degraded_global_interval_steps(4)
            .with_transport_thresholds(800, 8, 8);

        let plan = planner.plan_sync(
            4,
            TrainingCollectiveKind::AllReduce,
            16 * 1024 * 1024,
            TrainingCollectiveQuantization::Int8Symmetric,
            &policy,
        )?;

        assert_eq!(
            plan.cadence_receipt.cadence_class,
            CollectiveSyncCadenceClass::LocalThenGlobal
        );
        assert_eq!(plan.stages.len(), 3);
        assert_eq!(
            plan.stages.last().expect("global stage").scope,
            CollectiveSyncScope::GlobalMesh
        );
        assert_eq!(
            plan.stages
                .last()
                .expect("global stage")
                .plan
                .collective
                .quantization,
            TrainingCollectiveQuantization::Int8Symmetric
        );
        assert!(
            plan.cadence_receipt.triggers.iter().any(|trigger| {
                trigger.kind == CollectiveReplanTriggerKind::GlobalIntervalElapsed
            })
        );
        Ok(())
    }

    #[test]
    fn sync_planner_emits_mesh_revision_replan_trigger() -> Result<(), Box<dyn std::error::Error>> {
        let mut planner = four_way_planner()?;
        let policy = CollectiveSyncCadencePolicy::new();
        let _initial = planner.plan_sync(
            1,
            TrainingCollectiveKind::AllReduce,
            16 * 1024 * 1024,
            TrainingCollectiveQuantization::Int8Symmetric,
            &policy,
        )?;
        planner.observe_mesh(
            membership(
                2,
                vec!["worker-a", "worker-b", "worker-c", "worker-d"],
                vec!["worker-e"],
            ),
            mesh_members(&["worker-a", "worker-b", "worker-c", "worker-d"]),
        )?;

        let replanned = planner.plan_sync(
            2,
            TrainingCollectiveKind::AllReduce,
            16 * 1024 * 1024,
            TrainingCollectiveQuantization::Int8Symmetric,
            &policy,
        )?;

        assert!(
            replanned.cadence_receipt.triggers.iter().any(|trigger| {
                trigger.kind == CollectiveReplanTriggerKind::MeshRevisionChanged
            })
        );
        Ok(())
    }

    #[test]
    fn sync_planner_records_quantization_fallback_in_receipt()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut planner = ElasticCollectivePlanner::new(
            "mesh-train",
            "cuda",
            ClusterCommunicationClass::TensorCollectiveMesh,
            vec![
                TrainingDeviceMeshAxis::new("dp", TrainingDeviceMeshAxisKind::DataParallel, 2)
                    .with_collective_group_size(2),
                TrainingDeviceMeshAxis::new("tp", TrainingDeviceMeshAxisKind::TensorParallel, 2)
                    .with_collective_group_size(2),
            ],
        );
        planner.observe_mesh(
            membership(
                1,
                vec!["worker-a", "worker-b", "worker-c", "worker-d"],
                Vec::new(),
            ),
            mesh_members(&["worker-a", "worker-b", "worker-c", "worker-d"]),
        )?;
        planner.observe_transport_feedback(
            CollectiveTransportFeedback::new(2_000, 400, 12, 2)
                .with_detail("wan bandwidth fell below the safe floor"),
        );
        let policy = CollectiveSyncCadencePolicy::new()
            .with_degraded_global_interval_steps(4)
            .with_transport_thresholds(800, 8, 8)
            .with_fallback_global_quantization(TrainingCollectiveQuantization::None);

        let plan = planner.plan_sync(
            4,
            TrainingCollectiveKind::AllReduce,
            16 * 1024 * 1024,
            TrainingCollectiveQuantization::Int8Symmetric,
            &policy,
        )?;

        assert_eq!(
            plan.cadence_receipt.global_quantization,
            TrainingCollectiveQuantization::None
        );
        assert!(plan.cadence_receipt.triggers.iter().any(|trigger| {
            trigger.kind == CollectiveReplanTriggerKind::QuantizationApprovalMissing
        }));
        Ok(())
    }
}
