use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ClusterId, NodeId,
    ordered_state::{
        ClusterCatchupResponse, ClusterRecoveryDisposition, ClusterRecoveryEnvelopeError,
        ClusterState,
    },
};

/// Schema version for typed cluster benchmark receipts.
pub const CLUSTER_BENCHMARK_RECEIPT_SCHEMA_VERSION: u32 = 1;

/// Stable benchmark identity for one cluster benchmark gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterBenchmarkId {
    /// Whole-request remote scheduling benchmark gate.
    WholeRequestScheduler,
    /// Ordered recovery catchup benchmark gate.
    RecoveryCatchup,
    /// Replicated serving planning benchmark gate.
    ReplicatedServing,
    /// Pipeline-sharded planner benchmark gate.
    PipelineShardedPlanner,
    /// Layer-sharded planner benchmark gate.
    LayerShardedPlanner,
    /// Tensor-sharded planner benchmark gate.
    TensorShardedPlanner,
}

impl ClusterBenchmarkId {
    /// Returns a stable benchmark identifier string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WholeRequestScheduler => "whole_request_scheduler",
            Self::RecoveryCatchup => "recovery_catchup",
            Self::ReplicatedServing => "replicated_serving",
            Self::PipelineShardedPlanner => "pipeline_sharded_planner",
            Self::LayerShardedPlanner => "layer_sharded_planner",
            Self::TensorShardedPlanner => "tensor_sharded_planner",
        }
    }
}

/// Pass or fail outcome for one benchmark gate run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterBenchmarkOutcome {
    /// Measured total duration stayed within the configured budget.
    Passed,
    /// Measured total duration exceeded the configured budget.
    Failed,
}

impl ClusterBenchmarkOutcome {
    #[must_use]
    const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

/// Topology-backed context used by scheduling and sharding benchmark gates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTopologyBenchmarkContext {
    /// Cluster identity that owned the planning facts.
    pub cluster_id: ClusterId,
    /// Stable digest of the authoritative cluster snapshot used for the run.
    pub cluster_state_digest: String,
    /// Stable digest of the topology slice used for the run.
    pub topology_digest: String,
    /// Stable digest of artifact residency facts when they constrain the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_residency_digest: Option<String>,
}

impl ClusterTopologyBenchmarkContext {
    /// Creates topology-backed benchmark context explicitly.
    #[must_use]
    pub fn new(
        cluster_id: ClusterId,
        cluster_state_digest: impl Into<String>,
        topology_digest: impl Into<String>,
    ) -> Self {
        Self {
            cluster_id,
            cluster_state_digest: cluster_state_digest.into(),
            topology_digest: topology_digest.into(),
            artifact_residency_digest: None,
        }
    }

    /// Derives topology-backed context from authoritative cluster state.
    #[must_use]
    pub fn from_state(cluster_id: ClusterId, state: &ClusterState) -> Self {
        Self::new(cluster_id, state.stable_digest(), state.topology_digest())
            .with_artifact_residency_digest(state.artifact_residency_digest())
    }

    /// Attaches artifact residency truth for the benchmarked decision.
    #[must_use]
    pub fn with_artifact_residency_digest(
        mut self,
        artifact_residency_digest: impl Into<String>,
    ) -> Self {
        self.artifact_residency_digest = Some(artifact_residency_digest.into());
        self
    }

    /// Returns a stable digest for the topology-backed context.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_topology_benchmark_context|");
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(b"|cluster_state_digest|");
        hasher.update(self.cluster_state_digest.as_bytes());
        hasher.update(b"|topology_digest|");
        hasher.update(self.topology_digest.as_bytes());
        if let Some(artifact_residency_digest) = &self.artifact_residency_digest {
            hasher.update(b"|artifact_residency_digest|");
            hasher.update(artifact_residency_digest.as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// Recovery-backed context used by catchup benchmark gates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterRecoveryBenchmarkContext {
    /// Cluster identity that owned the recovery response.
    pub cluster_id: ClusterId,
    /// Node that requested the recovery response.
    pub requester_id: NodeId,
    /// Stable digest of the recovery response shape used for the run.
    pub recovery_response_digest: String,
    /// Recovery disposition produced for the request.
    pub disposition: ClusterRecoveryDisposition,
    /// Authoritative head event index exposed by the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_event_index: Option<u64>,
}

impl ClusterRecoveryBenchmarkContext {
    /// Creates recovery-backed benchmark context explicitly.
    #[must_use]
    pub fn new(
        cluster_id: ClusterId,
        requester_id: NodeId,
        recovery_response_digest: impl Into<String>,
        disposition: ClusterRecoveryDisposition,
    ) -> Self {
        Self {
            cluster_id,
            requester_id,
            recovery_response_digest: recovery_response_digest.into(),
            disposition,
            head_event_index: None,
        }
    }

    /// Derives recovery-backed context from one recovery response.
    pub fn from_response(
        response: &ClusterCatchupResponse,
    ) -> Result<Self, ClusterRecoveryEnvelopeError> {
        Ok(Self {
            cluster_id: response.cluster_id.clone(),
            requester_id: response.requester_id.clone(),
            recovery_response_digest: response.stable_digest()?,
            disposition: response.disposition,
            head_event_index: response.head_event_index.map(|index| index.as_u64()),
        })
    }

    /// Returns a stable digest for the recovery-backed context.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_recovery_benchmark_context|");
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(b"|requester_id|");
        hasher.update(self.requester_id.as_str().as_bytes());
        hasher.update(b"|recovery_response_digest|");
        hasher.update(self.recovery_response_digest.as_bytes());
        hasher.update(b"|disposition|");
        hasher.update(match self.disposition {
            ClusterRecoveryDisposition::CatchUp => b"catch_up".as_slice(),
            ClusterRecoveryDisposition::InstallSnapshot => b"install_snapshot".as_slice(),
            ClusterRecoveryDisposition::FullResync => b"full_resync".as_slice(),
        });
        if let Some(head_event_index) = self.head_event_index {
            hasher.update(b"|head_event_index|");
            hasher.update(head_event_index.to_string().as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// Typed benchmark context for one cluster benchmark gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClusterBenchmarkContext {
    /// Context derived from authoritative cluster-state topology facts.
    Topology(ClusterTopologyBenchmarkContext),
    /// Context derived from authoritative recovery response facts.
    Recovery(ClusterRecoveryBenchmarkContext),
}

impl ClusterBenchmarkContext {
    /// Returns a stable digest for one benchmark context.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        match self {
            Self::Topology(context) => context.stable_digest(),
            Self::Recovery(context) => context.stable_digest(),
        }
    }
}

/// Machine-checkable receipt for one cluster benchmark gate run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterBenchmarkReceipt {
    /// Schema version for the receipt shape.
    pub schema_version: u32,
    /// Stable benchmark identity.
    pub benchmark_id: ClusterBenchmarkId,
    /// Topology or recovery context used for the measurement.
    pub context: ClusterBenchmarkContext,
    /// Iteration count actually executed.
    pub iterations: u64,
    /// Maximum permitted total runtime for the gate.
    pub max_total_duration_ms: u64,
    /// Total runtime observed for the gate.
    pub total_duration_ns: u64,
    /// Average runtime per iteration.
    pub average_duration_ns: u64,
    /// Pass/fail outcome derived from the configured budget.
    pub outcome: ClusterBenchmarkOutcome,
}

impl ClusterBenchmarkReceipt {
    /// Creates one measured cluster benchmark receipt from the observed timing.
    #[must_use]
    pub fn measured(
        benchmark_id: ClusterBenchmarkId,
        context: ClusterBenchmarkContext,
        iterations: usize,
        max_total_duration_ms: u128,
        elapsed: Duration,
    ) -> Self {
        let iterations = u64::try_from(iterations).unwrap_or(u64::MAX);
        let max_total_duration_ms = u64::try_from(max_total_duration_ms).unwrap_or(u64::MAX);
        let total_duration_ns = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        let average_duration_ns = if iterations == 0 {
            0
        } else {
            total_duration_ns / iterations
        };
        let outcome = if elapsed.as_millis() <= u128::from(max_total_duration_ms) {
            ClusterBenchmarkOutcome::Passed
        } else {
            ClusterBenchmarkOutcome::Failed
        };
        Self {
            schema_version: CLUSTER_BENCHMARK_RECEIPT_SCHEMA_VERSION,
            benchmark_id,
            context,
            iterations,
            max_total_duration_ms,
            total_duration_ns,
            average_duration_ns,
            outcome,
        }
    }

    /// Returns a stable digest for the receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_benchmark_receipt|");
        hasher.update(self.schema_version.to_string().as_bytes());
        hasher.update(b"|benchmark_id|");
        hasher.update(self.benchmark_id.as_str().as_bytes());
        hasher.update(b"|context|");
        hasher.update(self.context.stable_digest().as_bytes());
        hasher.update(b"|iterations|");
        hasher.update(self.iterations.to_string().as_bytes());
        hasher.update(b"|max_total_duration_ms|");
        hasher.update(self.max_total_duration_ms.to_string().as_bytes());
        hasher.update(b"|total_duration_ns|");
        hasher.update(self.total_duration_ns.to_string().as_bytes());
        hasher.update(b"|average_duration_ns|");
        hasher.update(self.average_duration_ns.to_string().as_bytes());
        hasher.update(b"|outcome|");
        hasher.update(self.outcome.as_str().as_bytes());
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::{
        CLUSTER_BENCHMARK_RECEIPT_SCHEMA_VERSION, ClusterBenchmarkContext, ClusterBenchmarkId,
        ClusterBenchmarkOutcome, ClusterBenchmarkReceipt, ClusterRecoveryBenchmarkContext,
        ClusterTopologyBenchmarkContext,
    };
    use std::time::Duration;

    use crate::{AdmissionToken, ClusterId, ClusterNamespace, ClusterRecoveryDisposition, NodeId};

    #[test]
    fn cluster_benchmark_receipt_round_trips_with_stable_serialization_and_digest() {
        let receipt = ClusterBenchmarkReceipt::measured(
            ClusterBenchmarkId::WholeRequestScheduler,
            ClusterBenchmarkContext::Topology(
                ClusterTopologyBenchmarkContext::new(
                    sample_cluster_id(),
                    "cluster-state-digest",
                    "cluster-topology-digest",
                )
                .with_artifact_residency_digest("artifact-residency-digest"),
            ),
            10_000,
            2_500,
            Duration::from_millis(214),
        );
        let encoded = match serde_json::to_string(&receipt) {
            Ok(value) => value,
            Err(error) => panic!("benchmark receipt should encode: {error}"),
        };
        let encoded_again = match serde_json::to_string(&receipt) {
            Ok(value) => value,
            Err(error) => panic!("benchmark receipt should encode repeatably: {error}"),
        };
        let decoded: ClusterBenchmarkReceipt = match serde_json::from_str(&encoded) {
            Ok(value) => value,
            Err(error) => panic!("benchmark receipt should decode: {error}"),
        };
        assert_eq!(encoded, encoded_again);
        assert_eq!(decoded, receipt);
        assert_eq!(decoded.stable_digest(), receipt.stable_digest());
    }

    #[test]
    fn cluster_benchmark_receipt_digest_changes_when_budget_or_context_changes() {
        let receipt = ClusterBenchmarkReceipt::measured(
            ClusterBenchmarkId::ReplicatedServing,
            ClusterBenchmarkContext::Topology(
                ClusterTopologyBenchmarkContext::new(
                    sample_cluster_id(),
                    "cluster-state-digest",
                    "cluster-topology-digest",
                )
                .with_artifact_residency_digest("artifact-residency-digest"),
            ),
            5_000,
            4_000,
            Duration::from_millis(267),
        );
        let tighter_budget = ClusterBenchmarkReceipt::measured(
            ClusterBenchmarkId::ReplicatedServing,
            ClusterBenchmarkContext::Topology(
                ClusterTopologyBenchmarkContext::new(
                    sample_cluster_id(),
                    "cluster-state-digest",
                    "cluster-topology-digest",
                )
                .with_artifact_residency_digest("artifact-residency-digest"),
            ),
            5_000,
            200,
            Duration::from_millis(267),
        );
        let recovery_context = ClusterBenchmarkReceipt::measured(
            ClusterBenchmarkId::RecoveryCatchup,
            ClusterBenchmarkContext::Recovery(ClusterRecoveryBenchmarkContext::new(
                sample_cluster_id(),
                NodeId::new("worker-a"),
                "recovery-response-digest",
                ClusterRecoveryDisposition::CatchUp,
            )),
            5_000,
            2_500,
            Duration::from_millis(24),
        );
        assert_ne!(receipt.stable_digest(), tighter_budget.stable_digest());
        assert_ne!(receipt.stable_digest(), recovery_context.stable_digest());
    }

    #[test]
    fn cluster_benchmark_receipt_serializes_machine_checkable_fields() {
        let receipt = ClusterBenchmarkReceipt::measured(
            ClusterBenchmarkId::TensorShardedPlanner,
            ClusterBenchmarkContext::Recovery(ClusterRecoveryBenchmarkContext::new(
                sample_cluster_id(),
                NodeId::new("worker-b"),
                "recovery-response-digest",
                ClusterRecoveryDisposition::CatchUp,
            )),
            2_000,
            4_000,
            Duration::from_millis(133),
        );
        let encoded = match serde_json::to_value(&receipt) {
            Ok(value) => value,
            Err(error) => panic!("benchmark receipt should encode: {error}"),
        };
        assert_eq!(
            encoded["schema_version"],
            serde_json::json!(CLUSTER_BENCHMARK_RECEIPT_SCHEMA_VERSION)
        );
        assert_eq!(
            encoded["benchmark_id"],
            serde_json::json!("tensor_sharded_planner")
        );
        assert_eq!(encoded["context"]["kind"], serde_json::json!("recovery"));
        assert_eq!(encoded["iterations"], serde_json::json!(2_000));
        assert_eq!(encoded["max_total_duration_ms"], serde_json::json!(4_000));
        assert_eq!(
            encoded["total_duration_ns"],
            serde_json::json!(133_000_000u64)
        );
        assert_eq!(encoded["average_duration_ns"], serde_json::json!(66_500u64));
        assert_eq!(encoded["outcome"], serde_json::json!("passed"));
        assert_eq!(receipt.outcome, ClusterBenchmarkOutcome::Passed);
    }

    fn sample_cluster_id() -> ClusterId {
        ClusterId::new(
            &ClusterNamespace::new("cluster-bench"),
            &AdmissionToken::new("cluster-secret"),
        )
    }
}
