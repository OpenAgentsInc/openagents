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
        let worker_count = self.members.len().max(1);
        let (quantization, benchmark) =
            if requested_quantization == TrainingCollectiveQuantization::None {
                (TrainingCollectiveQuantization::None, None)
            } else {
                let benchmark = self
                    .benchmarks
                    .iter()
                    .find(|benchmark| {
                        benchmark.kind == kind
                            && benchmark.quantization == requested_quantization
                            && benchmark.accepted
                    })
                    .cloned()
                    .ok_or(CollectivePlanningError::QuantizationNotApproved {
                        kind,
                        quantization: requested_quantization,
                    })?;
                (requested_quantization, Some(benchmark))
            };
        let estimated_wire_bytes =
            estimate_collective_wire_bytes(kind, payload_bytes, worker_count, quantization);
        let mut collective = TrainingCollectiveContext::new(
            mesh.clone(),
            kind,
            quantization,
            payload_bytes,
            estimated_wire_bytes,
            worker_count,
        )
        .with_detail(format!(
            "{kind:?} collective planned over mesh `{}` revision {}",
            mesh.mesh_id, mesh.mesh_revision
        ));
        if let Some(benchmark) = benchmark {
            collective = collective.with_benchmark(
                benchmark.benchmark_digest,
                benchmark.speedup_bps,
                benchmark.quantized.max_relative_error_bps,
            );
        }
        Ok(CollectiveExecutionPlan {
            handoffs: ring_handoffs(
                &self.members,
                self.transport,
                quantization,
                estimated_wire_bytes,
            ),
            work_item: RuntimeWorkItem::new(
                RuntimeWorkClass::CollectiveStep,
                worker_count,
                estimated_wire_bytes,
            ),
            collective,
        })
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

#[cfg(test)]
mod tests {
    use psionic_runtime::{
        ClusterCommunicationClass, TrainingCollectiveKind, TrainingCollectiveQuantization,
        TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind, TrainingElasticMembershipContext,
    };

    use super::{
        CollectiveMeshMember, CollectivePlanningError, ElasticCollectivePlanner,
        QuantizedCollectiveBenchmark, QuantizedCollectiveBenchmarkSample,
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
}
