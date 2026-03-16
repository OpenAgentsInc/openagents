//! Runtime traits and execution surfaces for Psionic.

mod activation_fingerprint;
mod gpt_oss;
mod local_multi_device;
mod parity;
mod proof;
mod structured_output;
mod tassadar;
mod validation;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub use activation_fingerprint::*;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
pub use gpt_oss::*;
pub use local_multi_device::*;
pub use parity::*;
pub use proof::*;
use psionic_core::{
    BackendExtensionKind, DType, Device, DeviceKind, PsionicRefusal, PsionicRefusalCode,
    PsionicRefusalScope, QuantizationMode, QuantizedBlockLayout, TensorId, TensorSpec,
    ViewSemantics,
};
use psionic_ir::ExecutionPlan;
use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub use structured_output::*;
pub use tassadar::*;
use thiserror::Error;
pub use validation::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "runtime traits for devices and execution";

/// Stable runtime backend name.
pub type BackendName = &'static str;

/// Runtime failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeError {
    /// The requested tensor input was not supplied.
    #[error("missing input tensor {0}")]
    MissingInput(TensorId),
    /// A buffer shape or dtype was not what execution expected.
    #[error("invalid buffer for tensor {tensor}: expected {expected:?}, actual {actual:?}")]
    InvalidBuffer {
        /// Tensor ID that failed validation.
        tensor: TensorId,
        /// Expected tensor specification.
        expected: TensorSpec,
        /// Actual tensor specification.
        actual: TensorSpec,
    },
    /// The execution plan referenced a node that the backend cannot execute.
    #[error("unsupported execution step `{0}`")]
    UnsupportedStep(String),
    /// Generic backend failure.
    #[error("{0}")]
    Backend(String),
}

impl RuntimeError {
    /// Returns the canonical refusal when the runtime error represents one
    /// explicit unsupported or incompatibility boundary.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::InvalidBuffer { tensor, .. } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedLayout,
                    PsionicRefusalScope::Runtime,
                    self.to_string(),
                )
                .with_subject(format!("{tensor:?}")),
            ),
            Self::UnsupportedStep(step) => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Runtime,
                    self.to_string(),
                )
                .with_subject(step.clone()),
            ),
            Self::MissingInput(_) | Self::Backend(_) => None,
        }
    }
}

/// Backend-neutral local runtime error taxonomy used by served-product surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalRuntimeErrorCode {
    /// The caller supplied an invalid request.
    InvalidRequest,
    /// The caller targeted the wrong served product.
    UnsupportedProduct,
    /// The caller requested a model that is unsupported by the current path.
    UnsupportedModel,
    /// The requested model was not found.
    ModelNotFound,
    /// The requested model is not currently loaded.
    ModelNotLoaded,
    /// The requested backend or model capability is unsupported.
    UnsupportedCapability,
    /// A required local artifact is missing.
    ArtifactMissing,
    /// A local artifact is unreadable, corrupt, or otherwise invalid.
    ArtifactInvalid,
    /// The request exceeded an explicit context or prompt budget.
    ContextOverflow,
    /// The referenced generation session does not exist.
    SessionNotFound,
    /// The referenced generation session is incompatible with the request.
    SessionMismatch,
    /// The request exhausted explicit KV/cache limits.
    CacheExhausted,
    /// Local-serving admission policy refused the request.
    AdmissionRefused,
    /// The requested backend is unavailable.
    BackendUnavailable,
    /// The selected backend is degraded and refused execution.
    BackendDegraded,
    /// Backend execution failed after planning succeeded.
    BackendExecutionFailed,
    /// The runtime produced an invalid output payload.
    InvalidOutput,
    /// The caller cancelled the request.
    Cancelled,
    /// The client disconnected after execution started.
    Disconnected,
    /// The runtime hit an unexpected internal error.
    Internal,
}

/// Structured diagnostic carried across the local runtime seam.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRuntimeDiagnostic {
    /// Stable taxonomy code.
    pub code: LocalRuntimeErrorCode,
    /// Comparable HTTP-style status code for app cutover and conformance.
    pub status: u16,
    /// Plain-language message safe to surface in logs or UI.
    pub message: String,
    /// Stable product identifier when the diagnostic belongs to one request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    /// Stable model identifier when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// Runtime backend involved in the failure when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Honest backend health posture when the diagnostic is backend-driven.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_health: Option<HealthStatus>,
}

impl LocalRuntimeDiagnostic {
    /// Creates a structured local-runtime diagnostic.
    #[must_use]
    pub fn new(code: LocalRuntimeErrorCode, status: u16, message: impl Into<String>) -> Self {
        Self {
            code,
            status,
            message: message.into(),
            product_id: None,
            model_id: None,
            backend: None,
            backend_health: None,
        }
    }

    /// Attaches a served-product identifier.
    #[must_use]
    pub fn with_product_id(mut self, product_id: impl Into<String>) -> Self {
        self.product_id = Some(product_id.into());
        self
    }

    /// Attaches a model identifier.
    #[must_use]
    pub fn with_model_id(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = Some(model_id.into());
        self
    }

    /// Attaches the runtime backend name.
    #[must_use]
    pub fn with_backend(mut self, backend: impl Into<String>) -> Self {
        self.backend = Some(backend.into());
        self
    }

    /// Attaches the backend health posture.
    #[must_use]
    pub const fn with_backend_health(mut self, backend_health: HealthStatus) -> Self {
        self.backend_health = Some(backend_health);
        self
    }

    /// Returns the canonical refusal when the diagnostic represents one explicit
    /// unsupported or incompatibility boundary.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        let subject = self
            .backend
            .clone()
            .or_else(|| self.model_id.clone())
            .or_else(|| self.product_id.clone());
        let refusal = match self.code {
            LocalRuntimeErrorCode::UnsupportedCapability
            | LocalRuntimeErrorCode::BackendUnavailable
            | LocalRuntimeErrorCode::BackendDegraded => PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedBackendCapability,
                PsionicRefusalScope::Runtime,
                self.message.clone(),
            ),
            LocalRuntimeErrorCode::SessionMismatch => PsionicRefusal::new(
                PsionicRefusalCode::SerializationIncompatibility,
                PsionicRefusalScope::Runtime,
                self.message.clone(),
            ),
            _ => return None,
        };
        Some(match subject {
            Some(subject) => refusal.with_subject(subject),
            None => refusal,
        })
    }
}

/// Runtime-visible device description.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceDescriptor {
    /// Backend family name.
    pub backend: String,
    /// Logical device.
    pub device: Device,
    /// Human-readable device name when the backend can supply one.
    pub device_name: Option<String>,
    /// Supported dtypes for the device.
    pub supported_dtypes: Vec<DType>,
    /// Supported quantization modes for model-backed execution.
    pub supported_quantization: Vec<QuantizationSupport>,
    /// Optional memory capacity in bytes.
    pub memory_capacity_bytes: Option<u64>,
    /// Whether the device shares memory with the host, when known.
    pub unified_memory: Option<bool>,
    /// Stable feature flags relevant to runtime/backend selection.
    pub feature_flags: Vec<String>,
    /// AMD-specific topology/risk metadata when the device belongs to an AMD backend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd_metadata: Option<AmdDeviceMetadata>,
    /// NVIDIA-specific topology/risk metadata when the device belongs to a CUDA backend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvidia_metadata: Option<NvidiaDeviceMetadata>,
}

/// High-level memory posture for one advertised execution device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceMemoryClass {
    /// Pure host/system-memory execution.
    HostOnly,
    /// Shared host/device memory such as unified-memory accelerators.
    SharedHostDevice,
    /// Dedicated accelerator memory.
    DedicatedDevice,
}

/// High-level performance class for one advertised execution device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevicePerformanceClass {
    /// Reference path, typically host CPU execution.
    Reference,
    /// Integrated or shared-memory accelerator path.
    IntegratedAccelerator,
    /// Dedicated discrete accelerator path.
    DiscreteAccelerator,
    /// Partitioned accelerator path such as MIG.
    PartitionedAccelerator,
}

/// Reusable inventory qualifiers for compute-market capability surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInventoryQualifiers {
    /// Stable device identifier used for inventory comparisons when available.
    pub stable_device_id: String,
    /// Stable topology key such as PCI BDF when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_key: Option<String>,
    /// High-level performance class for the device.
    pub performance_class: DevicePerformanceClass,
    /// High-level memory posture for the device.
    pub memory_class: DeviceMemoryClass,
    /// Total memory visible to the runtime for this device when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_memory_bytes: Option<u64>,
    /// Currently free memory visible to the runtime for this device when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free_memory_bytes: Option<u64>,
}

/// Stable placement identifier for one device participating in an execution topology.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionDevicePlacement {
    /// Stable device identifier used for cross-run comparison.
    pub stable_device_id: String,
    /// Stable topology key such as PCI BDF when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_key: Option<String>,
    /// Deterministic placement order inside the topology plan.
    pub placement_index: usize,
}

impl ExecutionDevicePlacement {
    /// Creates a stable placement identifier from reusable inventory qualifiers.
    #[must_use]
    pub fn from_inventory(device: &DeviceInventoryQualifiers, placement_index: usize) -> Self {
        Self {
            stable_device_id: device.stable_device_id.clone(),
            topology_key: device.topology_key.clone(),
            placement_index,
        }
    }
}

/// Stable transport class used between cluster nodes for one advertised or realized path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTransportClass {
    /// No cross-node transport is involved.
    LocalOnly,
    /// In-process loopback handoff between colocated components.
    Loopback,
    /// Trusted-LAN datagram transport such as UDP.
    TrustedLanDatagram,
    /// Trusted-LAN stream transport such as TCP or QUIC.
    TrustedLanStream,
    /// Wider-network authenticated stream transport such as public-network TCP or QUIC.
    WiderNetworkStream,
    /// Multiple transport classes participated in the request path.
    Mixed,
}

/// High-level clustered execution posture for one planned or realized path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterExecutionDisposition {
    /// Work remained on the admitting node only.
    LocalOnly,
    /// The entire request was scheduled onto one remote execution node.
    RemoteWholeRequest,
    /// One of multiple serving replicas handled the request.
    ReplicaRouted,
    /// Model execution was partitioned across multiple nodes.
    Sharded,
}

/// Stable policy family whose digest contributed to one cluster decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterPolicyDigestKind {
    /// Admission or namespace policy.
    Admission,
    /// Coordinator term, fence, or commit-authority policy.
    Authority,
    /// Placement and scheduling policy.
    Placement,
    /// Queueing, fairness, cancellation, or backpressure policy.
    Serving,
    /// Replicated residency, warm-state, or load/unload policy.
    Replication,
    /// Shard partitioning, handoff, or transport policy.
    Sharding,
    /// Artifact staging or residency policy.
    Residency,
    /// Catchup, compaction, or recovery policy.
    Recovery,
}

/// One stable policy digest referenced by a clustered execution context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterPolicyDigest {
    /// Stable policy family.
    pub kind: ClusterPolicyDigestKind,
    /// Stable digest for the effective policy instance.
    pub digest: String,
}

impl ClusterPolicyDigest {
    /// Creates a cluster policy digest reference.
    #[must_use]
    pub fn new(kind: ClusterPolicyDigestKind, digest: impl Into<String>) -> Self {
        Self {
            kind,
            digest: digest.into(),
        }
    }
}

/// Explicit coordinator authority truth attached to one clustered execution path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCommitAuthorityEvidence {
    /// Coordinator node that held commit authority for this clustered decision.
    pub coordinator_node_id: String,
    /// Election term that fenced this coordinator authority.
    pub term: u64,
    /// Highest committed authoritative event index visible to that coordinator.
    pub committed_event_index: u64,
    /// Stable fencing token used to distinguish this coordinator epoch from stale ones.
    pub fence_token: String,
    /// Stable digest of the effective coordinator authority record.
    pub authority_digest: String,
}

impl ClusterCommitAuthorityEvidence {
    /// Creates one coordinator-authority evidence record.
    #[must_use]
    pub fn new(
        coordinator_node_id: impl Into<String>,
        term: u64,
        committed_event_index: u64,
        fence_token: impl Into<String>,
        authority_digest: impl Into<String>,
    ) -> Self {
        Self {
            coordinator_node_id: coordinator_node_id.into(),
            term,
            committed_event_index,
            fence_token: fence_token.into(),
            authority_digest: authority_digest.into(),
        }
    }
}

/// Authority scope recorded for one cluster command provenance fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterCommandAuthorityScopeEvidence {
    /// Only the current coordinator may authorize the mutation.
    CoordinatorOnly,
    /// Any allowed cluster member may authorize the command.
    ClusterMember,
    /// Only the targeted node may authorize the command.
    SelfNode,
    /// Only one endpoint of the link may authorize the command.
    LinkPeer,
    /// Only the proposed leader may authorize the command.
    ProposedLeader,
}

/// Admission fact family that one provenance record describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterAdmissionFactKind {
    /// Admission policy truth constrained the scheduled request path.
    AdmissionPolicy,
    /// Scheduler membership admitted the node that made the decision.
    SchedulerMembership,
    /// Selected execution-node membership admitted this worker into the plan.
    SelectedMembership,
    /// Discovery-candidate admission admitted the selected remote worker.
    SelectedCandidateAdmission,
    /// Artifact residency or staging fact admitted this worker/artifact pair.
    ArtifactResidency,
    /// Leadership truth admitted the current coordinator fence and term.
    Leadership,
}

/// Runtime-owned provenance fact derived from one authorized cluster command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCommandProvenanceEvidence {
    /// Admission fact family represented by this record.
    pub fact_kind: ClusterAdmissionFactKind,
    /// Node that submitted the authorizing command.
    pub submitter_node_id: String,
    /// Command authority scope enforced for this provenance fact.
    pub authority_scope: ClusterCommandAuthorityScopeEvidence,
    /// Stable digest of the command payload.
    pub command_digest: String,
    /// Stable digest of the authorization fact.
    pub authorization_digest: String,
    /// Stable digest of the policy used for authorization.
    pub authorization_policy_digest: String,
    /// Node this admission fact is attached to, when the fact is node-scoped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_node_id: Option<String>,
}

impl ClusterCommandProvenanceEvidence {
    /// Creates one runtime-owned cluster-command provenance fact.
    #[must_use]
    pub fn new(
        fact_kind: ClusterAdmissionFactKind,
        submitter_node_id: impl Into<String>,
        authority_scope: ClusterCommandAuthorityScopeEvidence,
        command_digest: impl Into<String>,
        authorization_digest: impl Into<String>,
        authorization_policy_digest: impl Into<String>,
    ) -> Self {
        Self {
            fact_kind,
            submitter_node_id: submitter_node_id.into(),
            authority_scope,
            command_digest: command_digest.into(),
            authorization_digest: authorization_digest.into(),
            authorization_policy_digest: authorization_policy_digest.into(),
            target_node_id: None,
        }
    }

    /// Attaches the node this provenance fact is about, when node-scoped.
    #[must_use]
    pub fn with_target_node_id(mut self, target_node_id: impl Into<String>) -> Self {
        self.target_node_id = Some(target_node_id.into());
        self
    }
}

/// Settlement-facing cluster provenance retained alongside delivery-proof inputs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterSettlementProvenanceInput {
    /// Stable cluster identifier.
    pub cluster_id: String,
    /// Scheduler node that admitted or routed the work.
    pub scheduler_node_id: String,
    /// Stable coordinator authority digest for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinator_authority_digest: Option<String>,
    /// Stable coordinator fence token for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinator_fence_token: Option<String>,
    /// Authorized command provenance facts relevant to this request path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_provenance: Vec<ClusterCommandProvenanceEvidence>,
}

impl ClusterSettlementProvenanceInput {
    /// Builds settlement provenance from one clustered execution context.
    #[must_use]
    pub fn from_cluster_execution(cluster_execution: &ClusterExecutionContext) -> Option<Self> {
        if cluster_execution.command_provenance.is_empty()
            && cluster_execution.commit_authority.is_none()
        {
            return None;
        }
        Some(Self {
            cluster_id: cluster_execution.cluster_id.clone(),
            scheduler_node_id: cluster_execution.scheduler_node_id.clone(),
            coordinator_authority_digest: cluster_execution
                .commit_authority
                .as_ref()
                .map(|authority| authority.authority_digest.clone()),
            coordinator_fence_token: cluster_execution
                .commit_authority
                .as_ref()
                .map(|authority| authority.fence_token.clone()),
            command_provenance: cluster_execution.command_provenance.clone(),
        })
    }
}

/// Artifact readiness posture for one selected cluster node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterArtifactResidencyDisposition {
    /// Required artifacts were already resident on the node.
    Resident,
    /// Admission requires a local copy from a neighboring node.
    CopyRequired,
    /// Admission requires an artifact pull from a backing store.
    PullRequired,
    /// The scheduler refused the node because artifacts were unavailable.
    Refused,
}

/// One explicit cluster node selected or considered for a provider lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterSelectedNode {
    /// Stable node identifier emitted by the cluster control plane.
    pub node_id: String,
    /// Runtime backend promised or delivered on this node.
    pub runtime_backend: String,
    /// Reusable selected-device inventory qualifiers when the scheduler can surface them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_inventory: Option<DeviceInventoryQualifiers>,
    /// Stable selected-device identifier when the scheduler can surface one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stable_device_id: Option<String>,
    /// Stable topology key such as PCI BDF when the scheduler can surface one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_key: Option<String>,
    /// Stable node role when the scheduler can surface it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Stable topology digest attributed to this node when it differs from the cluster aggregate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_digest: Option<String>,
    /// Stable served-artifact digest pinned to this node when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub served_artifact_digest: Option<String>,
    /// Explicit artifact residency posture for the node, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_residency: Option<ClusterArtifactResidencyDisposition>,
}

impl ClusterSelectedNode {
    /// Creates one selected-node record from stable identifiers.
    #[must_use]
    pub fn new(node_id: impl Into<String>, runtime_backend: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            runtime_backend: runtime_backend.into(),
            device_inventory: None,
            stable_device_id: None,
            topology_key: None,
            role: None,
            topology_digest: None,
            served_artifact_digest: None,
            artifact_residency: None,
        }
    }

    /// Attaches reusable selected-device inventory qualifiers.
    #[must_use]
    pub fn with_device_inventory(mut self, device_inventory: DeviceInventoryQualifiers) -> Self {
        self.device_inventory = Some(device_inventory);
        self
    }

    /// Attaches a stable selected-device identifier.
    #[must_use]
    pub fn with_stable_device_id(mut self, stable_device_id: impl Into<String>) -> Self {
        self.stable_device_id = Some(stable_device_id.into());
        self
    }

    /// Attaches a stable topology key.
    #[must_use]
    pub fn with_topology_key(mut self, topology_key: impl Into<String>) -> Self {
        self.topology_key = Some(topology_key.into());
        self
    }

    /// Attaches a stable node role.
    #[must_use]
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Attaches a node-scoped topology digest.
    #[must_use]
    pub fn with_topology_digest(mut self, digest: impl Into<String>) -> Self {
        self.topology_digest = Some(digest.into());
        self
    }

    /// Attaches a served-artifact digest for the node.
    #[must_use]
    pub fn with_served_artifact_digest(mut self, digest: impl Into<String>) -> Self {
        self.served_artifact_digest = Some(digest.into());
        self
    }

    /// Attaches the artifact residency posture for this node.
    #[must_use]
    pub fn with_artifact_residency(
        mut self,
        disposition: ClusterArtifactResidencyDisposition,
    ) -> Self {
        self.artifact_residency = Some(disposition);
        self
    }
}

/// Warm-state truth for one served replica node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterReplicaWarmState {
    /// Replica is not currently loaded.
    Cold,
    /// Replica is loading or warming toward readiness.
    Warming,
    /// Replica is warm and routable.
    Warm,
    /// Replica is draining and should not receive new work.
    Draining,
    /// Replica was explicitly refused for routing.
    Refused,
}

/// Routing posture for one replica node inside a replicated lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterReplicaRoutingDisposition {
    /// Replica is warm and available as standby capacity.
    WarmStandby,
    /// Replica was selected to serve the current request.
    Selected,
    /// Replica was evaluated but refused for routing.
    Refused,
}

/// Explicit replica-node state inside clustered replicated serving.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterReplicaNode {
    /// Stable replica index inside the replicated topology.
    pub replica_index: usize,
    /// Stable node and device identity for the replica.
    pub node: ClusterSelectedNode,
    /// Warm-state truth for the replica.
    pub warm_state: ClusterReplicaWarmState,
    /// Routing posture for the replica within this decision.
    pub routing: ClusterReplicaRoutingDisposition,
    /// Active requests currently admitted on the replica, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_requests: Option<usize>,
    /// Queued requests currently waiting behind active work, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queued_requests: Option<usize>,
    /// Plain-language routing or refusal detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterReplicaNode {
    /// Creates one replica-node entry from stable node identity and routing posture.
    #[must_use]
    pub fn new(
        replica_index: usize,
        node: ClusterSelectedNode,
        warm_state: ClusterReplicaWarmState,
        routing: ClusterReplicaRoutingDisposition,
    ) -> Self {
        Self {
            replica_index,
            node,
            warm_state,
            routing,
            active_requests: None,
            queued_requests: None,
            detail: None,
        }
    }

    /// Attaches active and queued request counts.
    #[must_use]
    pub const fn with_load(mut self, active_requests: usize, queued_requests: usize) -> Self {
        self.active_requests = Some(active_requests);
        self.queued_requests = Some(queued_requests);
        self
    }

    /// Attaches plain-language routing detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Explicit cross-node handoff kind inside a sharded execution lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterShardHandoffKind {
    /// Forward activation state across a shard boundary.
    Activation,
    /// Forward or synchronize KV-cache state across a shard boundary.
    KvCache,
    /// Synchronize tensor-parallel shard state across multiple nodes.
    TensorCollective,
}

/// Explicit cross-node handoff fact inside sharded cluster execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterShardHandoff {
    /// Source shard index.
    pub from_shard_id: usize,
    /// Destination shard index.
    pub to_shard_id: usize,
    /// Source node that emitted the handoff.
    pub from_node_id: String,
    /// Destination node that receives the handoff.
    pub to_node_id: String,
    /// Handoff kind.
    pub kind: ClusterShardHandoffKind,
    /// Transport class used for the handoff.
    pub transport: ClusterTransportClass,
    /// Layer boundary crossed by the handoff.
    pub layer_boundary: usize,
    /// Tensor axis synchronized by the handoff, when this is a tensor-sharded collective.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor_axis: Option<usize>,
    /// Inclusive tensor range start owned by the emitting shard, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor_range_start: Option<usize>,
    /// Exclusive tensor range end owned by the emitting shard, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor_range_end: Option<usize>,
    /// Estimated bytes transferred per token for this handoff.
    pub estimated_bytes_per_token: u64,
    /// Plain-language handoff detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterShardHandoff {
    /// Creates one shard-handoff fact from explicit shard, node, and transfer truth.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        from_shard_id: usize,
        to_shard_id: usize,
        from_node_id: impl Into<String>,
        to_node_id: impl Into<String>,
        kind: ClusterShardHandoffKind,
        transport: ClusterTransportClass,
        layer_boundary: usize,
        estimated_bytes_per_token: u64,
    ) -> Self {
        Self {
            from_shard_id,
            to_shard_id,
            from_node_id: from_node_id.into(),
            to_node_id: to_node_id.into(),
            kind,
            transport,
            layer_boundary,
            tensor_axis: None,
            tensor_range_start: None,
            tensor_range_end: None,
            estimated_bytes_per_token,
            detail: None,
        }
    }

    /// Attaches explicit tensor-axis partition facts for a tensor collective.
    #[must_use]
    pub const fn with_tensor_partition(
        mut self,
        tensor_axis: usize,
        tensor_range_start: usize,
        tensor_range_end: usize,
    ) -> Self {
        self.tensor_axis = Some(tensor_axis);
        self.tensor_range_start = Some(tensor_range_start);
        self.tensor_range_end = Some(tensor_range_end);
        self
    }

    /// Attaches plain-language handoff detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// High-level backend communication class required for one clustered execution lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterCommunicationClass {
    /// Forward the full request to one remote node without cross-node model-state handoff.
    RemoteDispatch,
    /// Route the request through one warm replica lane without model partitioning.
    ReplicaRouting,
    /// Stream stage handoff state across pipeline-parallel boundaries.
    PipelineStageHandoff,
    /// Stream activation or KV state across layer-sharded boundaries.
    LayerShardHandoff,
    /// Exchange mesh-style tensor collectives across tensor shards.
    TensorCollectiveMesh,
}

/// High-level clustered execution lane that may be declared by a backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterExecutionLane {
    /// Whole-request remote dispatch to one executor node.
    RemoteWholeRequest,
    /// Replica-routed serving to one warm lane.
    ReplicaRouted,
    /// Pipeline-parallel execution across ordered stage boundaries.
    PipelineSharded,
    /// Layer-sharded execution with cross-node activation or KV handoff.
    LayerSharded,
    /// Tensor-sharded execution with cross-node collectives.
    TensorSharded,
}

impl ClusterExecutionLane {
    /// Returns a stable machine-checkable name for this clustered lane.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RemoteWholeRequest => "remote_whole_request",
            Self::ReplicaRouted => "replica_routed",
            Self::PipelineSharded => "pipeline_sharded",
            Self::LayerSharded => "layer_sharded",
            Self::TensorSharded => "tensor_sharded",
        }
    }

    /// Returns the communication class required by this clustered lane.
    #[must_use]
    pub const fn required_communication_class(self) -> ClusterCommunicationClass {
        match self {
            Self::RemoteWholeRequest => ClusterCommunicationClass::RemoteDispatch,
            Self::ReplicaRouted => ClusterCommunicationClass::ReplicaRouting,
            Self::PipelineSharded => ClusterCommunicationClass::PipelineStageHandoff,
            Self::LayerSharded => ClusterCommunicationClass::LayerShardHandoff,
            Self::TensorSharded => ClusterCommunicationClass::TensorCollectiveMesh,
        }
    }
}

/// Trust posture for one cluster transport configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTrustPosture {
    /// Shared-admission trusted-LAN posture used for the first shipped scope.
    TrustedLanSharedAdmission,
    /// Authenticated configured-peer posture suitable for operator-managed wider networks.
    AuthenticatedConfiguredPeers,
    /// Attestation-aware configured-peer posture for stronger market-facing admission seams.
    AttestedConfiguredPeers,
}

/// Discovery posture for one cluster transport configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterDiscoveryPosture {
    /// Discovery is limited to local seed peers on the first trusted-LAN seam.
    TrustedLanSeedPeers,
    /// Discovery is limited to explicitly configured operator-managed peers.
    OperatorManagedConfiguredPeers,
    /// A future wider-network discovery posture was requested explicitly.
    ExplicitWiderNetworkRequested,
}

/// Current compute-market trust disposition derived from cluster policy truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterComputeMarketTrustDisposition {
    /// The current cluster trust policy is not sufficient for compute-market claims.
    Refused,
    /// The current cluster trust policy is sufficient for compute-market claims.
    Eligible,
}

/// Explicit refusal reasons for compute-market trust claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterComputeMarketTrustRefusalReason {
    /// Shared admission on a trusted LAN is not a market-safe posture.
    TrustedLanSharedAdmissionOnly,
    /// Wider-network transport still lacks mandatory authenticated messaging.
    MissingAuthenticatedTransport,
    /// Configured peers remain operator-managed allowlist entries instead of market admission.
    OperatorManagedConfiguredPeersOnly,
    /// Node admission is not yet backed by attested node identity.
    MissingAttestedNodeIdentityAdmission,
    /// Discovery posture is not yet explicit for wider non-LAN environments.
    MissingNonLanDiscoveryPosture,
}

/// Machine-checkable assessment for whether a cluster trust policy supports compute-market claims.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterComputeMarketTrustAssessment {
    /// Trust posture active for the underlying cluster policy.
    pub posture: ClusterTrustPosture,
    /// Discovery posture active for the underlying cluster policy.
    pub discovery_posture: ClusterDiscoveryPosture,
    /// Stable digest of the underlying trust policy.
    pub trust_policy_digest: String,
    /// Effective compute-market disposition for the current policy.
    pub disposition: ClusterComputeMarketTrustDisposition,
    /// Explicit reasons why wider compute-market claims remain refused.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusal_reasons: Vec<ClusterComputeMarketTrustRefusalReason>,
}

impl ClusterComputeMarketTrustAssessment {
    /// Returns a stable digest for the current compute-market trust assessment.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_compute_market_trust_assessment|");
        hasher.update(match self.posture {
            ClusterTrustPosture::TrustedLanSharedAdmission => {
                b"trusted_lan_shared_admission".as_slice()
            }
            ClusterTrustPosture::AuthenticatedConfiguredPeers => {
                b"authenticated_configured_peers".as_slice()
            }
            ClusterTrustPosture::AttestedConfiguredPeers => b"attested_configured_peers".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(match self.discovery_posture {
            ClusterDiscoveryPosture::TrustedLanSeedPeers => b"trusted_lan_seed_peers".as_slice(),
            ClusterDiscoveryPosture::OperatorManagedConfiguredPeers => {
                b"operator_managed_configured_peers".as_slice()
            }
            ClusterDiscoveryPosture::ExplicitWiderNetworkRequested => {
                b"explicit_wider_network_requested".as_slice()
            }
        });
        hasher.update(b"|");
        hasher.update(self.trust_policy_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(match self.disposition {
            ClusterComputeMarketTrustDisposition::Refused => b"refused".as_slice(),
            ClusterComputeMarketTrustDisposition::Eligible => b"eligible".as_slice(),
        });
        for refusal_reason in &self.refusal_reasons {
            hasher.update(b"|refusal|");
            hasher.update(match refusal_reason {
                ClusterComputeMarketTrustRefusalReason::TrustedLanSharedAdmissionOnly => {
                    b"trusted_lan_shared_admission_only".as_slice()
                }
                ClusterComputeMarketTrustRefusalReason::MissingAuthenticatedTransport => {
                    b"missing_authenticated_transport".as_slice()
                }
                ClusterComputeMarketTrustRefusalReason::OperatorManagedConfiguredPeersOnly => {
                    b"operator_managed_configured_peers_only".as_slice()
                }
                ClusterComputeMarketTrustRefusalReason::MissingAttestedNodeIdentityAdmission => {
                    b"missing_attested_node_identity_admission".as_slice()
                }
                ClusterComputeMarketTrustRefusalReason::MissingNonLanDiscoveryPosture => {
                    b"missing_non_lan_discovery_posture".as_slice()
                }
            });
        }
        format!("{:x}", hasher.finalize())
    }
}

/// Runtime-owned declared clustered-lane capability contract for one backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterExecutionCapabilityProfile {
    /// Runtime backend this profile applies to.
    pub runtime_backend: String,
    /// Declared clustered execution lanes supported by the backend today.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_lanes: Vec<ClusterExecutionLane>,
    /// Declared serving-semantics contracts for supported clustered lanes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub serving_semantics_capabilities: Vec<ClusterServingSemantics>,
    /// Declared clustered cache-compatibility truth for supported lanes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clustered_cache_capabilities: Vec<ClusterCacheCapability>,
    /// Declared prefill/decode split truth for supported clustered lanes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prefill_decode_capabilities: Vec<ClusterPrefillDecodeCapability>,
    /// Declared communication classes supported by the backend today.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_communication_classes: Vec<ClusterCommunicationClass>,
    /// Plain-language declaration or refusal detail for the current profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterExecutionCapabilityProfile {
    /// Creates a declared cluster capability profile for one runtime backend.
    #[must_use]
    pub fn new(runtime_backend: impl Into<String>) -> Self {
        Self {
            runtime_backend: runtime_backend.into(),
            supported_lanes: Vec::new(),
            serving_semantics_capabilities: Vec::new(),
            clustered_cache_capabilities: Vec::new(),
            prefill_decode_capabilities: Vec::new(),
            supported_communication_classes: Vec::new(),
            detail: None,
        }
    }

    /// Replaces the supported clustered lanes and normalizes the profile.
    #[must_use]
    pub fn with_supported_lanes(mut self, mut supported_lanes: Vec<ClusterExecutionLane>) -> Self {
        supported_lanes.sort_unstable();
        supported_lanes.dedup();
        self.supported_lanes = supported_lanes;
        self.normalize_supported_communication_classes();
        self
    }

    /// Replaces the supported communication classes and normalizes the profile.
    #[must_use]
    pub fn with_supported_communication_classes(
        mut self,
        mut supported_communication_classes: Vec<ClusterCommunicationClass>,
    ) -> Self {
        supported_communication_classes.sort_unstable();
        supported_communication_classes.dedup();
        self.supported_communication_classes = supported_communication_classes;
        self.normalize_supported_communication_classes();
        self
    }

    /// Replaces declared serving-semantics contracts and normalizes by lane.
    #[must_use]
    pub fn with_serving_semantics_capabilities(
        mut self,
        mut serving_semantics_capabilities: Vec<ClusterServingSemantics>,
    ) -> Self {
        serving_semantics_capabilities.sort_by_key(|capability| capability.lane);
        serving_semantics_capabilities.dedup_by_key(|capability| capability.lane);
        self.serving_semantics_capabilities = serving_semantics_capabilities;
        self
    }

    /// Appends one serving-semantics capability declaration.
    #[must_use]
    pub fn with_serving_semantics_capability(
        mut self,
        serving_semantics_capability: ClusterServingSemantics,
    ) -> Self {
        self.serving_semantics_capabilities
            .push(serving_semantics_capability);
        self.serving_semantics_capabilities
            .sort_by_key(|capability| capability.lane);
        self.serving_semantics_capabilities
            .dedup_by_key(|capability| capability.lane);
        self
    }

    /// Replaces declared clustered cache-compatibility truth and normalizes by lane.
    #[must_use]
    pub fn with_clustered_cache_capabilities(
        mut self,
        mut clustered_cache_capabilities: Vec<ClusterCacheCapability>,
    ) -> Self {
        clustered_cache_capabilities.sort_by_key(|capability| capability.lane);
        clustered_cache_capabilities.dedup_by_key(|capability| capability.lane);
        self.clustered_cache_capabilities = clustered_cache_capabilities;
        self
    }

    /// Appends one clustered cache-compatibility declaration.
    #[must_use]
    pub fn with_clustered_cache_capability(
        mut self,
        clustered_cache_capability: ClusterCacheCapability,
    ) -> Self {
        self.clustered_cache_capabilities
            .push(clustered_cache_capability);
        self.clustered_cache_capabilities
            .sort_by_key(|capability| capability.lane);
        self.clustered_cache_capabilities
            .dedup_by_key(|capability| capability.lane);
        self
    }

    /// Replaces declared prefill/decode split truth and normalizes by lane.
    #[must_use]
    pub fn with_prefill_decode_capabilities(
        mut self,
        mut prefill_decode_capabilities: Vec<ClusterPrefillDecodeCapability>,
    ) -> Self {
        prefill_decode_capabilities.sort_by_key(|capability| capability.lane);
        prefill_decode_capabilities.dedup_by_key(|capability| capability.lane);
        self.prefill_decode_capabilities = prefill_decode_capabilities;
        self
    }

    /// Appends one prefill/decode split declaration.
    #[must_use]
    pub fn with_prefill_decode_capability(
        mut self,
        prefill_decode_capability: ClusterPrefillDecodeCapability,
    ) -> Self {
        self.prefill_decode_capabilities
            .push(prefill_decode_capability);
        self.prefill_decode_capabilities
            .sort_by_key(|capability| capability.lane);
        self.prefill_decode_capabilities
            .dedup_by_key(|capability| capability.lane);
        self
    }

    /// Attaches plain-language declaration or refusal detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Returns whether the profile explicitly supports one clustered lane.
    #[must_use]
    pub fn supports_lane(&self, lane: ClusterExecutionLane) -> bool {
        self.supported_lanes.contains(&lane)
    }

    /// Returns declared clustered cache truth for one supported lane, when present.
    #[must_use]
    pub fn clustered_cache_capability(
        &self,
        lane: ClusterExecutionLane,
    ) -> Option<&ClusterCacheCapability> {
        self.clustered_cache_capabilities
            .iter()
            .find(|capability| capability.lane == lane)
    }

    /// Returns declared serving-semantics truth for one supported lane, when present.
    #[must_use]
    pub fn serving_semantics_capability(
        &self,
        lane: ClusterExecutionLane,
    ) -> Option<&ClusterServingSemantics> {
        self.serving_semantics_capabilities
            .iter()
            .find(|capability| capability.lane == lane)
    }

    /// Returns declared prefill/decode split truth for one supported lane, when present.
    #[must_use]
    pub fn prefill_decode_capability(
        &self,
        lane: ClusterExecutionLane,
    ) -> Option<&ClusterPrefillDecodeCapability> {
        self.prefill_decode_capabilities
            .iter()
            .find(|capability| capability.lane == lane)
    }

    /// Returns whether one clustered lane explicitly supports the requested split mode.
    #[must_use]
    pub fn supports_prefill_decode_mode(
        &self,
        lane: ClusterExecutionLane,
        mode: PrefillDecodeExecutionMode,
    ) -> bool {
        self.prefill_decode_capability(lane)
            .map(|capability| capability.supports_mode(mode))
            .unwrap_or(false)
    }

    /// Returns whether the profile explicitly supports one communication class.
    #[must_use]
    pub fn supports_communication_class(
        &self,
        communication_class: ClusterCommunicationClass,
    ) -> bool {
        self.supported_communication_classes
            .contains(&communication_class)
    }

    /// Builds a communication-class eligibility record from the declared profile.
    #[must_use]
    pub fn communication_eligibility(
        &self,
        required_class: ClusterCommunicationClass,
    ) -> ClusterCommunicationEligibility {
        let mut eligibility =
            ClusterCommunicationEligibility::new(self.runtime_backend.clone(), required_class)
                .with_supported_classes(self.supported_communication_classes.clone())
                .with_capability_profile_digest(self.stable_digest());
        if let Some(detail) = &self.detail {
            eligibility = eligibility.with_detail(detail.clone());
        }
        eligibility
    }

    /// Builds a lane-specific eligibility record from the declared profile.
    #[must_use]
    pub fn lane_communication_eligibility(
        &self,
        lane: ClusterExecutionLane,
    ) -> ClusterCommunicationEligibility {
        let mut eligibility = self.communication_eligibility(lane.required_communication_class());
        if !self.supports_lane(lane) {
            eligibility.eligible = false;
            if eligibility.detail.is_none() {
                eligibility = eligibility.with_detail(format!(
                    "backend `{}` does not declare `{}` clustered lane support",
                    self.runtime_backend,
                    lane.as_str()
                ));
            }
        }
        eligibility
    }

    /// Returns a stable digest for the declared clustered capability profile.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_execution_capability_profile|");
        hasher.update(self.runtime_backend.as_bytes());
        for lane in &self.supported_lanes {
            hasher.update(b"|lane|");
            hasher.update(match lane {
                ClusterExecutionLane::RemoteWholeRequest => b"remote_whole_request".as_slice(),
                ClusterExecutionLane::ReplicaRouted => b"replica_routed".as_slice(),
                ClusterExecutionLane::PipelineSharded => b"pipeline_sharded".as_slice(),
                ClusterExecutionLane::LayerSharded => b"layer_sharded".as_slice(),
                ClusterExecutionLane::TensorSharded => b"tensor_sharded".as_slice(),
            });
        }
        for cache_capability in &self.clustered_cache_capabilities {
            hasher.update(b"|clustered_cache|");
            hasher.update(cache_capability.lane.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(cache_capability.prefix_scope.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(cache_capability.kv_scope.as_str().as_bytes());
            for tier in &cache_capability.supported_residency_tiers {
                hasher.update(b"|tier|");
                hasher.update(tier.as_str().as_bytes());
            }
            hasher.update(b"|");
            hasher.update(if cache_capability.invalidates_on_route_change {
                b"route_change".as_slice()
            } else {
                b"route_stable".as_slice()
            });
            hasher.update(b"|");
            hasher.update(if cache_capability.invalidates_on_topology_change {
                b"topology_change".as_slice()
            } else {
                b"topology_stable".as_slice()
            });
            if let Some(detail) = &cache_capability.detail {
                hasher.update(b"|detail|");
                hasher.update(detail.as_bytes());
            }
        }
        for serving_semantics in &self.serving_semantics_capabilities {
            hasher.update(b"|serving_semantics|");
            hasher.update(serving_semantics.stable_digest().as_bytes());
        }
        for prefill_decode_capability in &self.prefill_decode_capabilities {
            hasher.update(b"|prefill_decode|");
            hasher.update(prefill_decode_capability.lane.as_str().as_bytes());
            for mode in &prefill_decode_capability.capability.supported_modes {
                hasher.update(b"|mode|");
                hasher.update(mode.as_str().as_bytes());
            }
            for transport in &prefill_decode_capability.capability.supported_transports {
                hasher.update(b"|transport|");
                hasher.update(transport.as_str().as_bytes());
            }
            hasher.update(b"|metrics|");
            hasher.update(
                if prefill_decode_capability.capability.exposes_split_metrics {
                    b"split_metrics".as_slice()
                } else {
                    b"no_split_metrics".as_slice()
                },
            );
            if let Some(detail) = &prefill_decode_capability.capability.detail {
                hasher.update(b"|detail|");
                hasher.update(detail.as_bytes());
            }
        }
        for communication_class in &self.supported_communication_classes {
            hasher.update(b"|communication_class|");
            hasher.update(match communication_class {
                ClusterCommunicationClass::RemoteDispatch => b"remote_dispatch".as_slice(),
                ClusterCommunicationClass::ReplicaRouting => b"replica_routing".as_slice(),
                ClusterCommunicationClass::PipelineStageHandoff => {
                    b"pipeline_stage_handoff".as_slice()
                }
                ClusterCommunicationClass::LayerShardHandoff => b"layer_shard_handoff".as_slice(),
                ClusterCommunicationClass::TensorCollectiveMesh => {
                    b"tensor_collective_mesh".as_slice()
                }
            });
        }
        if let Some(detail) = &self.detail {
            hasher.update(b"|detail|");
            hasher.update(detail.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    fn normalize_supported_communication_classes(&mut self) {
        let mut supported_communication_classes = self.supported_communication_classes.clone();
        supported_communication_classes.extend(
            self.supported_lanes
                .iter()
                .map(|lane| lane.required_communication_class()),
        );
        supported_communication_classes.sort_unstable();
        supported_communication_classes.dedup();
        self.supported_communication_classes = supported_communication_classes;
    }
}

/// Truthful execution mode for prompt-prefill and decode work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefillDecodeExecutionMode {
    /// Prompt processing and decode remain one inseparable runtime lane.
    Monolithic,
    /// Prefill and decode are split but remain co-located inside one runtime owner.
    DisaggregatedColocated,
    /// Prefill and decode are split across an explicit KV-transfer boundary.
    DisaggregatedKvTransfer,
}

impl PrefillDecodeExecutionMode {
    /// Returns a stable machine-checkable name for this execution mode.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Monolithic => "monolithic",
            Self::DisaggregatedColocated => "disaggregated_colocated",
            Self::DisaggregatedKvTransfer => "disaggregated_kv_transfer",
        }
    }
}

/// Truthful transport or ownership seam between prefill and decode phases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefillDecodeTransport {
    /// Prefill hands a cache view directly to decode inside one process/runtime boundary.
    InProcessKvState,
    /// Prefill hands a cache image through a host-memory seam.
    SharedHostMemory,
    /// Prefill hands a cache image through a local IPC seam.
    LocalIpc,
    /// Prefill hands a cache image through one explicit cluster transport boundary.
    ClusterTransport,
}

impl PrefillDecodeTransport {
    /// Returns a stable machine-checkable name for this transport.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InProcessKvState => "in_process_kv_state",
            Self::SharedHostMemory => "shared_host_memory",
            Self::LocalIpc => "local_ipc",
            Self::ClusterTransport => "cluster_transport",
        }
    }
}

/// Capability truth for prompt-prefill and decode split behavior.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefillDecodeCapability {
    /// Supported prefill/decode execution modes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_modes: Vec<PrefillDecodeExecutionMode>,
    /// Supported handoff transports or ownership seams.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_transports: Vec<PrefillDecodeTransport>,
    /// Whether the runtime can surface TTFT and ITL separately.
    pub exposes_split_metrics: bool,
    /// Optional plain-language detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl PrefillDecodeCapability {
    /// Monolithic-only capability with no explicit split metrics.
    #[must_use]
    pub fn monolithic_only() -> Self {
        Self {
            supported_modes: vec![PrefillDecodeExecutionMode::Monolithic],
            supported_transports: Vec::new(),
            exposes_split_metrics: false,
            detail: None,
        }
    }

    /// Co-located split capability with separate TTFT and ITL metrics.
    #[must_use]
    pub fn colocated_split() -> Self {
        Self {
            supported_modes: vec![PrefillDecodeExecutionMode::DisaggregatedColocated],
            supported_transports: vec![PrefillDecodeTransport::InProcessKvState],
            exposes_split_metrics: true,
            detail: None,
        }
    }

    /// KV-transfer split capability with separate TTFT and ITL metrics.
    #[must_use]
    pub fn kv_transfer_split() -> Self {
        Self {
            supported_modes: vec![PrefillDecodeExecutionMode::DisaggregatedKvTransfer],
            supported_transports: vec![PrefillDecodeTransport::ClusterTransport],
            exposes_split_metrics: true,
            detail: None,
        }
    }

    /// Returns whether one execution mode is supported.
    #[must_use]
    pub fn supports_mode(&self, mode: PrefillDecodeExecutionMode) -> bool {
        self.supported_modes.contains(&mode)
    }

    /// Attaches plain-language detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Cluster-lane-specific prompt-prefill/decode split truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterPrefillDecodeCapability {
    /// Clustered lane this split truth applies to.
    pub lane: ClusterExecutionLane,
    /// Capability facts for this lane.
    pub capability: PrefillDecodeCapability,
}

impl ClusterPrefillDecodeCapability {
    /// Creates split-capability truth for one clustered lane.
    #[must_use]
    pub fn new(lane: ClusterExecutionLane, capability: PrefillDecodeCapability) -> Self {
        Self { lane, capability }
    }

    /// Returns whether one execution mode is supported on this lane.
    #[must_use]
    pub fn supports_mode(&self, mode: PrefillDecodeExecutionMode) -> bool {
        self.capability.supports_mode(mode)
    }
}

/// Explicit handoff facts between prefill and decode phases.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefillDecodeHandoff {
    /// Truthful execution mode for the realized phase split.
    pub mode: PrefillDecodeExecutionMode,
    /// Truthful transport or ownership seam used for the handoff.
    pub transport: PrefillDecodeTransport,
    /// Source node that produced the prefill state, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_node_id: Option<String>,
    /// Target node that consumed the decode state, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_node_id: Option<String>,
    /// KV pages visible at the prefill/decode boundary.
    pub kv_pages: usize,
    /// KV bytes visible at the prefill/decode boundary.
    pub kv_bytes: u64,
    /// Handoff latency in nanoseconds, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handoff_latency_ns: Option<u64>,
    /// Optional plain-language detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl PrefillDecodeHandoff {
    /// Creates a co-located prefill/decode handoff over one in-process KV seam.
    #[must_use]
    pub fn colocated(kv_pages: usize, kv_bytes: u64) -> Self {
        Self {
            mode: PrefillDecodeExecutionMode::DisaggregatedColocated,
            transport: PrefillDecodeTransport::InProcessKvState,
            source_node_id: None,
            target_node_id: None,
            kv_pages,
            kv_bytes,
            handoff_latency_ns: None,
            detail: None,
        }
    }

    /// Creates a KV-transfer handoff between two explicit nodes.
    #[must_use]
    pub fn kv_transfer(
        source_node_id: impl Into<String>,
        target_node_id: impl Into<String>,
        kv_pages: usize,
        kv_bytes: u64,
        handoff_latency_ns: Option<u64>,
    ) -> Self {
        Self {
            mode: PrefillDecodeExecutionMode::DisaggregatedKvTransfer,
            transport: PrefillDecodeTransport::ClusterTransport,
            source_node_id: Some(source_node_id.into()),
            target_node_id: Some(target_node_id.into()),
            kv_pages,
            kv_bytes,
            handoff_latency_ns,
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

/// Explicit backend communication-class eligibility used by cluster planning and evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCommunicationEligibility {
    /// Runtime backend the eligibility applies to.
    pub runtime_backend: String,
    /// Communication class required for this clustered path.
    pub required_class: ClusterCommunicationClass,
    /// Communication classes the backend truthfully supports today.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_classes: Vec<ClusterCommunicationClass>,
    /// Stable digest of the declared capability profile the eligibility was derived from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_profile_digest: Option<String>,
    /// Whether the backend currently satisfies the required class.
    pub eligible: bool,
    /// Plain-language detail describing the eligibility or refusal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterCommunicationEligibility {
    /// Creates one communication-class eligibility record for a runtime backend.
    #[must_use]
    pub fn new(
        runtime_backend: impl Into<String>,
        required_class: ClusterCommunicationClass,
    ) -> Self {
        Self {
            runtime_backend: runtime_backend.into(),
            required_class,
            supported_classes: Vec::new(),
            capability_profile_digest: None,
            eligible: false,
            detail: None,
        }
    }

    /// Replaces the supported communication-class set and updates eligibility.
    #[must_use]
    pub fn with_supported_classes(
        mut self,
        mut supported_classes: Vec<ClusterCommunicationClass>,
    ) -> Self {
        supported_classes.sort_unstable();
        supported_classes.dedup();
        self.eligible = supported_classes.contains(&self.required_class);
        self.supported_classes = supported_classes;
        self
    }

    /// Attaches the stable digest of the declared capability profile.
    #[must_use]
    pub fn with_capability_profile_digest(
        mut self,
        capability_profile_digest: impl Into<String>,
    ) -> Self {
        self.capability_profile_digest = Some(capability_profile_digest.into());
        self
    }

    /// Attaches plain-language eligibility or refusal detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Builds a communication-class eligibility record from a declared capability profile.
    #[must_use]
    pub fn from_capability_profile(
        capability_profile: &ClusterExecutionCapabilityProfile,
        required_class: ClusterCommunicationClass,
    ) -> Self {
        capability_profile.communication_eligibility(required_class)
    }

    /// Builds a lane-specific eligibility record from a declared capability profile.
    #[must_use]
    pub fn from_capability_profile_lane(
        capability_profile: &ClusterExecutionCapabilityProfile,
        lane: ClusterExecutionLane,
    ) -> Self {
        capability_profile.lane_communication_eligibility(lane)
    }
}

/// Stable scheduler fallback reason for one clustered execution path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterFallbackReason {
    /// The candidate node failed admission policy.
    AdmissionRefused,
    /// The candidate node lacked required artifacts.
    ArtifactUnavailable,
    /// The candidate node or backend was degraded.
    BackendDegraded,
    /// The candidate node was rerouted because queue or admission pressure was too high.
    QueueBackpressure,
    /// The candidate node was rerouted because decode fairness reserved capacity.
    DecodeFairness,
    /// The candidate node was rerouted because its observed service health was too slow.
    SlowNodeBackpressure,
    /// The candidate node became unavailable.
    NodeUnavailable,
    /// The candidate node no longer matched required topology facts.
    TopologyMismatch,
    /// Cross-node transport degraded below policy.
    TransportDegraded,
}

/// One explicit scheduler fallback transition for a clustered execution path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterFallbackStep {
    /// Preferred source node or previously selected node, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_node_id: Option<String>,
    /// Replacement or final selected node.
    pub to_node_id: String,
    /// Stable scheduler fallback reason.
    pub reason: ClusterFallbackReason,
    /// Optional plain-language detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterFallbackStep {
    /// Creates one scheduler fallback transition.
    #[must_use]
    pub fn new(to_node_id: impl Into<String>, reason: ClusterFallbackReason) -> Self {
        Self {
            from_node_id: None,
            to_node_id: to_node_id.into(),
            reason,
            detail: None,
        }
    }

    /// Attaches the previously selected node.
    #[must_use]
    pub fn from_node(mut self, node_id: impl Into<String>) -> Self {
        self.from_node_id = Some(node_id.into());
        self
    }

    /// Attaches plain-language fallback detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Ordered role of one stage in a pipeline-parallel plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterPipelineStageRole {
    /// Entry stage that receives prompt input first.
    Entry,
    /// Middle stage that relays intermediate state onward.
    Middle,
    /// Exit stage that produces the final decode output.
    Exit,
}

/// Machine-checkable timing and transport facts for one pipeline stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterPipelineStage {
    /// Stable stage index inside the pipeline.
    pub stage_index: usize,
    /// Node that owns this stage.
    pub node_id: String,
    /// High-level role of the stage in the pipeline.
    pub role: ClusterPipelineStageRole,
    /// Inclusive starting layer index.
    pub start_layer: usize,
    /// Exclusive ending layer index.
    pub end_layer: usize,
    /// Estimated startup cost before this stage is warm enough to serve.
    pub startup_cost_ms: u64,
    /// Estimated prompt/prefill latency attributable to this stage.
    pub prefill_latency_ms: u64,
    /// Estimated per-token decode latency attributable to this stage.
    pub decode_latency_ms: u64,
    /// Transport used when handing off to the next stage, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handoff_transport: Option<ClusterTransportClass>,
    /// Median or configured handoff latency to the next stage, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handoff_latency_ms: Option<u64>,
    /// Observed or configured handoff bandwidth to the next stage, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handoff_bandwidth_mbps: Option<u64>,
    /// Optional plain-language stage detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterPipelineStage {
    /// Creates one pipeline-stage record from explicit timing and layer bounds.
    #[must_use]
    pub fn new(
        stage_index: usize,
        node_id: impl Into<String>,
        role: ClusterPipelineStageRole,
        start_layer: usize,
        end_layer: usize,
        startup_cost_ms: u64,
        prefill_latency_ms: u64,
        decode_latency_ms: u64,
    ) -> Self {
        Self {
            stage_index,
            node_id: node_id.into(),
            role,
            start_layer,
            end_layer,
            startup_cost_ms,
            prefill_latency_ms,
            decode_latency_ms,
            handoff_transport: None,
            handoff_latency_ms: None,
            handoff_bandwidth_mbps: None,
            detail: None,
        }
    }

    /// Attaches handoff transport facts to the next stage.
    #[must_use]
    pub fn with_handoff(
        mut self,
        handoff_transport: ClusterTransportClass,
        handoff_latency_ms: Option<u64>,
        handoff_bandwidth_mbps: Option<u64>,
    ) -> Self {
        self.handoff_transport = Some(handoff_transport);
        self.handoff_latency_ms = handoff_latency_ms;
        self.handoff_bandwidth_mbps = handoff_bandwidth_mbps;
        self
    }

    /// Attaches plain-language stage detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Truthful locality scope for reusable clustered cache state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterCacheScope {
    /// The clustered lane cannot truthfully promise reusable state.
    Unavailable,
    /// Reuse is only truthful when the request returns to one specific remote node.
    RequestNodeLocal,
    /// Reuse is only truthful on one warm replica identity.
    ReplicaLocal,
    /// Reuse is only truthful when every stage remains pinned to the same topology.
    StageLocal,
    /// Reuse remains truthful across the whole clustered lane.
    ClusterWide,
}

impl ClusterCacheScope {
    /// Returns a stable machine-checkable name for this scope.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::RequestNodeLocal => "request_node_local",
            Self::ReplicaLocal => "replica_local",
            Self::StageLocal => "stage_local",
            Self::ClusterWide => "cluster_wide",
        }
    }
}

/// Advertised clustered cache-compatibility truth for one clustered lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCacheCapability {
    /// Clustered lane this cache truth applies to.
    pub lane: ClusterExecutionLane,
    /// Truthful prefix-cache scope for the lane.
    pub prefix_scope: ClusterCacheScope,
    /// Truthful KV-state scope for the lane.
    pub kv_scope: ClusterCacheScope,
    /// Residency tiers that the lane can surface truthfully.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_residency_tiers: Vec<KvResidencyTier>,
    /// Whether route changes invalidate otherwise compatible state.
    pub invalidates_on_route_change: bool,
    /// Whether topology changes invalidate otherwise compatible state.
    pub invalidates_on_topology_change: bool,
    /// Optional plain-language detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterCacheCapability {
    /// Creates clustered cache-compatibility truth for one lane.
    #[must_use]
    pub fn new(
        lane: ClusterExecutionLane,
        prefix_scope: ClusterCacheScope,
        kv_scope: ClusterCacheScope,
    ) -> Self {
        Self {
            lane,
            prefix_scope,
            kv_scope,
            supported_residency_tiers: Vec::new(),
            invalidates_on_route_change: false,
            invalidates_on_topology_change: false,
            detail: None,
        }
    }

    /// Attaches the residency tiers the lane can surface truthfully.
    #[must_use]
    pub fn with_residency_tiers(
        mut self,
        mut supported_residency_tiers: Vec<KvResidencyTier>,
    ) -> Self {
        supported_residency_tiers.sort_unstable();
        supported_residency_tiers.dedup();
        self.supported_residency_tiers = supported_residency_tiers;
        self
    }

    /// Appends one supported residency tier.
    #[must_use]
    pub fn with_residency_tier(mut self, supported_residency_tier: KvResidencyTier) -> Self {
        self.supported_residency_tiers
            .push(supported_residency_tier);
        self.supported_residency_tiers.sort_unstable();
        self.supported_residency_tiers.dedup();
        self
    }

    /// Marks route changes as invalidating otherwise compatible state.
    #[must_use]
    pub const fn invalidates_on_route_change(mut self) -> Self {
        self.invalidates_on_route_change = true;
        self
    }

    /// Marks topology changes as invalidating otherwise compatible state.
    #[must_use]
    pub const fn invalidates_on_topology_change(mut self) -> Self {
        self.invalidates_on_topology_change = true;
        self
    }

    /// Attaches plain-language capability detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Realized clustered cache usage and invalidation posture for one request path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCacheUsage {
    /// Clustered lane this request realized.
    pub lane: ClusterExecutionLane,
    /// Truthful prefix-cache scope for the realized path.
    pub prefix_scope: ClusterCacheScope,
    /// Truthful KV-state scope for the realized path.
    pub kv_scope: ClusterCacheScope,
    /// Action taken for prefix reuse under clustered routing truth.
    pub prefix_action: CacheAction,
    /// Action taken for KV reuse under clustered routing truth.
    pub kv_action: CacheAction,
    /// Trigger that invalidated clustered cache truth when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalidation_trigger: Option<CacheInvalidationTrigger>,
    /// Optional plain-language detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterCacheUsage {
    /// Creates realized clustered cache usage from explicit scope and actions.
    #[must_use]
    pub fn new(
        lane: ClusterExecutionLane,
        prefix_scope: ClusterCacheScope,
        kv_scope: ClusterCacheScope,
        prefix_action: CacheAction,
        kv_action: CacheAction,
    ) -> Self {
        Self {
            lane,
            prefix_scope,
            kv_scope,
            prefix_action,
            kv_action,
            invalidation_trigger: None,
            detail: None,
        }
    }

    /// Attaches the invalidation trigger that changed clustered cache truth.
    #[must_use]
    pub const fn with_invalidation_trigger(
        mut self,
        invalidation_trigger: CacheInvalidationTrigger,
    ) -> Self {
        self.invalidation_trigger = Some(invalidation_trigger);
        self
    }

    /// Attaches plain-language usage detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Warm-route posture that constrains truthful clustered cache or route reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterWarmRoutePosture {
    /// The lane only requires selection of one ready node; warm-route pinning is not promised.
    ReadyNodeSelection,
    /// The lane requires the same warm route identity to preserve truthful reuse.
    RoutePinned,
    /// The lane requires the same shard/stage topology to preserve truthful reuse.
    TopologyPinned,
}

impl ClusterWarmRoutePosture {
    /// Returns a stable machine-checkable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadyNodeSelection => "ready_node_selection",
            Self::RoutePinned => "route_pinned",
            Self::TopologyPinned => "topology_pinned",
        }
    }
}

/// Canonical serving-semantics contract shared by local and clustered execution surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterServingSemantics {
    /// Clustered lane this serving contract applies to.
    pub lane: ClusterExecutionLane,
    /// Canonical execution-profile contract reused from local serving.
    pub execution_profile: ExecutionCapabilityProfile,
    /// Warm-route posture that constrains truthful reuse for the lane.
    pub warm_route_posture: ClusterWarmRoutePosture,
    /// Optional plain-language detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterServingSemantics {
    /// Creates a lane-scoped serving-semantics contract.
    #[must_use]
    pub fn new(
        lane: ClusterExecutionLane,
        execution_profile: ExecutionCapabilityProfile,
        warm_route_posture: ClusterWarmRoutePosture,
    ) -> Self {
        Self {
            lane,
            execution_profile,
            warm_route_posture,
            detail: None,
        }
    }

    /// Attaches plain-language detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Returns a stable digest for the serving-semantics contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_serving_semantics|");
        hasher.update(self.lane.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(self.execution_profile.stable_digest().as_bytes());
        hasher.update(b"|");
        hasher.update(self.warm_route_posture.as_str().as_bytes());
        if let Some(detail) = &self.detail {
            hasher.update(b"|detail|");
            hasher.update(detail.as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// Runtime-owned clustered execution evidence shared by capability, provenance, and receipt types.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterExecutionContext {
    /// Stable cluster identifier.
    pub cluster_id: String,
    /// Stable digest of the authoritative cluster-state snapshot.
    pub cluster_state_digest: String,
    /// Stable digest of the cluster topology facts used for the decision.
    pub topology_digest: String,
    /// Stable digest of artifact residency facts used for the decision, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_residency_digest: Option<String>,
    /// Stable digest of replica warm-state facts used for replicated routing, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replica_state_digest: Option<String>,
    /// Stable digest of the sharded-model manifest constraining this execution path, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharded_model_manifest_digest: Option<String>,
    /// Explicit coordinator authority that fenced this cluster decision, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_authority: Option<ClusterCommitAuthorityEvidence>,
    /// Stable node identifier that admitted or scheduled the work.
    pub scheduler_node_id: String,
    /// Transport class used across the request path.
    pub transport: ClusterTransportClass,
    /// High-level cluster execution posture.
    pub disposition: ClusterExecutionDisposition,
    /// Explicit cluster-owned execution topology when it differs from the local backend view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_topology: Option<ExecutionTopologyPlan>,
    /// Stable policy digests that constrained the decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_digests: Vec<ClusterPolicyDigest>,
    /// Explicit backend communication-class eligibility for this clustered path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub communication_eligibility: Option<ClusterCommunicationEligibility>,
    /// Explicit placement diagnostics that explain optional external hint intake.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placement_diagnostics: Vec<String>,
    /// Explicit selected nodes for the planned or realized path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_nodes: Vec<ClusterSelectedNode>,
    /// Explicit replica-node state when the served lane is replicated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replica_nodes: Vec<ClusterReplicaNode>,
    /// Explicit cross-node activation or KV handoffs for sharded execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shard_handoffs: Vec<ClusterShardHandoff>,
    /// Explicit stage timing and transport facts for pipeline-parallel execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pipeline_stages: Vec<ClusterPipelineStage>,
    /// Explicit prefill/decode split seam for the realized clustered path, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefill_decode_handoff: Option<PrefillDecodeHandoff>,
    /// Canonical serving-semantics contract for the realized clustered lane, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serving_semantics: Option<ClusterServingSemantics>,
    /// Realized clustered cache-compatibility and invalidation truth for the request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clustered_cache_usage: Option<ClusterCacheUsage>,
    /// Authorized command provenance facts that admitted or fenced this request path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_provenance: Vec<ClusterCommandProvenanceEvidence>,
    /// Explicit fallback history when the scheduler rerouted work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallback_history: Vec<ClusterFallbackStep>,
    /// Plain-language degraded-routing reason when the path was not the ideal placement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
    /// Optional training recovery posture when the clustered lane is training-class.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_recovery: Option<TrainingRecoveryContext>,
    /// Optional training collective and device-mesh posture for training-class clustered execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_collective: Option<TrainingCollectiveContext>,
}

impl ClusterExecutionContext {
    /// Creates a clustered execution context from stable digests and routing facts.
    #[must_use]
    pub fn new(
        cluster_id: impl Into<String>,
        cluster_state_digest: impl Into<String>,
        topology_digest: impl Into<String>,
        scheduler_node_id: impl Into<String>,
        transport: ClusterTransportClass,
        disposition: ClusterExecutionDisposition,
    ) -> Self {
        Self {
            cluster_id: cluster_id.into(),
            cluster_state_digest: cluster_state_digest.into(),
            topology_digest: topology_digest.into(),
            artifact_residency_digest: None,
            replica_state_digest: None,
            sharded_model_manifest_digest: None,
            commit_authority: None,
            scheduler_node_id: scheduler_node_id.into(),
            transport,
            disposition,
            execution_topology: None,
            policy_digests: Vec::new(),
            communication_eligibility: None,
            placement_diagnostics: Vec::new(),
            selected_nodes: Vec::new(),
            replica_nodes: Vec::new(),
            shard_handoffs: Vec::new(),
            pipeline_stages: Vec::new(),
            prefill_decode_handoff: None,
            serving_semantics: None,
            clustered_cache_usage: None,
            command_provenance: Vec::new(),
            fallback_history: Vec::new(),
            degraded_reason: None,
            training_recovery: None,
            training_collective: None,
        }
    }

    /// Attaches an artifact residency digest.
    #[must_use]
    pub fn with_artifact_residency_digest(mut self, digest: impl Into<String>) -> Self {
        self.artifact_residency_digest = Some(digest.into());
        self
    }

    /// Attaches a replica-state digest.
    #[must_use]
    pub fn with_replica_state_digest(mut self, digest: impl Into<String>) -> Self {
        self.replica_state_digest = Some(digest.into());
        self
    }

    /// Attaches a sharded-model manifest digest.
    #[must_use]
    pub fn with_sharded_model_manifest_digest(mut self, digest: impl Into<String>) -> Self {
        self.sharded_model_manifest_digest = Some(digest.into());
        self
    }

    /// Attaches explicit coordinator-authority evidence.
    #[must_use]
    pub fn with_commit_authority(
        mut self,
        commit_authority: ClusterCommitAuthorityEvidence,
    ) -> Self {
        self.commit_authority = Some(commit_authority);
        self
    }

    /// Attaches explicit cluster-owned execution topology.
    #[must_use]
    pub fn with_execution_topology(mut self, execution_topology: ExecutionTopologyPlan) -> Self {
        self.execution_topology = Some(execution_topology);
        self
    }

    /// Appends one policy digest reference.
    #[must_use]
    pub fn with_policy_digest(mut self, policy_digest: ClusterPolicyDigest) -> Self {
        self.policy_digests.push(policy_digest);
        self
    }

    /// Attaches explicit backend communication-class eligibility truth.
    #[must_use]
    pub fn with_communication_eligibility(
        mut self,
        communication_eligibility: ClusterCommunicationEligibility,
    ) -> Self {
        self.communication_eligibility = Some(communication_eligibility);
        self
    }

    /// Replaces placement diagnostics associated with optional external hints.
    #[must_use]
    pub fn with_placement_diagnostics(mut self, placement_diagnostics: Vec<String>) -> Self {
        self.placement_diagnostics = placement_diagnostics;
        self
    }

    /// Appends one placement diagnostic associated with optional external hints.
    #[must_use]
    pub fn with_placement_diagnostic(mut self, placement_diagnostic: impl Into<String>) -> Self {
        self.placement_diagnostics.push(placement_diagnostic.into());
        self
    }

    /// Replaces the explicit selected-node set.
    #[must_use]
    pub fn with_selected_nodes(mut self, selected_nodes: Vec<ClusterSelectedNode>) -> Self {
        self.selected_nodes = selected_nodes;
        self
    }

    /// Replaces the explicit replica-node set.
    #[must_use]
    pub fn with_replica_nodes(mut self, replica_nodes: Vec<ClusterReplicaNode>) -> Self {
        self.replica_nodes = replica_nodes;
        self
    }

    /// Replaces the explicit shard-handoff set.
    #[must_use]
    pub fn with_shard_handoffs(mut self, shard_handoffs: Vec<ClusterShardHandoff>) -> Self {
        self.shard_handoffs = shard_handoffs;
        self
    }

    /// Replaces the explicit pipeline-stage set.
    #[must_use]
    pub fn with_pipeline_stages(mut self, pipeline_stages: Vec<ClusterPipelineStage>) -> Self {
        self.pipeline_stages = pipeline_stages;
        self
    }

    /// Attaches explicit prefill/decode split handoff truth.
    #[must_use]
    pub fn with_prefill_decode_handoff(
        mut self,
        prefill_decode_handoff: PrefillDecodeHandoff,
    ) -> Self {
        self.prefill_decode_handoff = Some(prefill_decode_handoff);
        self
    }

    /// Attaches canonical serving-semantics truth for the realized clustered lane.
    #[must_use]
    pub fn with_serving_semantics(mut self, serving_semantics: ClusterServingSemantics) -> Self {
        self.serving_semantics = Some(serving_semantics);
        self
    }

    /// Attaches realized clustered cache usage for the request path.
    #[must_use]
    pub fn with_clustered_cache_usage(mut self, clustered_cache_usage: ClusterCacheUsage) -> Self {
        self.clustered_cache_usage = Some(clustered_cache_usage);
        self
    }

    /// Replaces the command-provenance set for this clustered execution path.
    #[must_use]
    pub fn with_command_provenance(
        mut self,
        command_provenance: Vec<ClusterCommandProvenanceEvidence>,
    ) -> Self {
        self.command_provenance = command_provenance;
        self
    }

    /// Appends one command-provenance fact.
    #[must_use]
    pub fn with_command_provenance_fact(
        mut self,
        command_provenance: ClusterCommandProvenanceEvidence,
    ) -> Self {
        self.command_provenance.push(command_provenance);
        self
    }

    /// Appends one fallback transition.
    #[must_use]
    pub fn with_fallback(mut self, fallback: ClusterFallbackStep) -> Self {
        self.fallback_history.push(fallback);
        self
    }

    /// Attaches a degraded-routing reason.
    #[must_use]
    pub fn with_degraded_reason(mut self, degraded_reason: impl Into<String>) -> Self {
        self.degraded_reason = Some(degraded_reason.into());
        self
    }

    /// Attaches training recovery posture for training-class clustered execution.
    #[must_use]
    pub fn with_training_recovery(mut self, training_recovery: TrainingRecoveryContext) -> Self {
        self.training_recovery = Some(training_recovery);
        self
    }

    /// Attaches training collective and device-mesh posture for training-class clustered execution.
    #[must_use]
    pub fn with_training_collective(
        mut self,
        training_collective: TrainingCollectiveContext,
    ) -> Self {
        self.training_collective = Some(training_collective);
        self
    }

    /// Returns the best available device inventory view surfaced by cluster execution evidence.
    #[must_use]
    pub fn selected_devices_inventory(&self) -> Vec<DeviceInventoryQualifiers> {
        let replica_devices = self
            .replica_nodes
            .iter()
            .filter_map(|replica| replica.node.device_inventory.clone())
            .collect::<Vec<_>>();
        if !replica_devices.is_empty() {
            return replica_devices;
        }
        self.selected_nodes
            .iter()
            .filter_map(|node| node.device_inventory.clone())
            .collect()
    }
}

/// High-level topology mode for one compiled or advertised execution path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTopologyKind {
    /// The whole model executes on one concrete device.
    SingleDevice,
    /// Multiple devices each hold a full replica of the executable state.
    Replicated,
    /// Model layers are partitioned into ordered pipeline stages.
    PipelineSharded,
    /// Model layers are partitioned across multiple devices.
    LayerSharded,
    /// One tensor axis is partitioned across multiple devices.
    TensorSharded,
}

/// One logical partition assigned to a concrete device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionPartition {
    /// One device owns the full executable state.
    WholeModel,
    /// One replica of a replicated plan.
    Replica {
        /// Stable replica index inside the topology plan.
        replica_index: usize,
    },
    /// A contiguous block of layers owned by one device.
    LayerRange {
        /// Inclusive starting layer index.
        start_layer: usize,
        /// Exclusive ending layer index.
        end_layer: usize,
    },
    /// A contiguous logical slice of one tensor axis.
    TensorRange {
        /// Tensor axis being partitioned.
        axis: usize,
        /// Inclusive starting logical element.
        start: usize,
        /// Exclusive ending logical element.
        end: usize,
    },
}

/// One stable shard or placement assignment inside an execution topology.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionShardAssignment {
    /// Stable shard identifier within the topology plan.
    pub shard_id: usize,
    /// Concrete device placement that owns this partition.
    pub device: ExecutionDevicePlacement,
    /// Logical model or tensor partition mapped to that device.
    pub partition: ExecutionPartition,
}

/// Explicit multi-device or sharded execution topology.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTopologyPlan {
    /// Backend that will execute this topology.
    pub effective_backend: String,
    /// High-level topology kind.
    pub kind: ExecutionTopologyKind,
    /// Ordered shard/placement assignments.
    pub assignments: Vec<ExecutionShardAssignment>,
}

impl ExecutionTopologyPlan {
    /// Creates a single-device topology.
    #[must_use]
    pub fn single_device(
        effective_backend: impl Into<String>,
        device: DeviceInventoryQualifiers,
    ) -> Self {
        Self {
            effective_backend: effective_backend.into(),
            kind: ExecutionTopologyKind::SingleDevice,
            assignments: vec![ExecutionShardAssignment {
                shard_id: 0,
                device: ExecutionDevicePlacement::from_inventory(&device, 0),
                partition: ExecutionPartition::WholeModel,
            }],
        }
    }

    /// Creates a replicated multi-device topology.
    #[must_use]
    pub fn replicated(
        effective_backend: impl Into<String>,
        devices: Vec<DeviceInventoryQualifiers>,
    ) -> Self {
        let assignments = devices
            .iter()
            .enumerate()
            .map(|(index, device)| ExecutionShardAssignment {
                shard_id: index,
                device: ExecutionDevicePlacement::from_inventory(device, index),
                partition: ExecutionPartition::Replica {
                    replica_index: index,
                },
            })
            .collect();
        Self {
            effective_backend: effective_backend.into(),
            kind: ExecutionTopologyKind::Replicated,
            assignments,
        }
    }

    /// Creates a pipeline-sharded topology from explicit per-stage layer ranges.
    #[must_use]
    pub fn pipeline_sharded(
        effective_backend: impl Into<String>,
        stages: Vec<(DeviceInventoryQualifiers, usize, usize)>,
    ) -> Self {
        let assignments = stages
            .iter()
            .enumerate()
            .map(
                |(index, (device, start_layer, end_layer))| ExecutionShardAssignment {
                    shard_id: index,
                    device: ExecutionDevicePlacement::from_inventory(device, index),
                    partition: ExecutionPartition::LayerRange {
                        start_layer: *start_layer,
                        end_layer: *end_layer,
                    },
                },
            )
            .collect();
        Self {
            effective_backend: effective_backend.into(),
            kind: ExecutionTopologyKind::PipelineSharded,
            assignments,
        }
    }

    /// Creates a layer-sharded topology from explicit per-device layer ranges.
    #[must_use]
    pub fn layer_sharded(
        effective_backend: impl Into<String>,
        shards: Vec<(DeviceInventoryQualifiers, usize, usize)>,
    ) -> Self {
        let assignments = shards
            .iter()
            .enumerate()
            .map(
                |(index, (device, start_layer, end_layer))| ExecutionShardAssignment {
                    shard_id: index,
                    device: ExecutionDevicePlacement::from_inventory(device, index),
                    partition: ExecutionPartition::LayerRange {
                        start_layer: *start_layer,
                        end_layer: *end_layer,
                    },
                },
            )
            .collect();
        Self {
            effective_backend: effective_backend.into(),
            kind: ExecutionTopologyKind::LayerSharded,
            assignments,
        }
    }

    /// Creates a tensor-sharded topology from explicit per-device axis ranges.
    #[must_use]
    pub fn tensor_sharded(
        effective_backend: impl Into<String>,
        axis: usize,
        shards: Vec<(DeviceInventoryQualifiers, usize, usize)>,
    ) -> Self {
        let assignments = shards
            .iter()
            .enumerate()
            .map(|(index, (device, start, end))| ExecutionShardAssignment {
                shard_id: index,
                device: ExecutionDevicePlacement::from_inventory(device, index),
                partition: ExecutionPartition::TensorRange {
                    axis,
                    start: *start,
                    end: *end,
                },
            })
            .collect();
        Self {
            effective_backend: effective_backend.into(),
            kind: ExecutionTopologyKind::TensorSharded,
            assignments,
        }
    }

    /// Returns a stable digest for the topology assignments.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.effective_backend.as_bytes());
        hasher.update(b"|");
        hasher.update(match self.kind {
            ExecutionTopologyKind::SingleDevice => b"single_device".as_slice(),
            ExecutionTopologyKind::Replicated => b"replicated".as_slice(),
            ExecutionTopologyKind::PipelineSharded => b"pipeline_sharded".as_slice(),
            ExecutionTopologyKind::LayerSharded => b"layer_sharded".as_slice(),
            ExecutionTopologyKind::TensorSharded => b"tensor_sharded".as_slice(),
        });
        for assignment in &self.assignments {
            hasher.update(b"|");
            hasher.update(assignment.shard_id.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(assignment.device.stable_device_id.as_bytes());
            hasher.update(b"|");
            hasher.update(
                assignment
                    .device
                    .topology_key
                    .as_deref()
                    .unwrap_or_default()
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(assignment.device.placement_index.to_string().as_bytes());
            hasher.update(b"|");
            match assignment.partition {
                ExecutionPartition::WholeModel => hasher.update(b"whole_model"),
                ExecutionPartition::Replica { replica_index } => {
                    hasher.update(b"replica|");
                    hasher.update(replica_index.to_string().as_bytes());
                }
                ExecutionPartition::LayerRange {
                    start_layer,
                    end_layer,
                } => {
                    hasher.update(b"layer_range|");
                    hasher.update(start_layer.to_string().as_bytes());
                    hasher.update(b"|");
                    hasher.update(end_layer.to_string().as_bytes());
                }
                ExecutionPartition::TensorRange { axis, start, end } => {
                    hasher.update(b"tensor_range|");
                    hasher.update(axis.to_string().as_bytes());
                    hasher.update(b"|");
                    hasher.update(start.to_string().as_bytes());
                    hasher.update(b"|");
                    hasher.update(end.to_string().as_bytes());
                }
            }
        }
        format!("{:x}", hasher.finalize())
    }
}

/// Machine-checkable promised accelerator requirements for one compute-market offer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceleratorExecutionRequirement {
    /// Runtime backend promised for the offer.
    pub runtime_backend: String,
    /// Minimum number of devices required by the offer.
    pub minimum_device_count: usize,
    /// Exact topology kind required by the offer when one matters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_topology_kind: Option<ExecutionTopologyKind>,
    /// Exact stable device IDs required by the offer, when pinned.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_stable_device_ids: Vec<String>,
    /// Exact topology keys required by the offer, when pinned.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_topology_keys: Vec<String>,
    /// Minimum performance class required across delivered devices.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_performance_class: Option<DevicePerformanceClass>,
    /// Minimum memory class required across delivered devices.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_memory_class: Option<DeviceMemoryClass>,
    /// Minimum aggregate visible device memory required by the offer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_total_memory_bytes: Option<u64>,
}

impl AcceleratorExecutionRequirement {
    /// Creates a new promised accelerator requirement.
    #[must_use]
    pub fn new(runtime_backend: impl Into<String>, minimum_device_count: usize) -> Self {
        Self {
            runtime_backend: runtime_backend.into(),
            minimum_device_count,
            required_topology_kind: None,
            required_stable_device_ids: Vec::new(),
            required_topology_keys: Vec::new(),
            minimum_performance_class: None,
            minimum_memory_class: None,
            minimum_total_memory_bytes: None,
        }
    }

    /// Pins the offer to one exact topology kind.
    #[must_use]
    pub const fn with_topology_kind(mut self, kind: ExecutionTopologyKind) -> Self {
        self.required_topology_kind = Some(kind);
        self
    }

    /// Pins the offer to exact stable device IDs.
    #[must_use]
    pub fn with_required_devices(mut self, stable_device_ids: Vec<String>) -> Self {
        self.required_stable_device_ids = stable_device_ids;
        self
    }

    /// Pins the offer to exact topology keys.
    #[must_use]
    pub fn with_required_topology_keys(mut self, topology_keys: Vec<String>) -> Self {
        self.required_topology_keys = topology_keys;
        self
    }

    /// Requires at least one device performance class.
    #[must_use]
    pub const fn with_minimum_performance_class(mut self, class: DevicePerformanceClass) -> Self {
        self.minimum_performance_class = Some(class);
        self
    }

    /// Requires at least one device memory class.
    #[must_use]
    pub const fn with_minimum_memory_class(mut self, class: DeviceMemoryClass) -> Self {
        self.minimum_memory_class = Some(class);
        self
    }

    /// Requires a minimum aggregate visible memory budget.
    #[must_use]
    pub const fn with_minimum_total_memory_bytes(mut self, bytes: u64) -> Self {
        self.minimum_total_memory_bytes = Some(bytes);
        self
    }

    /// Evaluates one promised offer against the delivered execution context.
    #[must_use]
    pub fn evaluate(
        &self,
        delivered: DeliveredExecutionContext,
    ) -> AcceleratorDeliverabilityReport {
        let mut differences = Vec::new();
        let mut underdelivered = false;

        if delivered.runtime_backend != self.runtime_backend {
            underdelivered = true;
            differences.push(AcceleratorDeliverabilityDifference::new(
                AcceleratorDeliverabilityDifferenceCode::RuntimeBackendChanged,
                format!(
                    "promised backend `{}` but delivered `{}`",
                    self.runtime_backend, delivered.runtime_backend
                ),
            ));
        }

        if delivered.selected_devices.len() < self.minimum_device_count {
            underdelivered = true;
            differences.push(AcceleratorDeliverabilityDifference::new(
                AcceleratorDeliverabilityDifferenceCode::DeviceCountReduced,
                format!(
                    "promised at least {} devices but delivered {}",
                    self.minimum_device_count,
                    delivered.selected_devices.len()
                ),
            ));
        }

        if let Some(required_kind) = self.required_topology_kind {
            if delivered.topology_kind() != Some(required_kind) {
                differences.push(AcceleratorDeliverabilityDifference::new(
                    AcceleratorDeliverabilityDifferenceCode::TopologyKindChanged,
                    format!(
                        "promised topology {:?} but delivered {:?}",
                        required_kind,
                        delivered.topology_kind()
                    ),
                ));
            }
        }

        for stable_device_id in &self.required_stable_device_ids {
            if !delivered
                .selected_devices
                .iter()
                .any(|device| device.stable_device_id == *stable_device_id)
            {
                underdelivered = true;
                differences.push(AcceleratorDeliverabilityDifference::new(
                    AcceleratorDeliverabilityDifferenceCode::StableDeviceMissing,
                    format!("required stable device `{stable_device_id}` was not delivered"),
                ));
            }
        }

        for topology_key in &self.required_topology_keys {
            if !delivered
                .selected_devices
                .iter()
                .any(|device| device.topology_key.as_deref() == Some(topology_key.as_str()))
            {
                underdelivered = true;
                differences.push(AcceleratorDeliverabilityDifference::new(
                    AcceleratorDeliverabilityDifferenceCode::TopologyKeyMissing,
                    format!("required topology key `{topology_key}` was not delivered"),
                ));
            }
        }

        if let Some(required_class) = self.minimum_performance_class {
            if delivered.selected_devices.iter().any(|device| {
                device_performance_rank(device.performance_class)
                    < device_performance_rank(required_class)
            }) {
                underdelivered = true;
                differences.push(AcceleratorDeliverabilityDifference::new(
                    AcceleratorDeliverabilityDifferenceCode::PerformanceClassReduced,
                    format!(
                        "promised performance class {:?} or better across delivered devices",
                        required_class
                    ),
                ));
            }
        }

        if let Some(required_class) = self.minimum_memory_class {
            if delivered.selected_devices.iter().any(|device| {
                device_memory_rank(device.memory_class) < device_memory_rank(required_class)
            }) {
                underdelivered = true;
                differences.push(AcceleratorDeliverabilityDifference::new(
                    AcceleratorDeliverabilityDifferenceCode::MemoryClassReduced,
                    format!(
                        "promised memory class {:?} or better across delivered devices",
                        required_class
                    ),
                ));
            }
        }

        if let Some(required_total_memory) = self.minimum_total_memory_bytes {
            match delivered.total_memory_bytes() {
                Some(total_memory) if total_memory < required_total_memory => {
                    underdelivered = true;
                    differences.push(AcceleratorDeliverabilityDifference::new(
                        AcceleratorDeliverabilityDifferenceCode::TotalMemoryReduced,
                        format!(
                            "promised at least {required_total_memory} bytes of total accelerator memory but delivered {total_memory}"
                        ),
                    ));
                }
                None => {
                    underdelivered = true;
                    differences.push(AcceleratorDeliverabilityDifference::new(
                        AcceleratorDeliverabilityDifferenceCode::TotalMemoryUnknown,
                        "promised total accelerator memory could not be verified from delivered inventory",
                    ));
                }
                Some(_) => {}
            }
        }

        let status = if underdelivered {
            AcceleratorDeliverabilityStatus::Underdelivered
        } else if differences.is_empty() {
            AcceleratorDeliverabilityStatus::Exact
        } else {
            AcceleratorDeliverabilityStatus::CompatibleSubstitution
        };

        AcceleratorDeliverabilityReport {
            status,
            requirement: self.clone(),
            delivered,
            differences,
        }
    }
}

/// Delivered execution facts used to compare against one promised accelerator offer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveredExecutionContext {
    /// Runtime backend that actually executed the work.
    pub runtime_backend: String,
    /// Explicit multi-device or sharded topology when one was planned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_topology: Option<ExecutionTopologyPlan>,
    /// Delivered device inventory qualifiers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_devices: Vec<DeviceInventoryQualifiers>,
    /// Explicit clustered execution facts when the request crossed node boundaries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_execution: Option<ClusterExecutionContext>,
}

impl DeliveredExecutionContext {
    /// Creates a delivered execution context from explicit backend/topology/device facts.
    #[must_use]
    pub fn new(
        runtime_backend: impl Into<String>,
        execution_topology: Option<ExecutionTopologyPlan>,
        selected_devices: Vec<DeviceInventoryQualifiers>,
    ) -> Self {
        Self {
            runtime_backend: runtime_backend.into(),
            execution_topology,
            selected_devices,
            cluster_execution: None,
        }
    }

    /// Returns the delivered topology kind when one is explicit or derivable.
    #[must_use]
    pub fn topology_kind(&self) -> Option<ExecutionTopologyKind> {
        self.execution_topology
            .as_ref()
            .map(|plan| plan.kind)
            .or_else(|| {
                (self.selected_devices.len() == 1).then_some(ExecutionTopologyKind::SingleDevice)
            })
    }

    /// Returns the aggregate visible memory across delivered devices when known.
    #[must_use]
    pub fn total_memory_bytes(&self) -> Option<u64> {
        if self.selected_devices.is_empty() {
            return Some(0);
        }
        self.selected_devices.iter().try_fold(0u64, |acc, device| {
            device.total_memory_bytes.map(|value| acc + value)
        })
    }

    /// Attaches explicit clustered execution facts.
    #[must_use]
    pub fn with_cluster_execution(mut self, cluster_execution: ClusterExecutionContext) -> Self {
        if let Some(execution_topology) = &cluster_execution.execution_topology {
            self.execution_topology = Some(execution_topology.clone());
        }
        let cluster_selected_devices = cluster_execution.selected_devices_inventory();
        if !cluster_selected_devices.is_empty() {
            self.selected_devices = cluster_selected_devices;
        }
        self.cluster_execution = Some(cluster_execution);
        self
    }
}

/// High-level result for one promised-vs-delivered accelerator comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceleratorDeliverabilityStatus {
    /// Delivered execution exactly matched the promised accelerator offer.
    Exact,
    /// Delivered execution differed, but still met the minimum promise.
    CompatibleSubstitution,
    /// Delivered execution failed to satisfy the promised accelerator offer.
    Underdelivered,
}

/// High-level recovery posture for training-class clustered execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingRecoveryPosture {
    /// Membership, checkpoint, and worker state are steady.
    SteadyState,
    /// Recovery is blocked or waiting on a durable checkpoint.
    LateJoinPending,
    /// A recovering node is explicitly resuming state.
    Recovering,
    /// Membership is changing while work continues.
    ElasticReconfiguration,
    /// A new checkpoint is flushing asynchronously.
    AsyncCheckpointInFlight,
}

/// Availability class for checkpoint-backed training recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingCheckpointAvailability {
    /// No durable checkpoint is currently available.
    None,
    /// A checkpoint is being written but is not yet durable.
    AsyncWriteInFlight,
    /// A durable checkpoint is available for recovery or late join.
    Durable,
}

/// Explicit elastic-membership facts used by training recovery logic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingElasticMembershipContext {
    /// Monotonic membership epoch derived from observed cluster truth.
    pub membership_epoch: u64,
    /// Stable digest of the authoritative cluster snapshot for this epoch.
    pub cluster_state_digest: String,
    /// Stable digest of topology-relevant facts for this epoch.
    pub topology_digest: String,
    /// Nodes currently considered active workers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_node_ids: Vec<String>,
    /// Nodes currently joining and not yet trusted as active workers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joining_node_ids: Vec<String>,
    /// Nodes currently draining and excluded from new work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub draining_node_ids: Vec<String>,
    /// Nodes known but offline.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offline_node_ids: Vec<String>,
}

impl TrainingElasticMembershipContext {
    /// Creates training membership facts from one epoch and digest set.
    #[must_use]
    pub fn new(
        membership_epoch: u64,
        cluster_state_digest: impl Into<String>,
        topology_digest: impl Into<String>,
        active_node_ids: Vec<String>,
    ) -> Self {
        Self {
            membership_epoch,
            cluster_state_digest: cluster_state_digest.into(),
            topology_digest: topology_digest.into(),
            active_node_ids: sorted_distinct_strings(active_node_ids),
            joining_node_ids: Vec::new(),
            draining_node_ids: Vec::new(),
            offline_node_ids: Vec::new(),
        }
    }

    /// Attaches joining-node facts.
    #[must_use]
    pub fn with_joining_node_ids(mut self, joining_node_ids: Vec<String>) -> Self {
        self.joining_node_ids = sorted_distinct_strings(joining_node_ids);
        self
    }

    /// Attaches draining-node facts.
    #[must_use]
    pub fn with_draining_node_ids(mut self, draining_node_ids: Vec<String>) -> Self {
        self.draining_node_ids = sorted_distinct_strings(draining_node_ids);
        self
    }

    /// Attaches offline-node facts.
    #[must_use]
    pub fn with_offline_node_ids(mut self, offline_node_ids: Vec<String>) -> Self {
        self.offline_node_ids = sorted_distinct_strings(offline_node_ids);
        self
    }
}

/// Stable durable or in-flight checkpoint identity for training recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingCheckpointReference {
    /// Stable checkpoint family such as `train.decoder`.
    pub checkpoint_family: String,
    /// Stable stream identifier exporting the checkpoint bytes.
    pub stream_id: String,
    /// Stable manifest digest constraining the checkpoint stream.
    pub manifest_digest: String,
    /// Stable payload digest for the checkpoint object.
    pub object_digest: String,
    /// Writer node that emitted the checkpoint.
    pub writer_node_id: String,
    /// Membership epoch observed when the checkpoint started.
    pub membership_epoch: u64,
    /// Stable cluster snapshot digest observed when the checkpoint started.
    pub cluster_state_digest: String,
    /// Stable topology digest observed when the checkpoint started.
    pub topology_digest: String,
    /// Logical timestamp when the checkpoint write began.
    pub started_at_ms: u64,
    /// Optional stable checkpoint reference supplied by the producer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_ref: Option<String>,
    /// Optional logical checkpoint step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u64>,
    /// Logical timestamp when the checkpoint became durable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durable_at_ms: Option<u64>,
}

impl TrainingCheckpointReference {
    /// Creates one training checkpoint reference from explicit stable identities.
    #[must_use]
    pub fn new(
        checkpoint_family: impl Into<String>,
        stream_id: impl Into<String>,
        manifest_digest: impl Into<String>,
        object_digest: impl Into<String>,
        writer_node_id: impl Into<String>,
        membership_epoch: u64,
        cluster_state_digest: impl Into<String>,
        topology_digest: impl Into<String>,
        started_at_ms: u64,
    ) -> Self {
        Self {
            checkpoint_family: checkpoint_family.into(),
            stream_id: stream_id.into(),
            manifest_digest: manifest_digest.into(),
            object_digest: object_digest.into(),
            writer_node_id: writer_node_id.into(),
            membership_epoch,
            cluster_state_digest: cluster_state_digest.into(),
            topology_digest: topology_digest.into(),
            started_at_ms,
            checkpoint_ref: None,
            step: None,
            durable_at_ms: None,
        }
    }

    /// Attaches a stable checkpoint reference.
    #[must_use]
    pub fn with_checkpoint_ref(mut self, checkpoint_ref: impl Into<String>) -> Self {
        self.checkpoint_ref = Some(checkpoint_ref.into());
        self
    }

    /// Attaches a logical step.
    #[must_use]
    pub const fn with_step(mut self, step: u64) -> Self {
        self.step = Some(step);
        self
    }

    /// Marks the checkpoint durable.
    #[must_use]
    pub const fn with_durable_at_ms(mut self, durable_at_ms: u64) -> Self {
        self.durable_at_ms = Some(durable_at_ms);
        self
    }
}

/// Runtime-visible training recovery posture attached to clustered execution evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingRecoveryContext {
    /// High-level training recovery posture.
    pub posture: TrainingRecoveryPosture,
    /// Availability class for durable checkpoint-backed recovery.
    pub checkpoint_availability: TrainingCheckpointAvailability,
    /// Current elastic-membership facts.
    pub elastic_membership: TrainingElasticMembershipContext,
    /// Latest in-flight or durable checkpoint reference, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_checkpoint: Option<TrainingCheckpointReference>,
    /// Nodes explicitly recovering state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recovering_node_ids: Vec<String>,
    /// Nodes explicitly joining the world after the current checkpoint.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub late_joiner_node_ids: Vec<String>,
    /// Logical timestamp when recovery or reconfiguration was requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_at_ms: Option<u64>,
    /// Plain-language detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl TrainingRecoveryContext {
    /// Creates runtime-visible training recovery posture from explicit membership facts.
    #[must_use]
    pub fn new(
        posture: TrainingRecoveryPosture,
        checkpoint_availability: TrainingCheckpointAvailability,
        elastic_membership: TrainingElasticMembershipContext,
    ) -> Self {
        Self {
            posture,
            checkpoint_availability,
            elastic_membership,
            latest_checkpoint: None,
            recovering_node_ids: Vec::new(),
            late_joiner_node_ids: Vec::new(),
            requested_at_ms: None,
            detail: None,
        }
    }

    /// Attaches the latest in-flight or durable checkpoint.
    #[must_use]
    pub fn with_latest_checkpoint(
        mut self,
        latest_checkpoint: TrainingCheckpointReference,
    ) -> Self {
        self.latest_checkpoint = Some(latest_checkpoint);
        self
    }

    /// Attaches explicit recovering-node facts.
    #[must_use]
    pub fn with_recovering_node_ids(mut self, recovering_node_ids: Vec<String>) -> Self {
        self.recovering_node_ids = sorted_distinct_strings(recovering_node_ids);
        self
    }

    /// Attaches explicit late-joiner facts.
    #[must_use]
    pub fn with_late_joiner_node_ids(mut self, late_joiner_node_ids: Vec<String>) -> Self {
        self.late_joiner_node_ids = sorted_distinct_strings(late_joiner_node_ids);
        self
    }

    /// Attaches the logical request timestamp.
    #[must_use]
    pub const fn with_requested_at_ms(mut self, requested_at_ms: u64) -> Self {
        self.requested_at_ms = Some(requested_at_ms);
        self
    }

    /// Attaches plain-language detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// High-level training device-mesh axis kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingDeviceMeshAxisKind {
    /// Data-parallel replica axis.
    DataParallel,
    /// Tensor-parallel shard axis.
    TensorParallel,
    /// Pipeline stage axis.
    PipelineParallel,
    /// Expert-parallel axis.
    ExpertParallel,
}

/// One named axis in an elastic training device mesh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingDeviceMeshAxis {
    /// Stable axis identifier.
    pub axis_id: String,
    /// High-level axis kind.
    pub kind: TrainingDeviceMeshAxisKind,
    /// Logical extent of the axis.
    pub extent: usize,
    /// Collective group size realized on this axis.
    pub collective_group_size: usize,
    /// Plain-language axis detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl TrainingDeviceMeshAxis {
    /// Creates a device-mesh axis from explicit identity and extent.
    #[must_use]
    pub fn new(
        axis_id: impl Into<String>,
        kind: TrainingDeviceMeshAxisKind,
        extent: usize,
    ) -> Self {
        let extent = extent.max(1);
        Self {
            axis_id: axis_id.into(),
            kind,
            extent,
            collective_group_size: extent,
            detail: None,
        }
    }

    /// Overrides the collective group size for this axis.
    #[must_use]
    pub fn with_collective_group_size(mut self, collective_group_size: usize) -> Self {
        self.collective_group_size = collective_group_size.max(1);
        self
    }

    /// Attaches plain-language axis detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Runtime-visible device-mesh posture for training-class execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingDeviceMeshContext {
    /// Stable mesh identifier.
    pub mesh_id: String,
    /// Monotonic mesh revision derived from elastic membership truth.
    pub mesh_revision: u64,
    /// Effective backend running the mesh.
    pub effective_backend: String,
    /// Communication class required by the mesh.
    pub communication_class: ClusterCommunicationClass,
    /// Elastic membership facts currently constraining the mesh.
    pub elastic_membership: TrainingElasticMembershipContext,
    /// Explicit node IDs participating in this mesh revision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub member_node_ids: Vec<String>,
    /// Explicit axes realized by the mesh.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<TrainingDeviceMeshAxis>,
}

impl TrainingDeviceMeshContext {
    /// Creates a device-mesh context from explicit identity and membership facts.
    #[must_use]
    pub fn new(
        mesh_id: impl Into<String>,
        mesh_revision: u64,
        effective_backend: impl Into<String>,
        communication_class: ClusterCommunicationClass,
        elastic_membership: TrainingElasticMembershipContext,
        member_node_ids: Vec<String>,
    ) -> Self {
        Self {
            mesh_id: mesh_id.into(),
            mesh_revision,
            effective_backend: effective_backend.into(),
            communication_class,
            elastic_membership,
            member_node_ids: sorted_distinct_strings(member_node_ids),
            axes: Vec::new(),
        }
    }

    /// Replaces the explicit axis set.
    #[must_use]
    pub fn with_axes(mut self, axes: Vec<TrainingDeviceMeshAxis>) -> Self {
        self.axes = axes;
        self
    }
}

/// High-level training collective kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingCollectiveKind {
    /// All members reduce and receive the full result.
    AllReduce,
    /// All members gather the full set of shards.
    AllGather,
    /// All members receive only their reduced partition.
    ReduceScatter,
    /// One member broadcasts state to the rest of the mesh.
    Broadcast,
}

/// Explicit quantization mode for collective communication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingCollectiveQuantization {
    /// No communication quantization is applied.
    None,
    /// Symmetric int8 packing for the collective payload.
    Int8Symmetric,
    /// Blockwise NF4-style packing for the collective payload.
    Nf4Blockwise,
}

/// Runtime-visible collective posture attached to clustered training execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingCollectiveContext {
    /// Device-mesh posture used for the collective.
    pub device_mesh: TrainingDeviceMeshContext,
    /// High-level collective kind.
    pub kind: TrainingCollectiveKind,
    /// Quantization mode applied to communication.
    pub quantization: TrainingCollectiveQuantization,
    /// Logical payload size before wire-level quantization.
    pub payload_bytes: u64,
    /// Estimated wire bytes after collective quantization and fanout.
    pub estimated_wire_bytes: u64,
    /// Number of mesh workers participating in the collective.
    pub worker_count: usize,
    /// Stable digest for the benchmark that justified the quantized path, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_digest: Option<String>,
    /// Relative speedup in basis points versus the baseline benchmark.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_speedup_bps: Option<u64>,
    /// Maximum relative numerical error observed in the benchmark.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_relative_error_bps: Option<u64>,
    /// Plain-language collective detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl TrainingCollectiveContext {
    /// Creates collective posture from explicit mesh, payload, and worker facts.
    #[must_use]
    pub fn new(
        device_mesh: TrainingDeviceMeshContext,
        kind: TrainingCollectiveKind,
        quantization: TrainingCollectiveQuantization,
        payload_bytes: u64,
        estimated_wire_bytes: u64,
        worker_count: usize,
    ) -> Self {
        Self {
            device_mesh,
            kind,
            quantization,
            payload_bytes,
            estimated_wire_bytes,
            worker_count: worker_count.max(1),
            benchmark_digest: None,
            benchmark_speedup_bps: None,
            max_relative_error_bps: None,
            detail: None,
        }
    }

    /// Attaches benchmark justification for the collective path.
    #[must_use]
    pub fn with_benchmark(
        mut self,
        benchmark_digest: impl Into<String>,
        benchmark_speedup_bps: u64,
        max_relative_error_bps: u64,
    ) -> Self {
        self.benchmark_digest = Some(benchmark_digest.into());
        self.benchmark_speedup_bps = Some(benchmark_speedup_bps);
        self.max_relative_error_bps = Some(max_relative_error_bps);
        self
    }

    /// Attaches plain-language collective detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Stable difference code for one promised-vs-delivered accelerator comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceleratorDeliverabilityDifferenceCode {
    /// The runtime backend changed between promise and delivery.
    RuntimeBackendChanged,
    /// The delivered topology kind differed from the promise.
    TopologyKindChanged,
    /// Fewer devices were delivered than promised.
    DeviceCountReduced,
    /// One required stable device ID was not delivered.
    StableDeviceMissing,
    /// One required topology key was not delivered.
    TopologyKeyMissing,
    /// Delivered devices fell below the promised performance class.
    PerformanceClassReduced,
    /// Delivered devices fell below the promised memory class.
    MemoryClassReduced,
    /// Delivered aggregate memory fell below the promised minimum.
    TotalMemoryReduced,
    /// Delivered aggregate memory could not be verified.
    TotalMemoryUnknown,
}

/// One explicit difference between a promised and delivered accelerator offer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceleratorDeliverabilityDifference {
    /// Stable difference code.
    pub code: AcceleratorDeliverabilityDifferenceCode,
    /// Short machine-readable explanation for the difference.
    pub detail: String,
}

impl AcceleratorDeliverabilityDifference {
    fn new(code: AcceleratorDeliverabilityDifferenceCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

/// Machine-checkable promised-vs-delivered report for accelerator-sensitive offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceleratorDeliverabilityReport {
    /// High-level comparison status.
    pub status: AcceleratorDeliverabilityStatus,
    /// Promised accelerator requirement.
    pub requirement: AcceleratorExecutionRequirement,
    /// Delivered execution context.
    pub delivered: DeliveredExecutionContext,
    /// Explicit substitutions or underdelivery differences.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub differences: Vec<AcceleratorDeliverabilityDifference>,
}

const fn device_performance_rank(class: DevicePerformanceClass) -> u8 {
    match class {
        DevicePerformanceClass::Reference => 0,
        DevicePerformanceClass::IntegratedAccelerator => 1,
        DevicePerformanceClass::PartitionedAccelerator => 2,
        DevicePerformanceClass::DiscreteAccelerator => 3,
    }
}

const fn device_memory_rank(class: DeviceMemoryClass) -> u8 {
    match class {
        DeviceMemoryClass::HostOnly => 0,
        DeviceMemoryClass::SharedHostDevice => 1,
        DeviceMemoryClass::DedicatedDevice => 2,
    }
}

impl DeviceDescriptor {
    /// Returns stable inventory qualifiers derived from the current device/topology metadata.
    #[must_use]
    pub fn inventory_qualifiers(&self) -> DeviceInventoryQualifiers {
        let topology_key = self
            .nvidia_metadata
            .as_ref()
            .and_then(|metadata| metadata.topology.pci_bdf.clone())
            .or_else(|| {
                self.amd_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.topology.pci_bdf.clone())
            });
        let stable_device_id = topology_key
            .clone()
            .or_else(|| self.device.label().map(String::from))
            .unwrap_or_else(|| format!("{}:{}", self.backend, self.device.ordinal()));
        let performance_class = if self.device.kind() == DeviceKind::Cpu {
            DevicePerformanceClass::Reference
        } else if self
            .nvidia_metadata
            .as_ref()
            .and_then(|metadata| metadata.topology.mig_profile.as_ref())
            .is_some()
        {
            DevicePerformanceClass::PartitionedAccelerator
        } else if self.unified_memory == Some(true) {
            DevicePerformanceClass::IntegratedAccelerator
        } else {
            DevicePerformanceClass::DiscreteAccelerator
        };
        let memory_class = if self.device.kind() == DeviceKind::Cpu {
            DeviceMemoryClass::HostOnly
        } else if self.unified_memory == Some(true) {
            DeviceMemoryClass::SharedHostDevice
        } else {
            DeviceMemoryClass::DedicatedDevice
        };

        DeviceInventoryQualifiers {
            stable_device_id,
            topology_key,
            performance_class,
            memory_class,
            total_memory_bytes: self.memory_capacity_bytes,
            free_memory_bytes: None,
        }
    }
}

/// Exact allocator-pool reuse posture for one backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocatorPoolMode {
    /// Do not retain freed buffers for reuse.
    Disabled,
    /// Reuse only buffers whose tensor spec matches exactly.
    ExactTensorSpec,
}

/// Explicit allocator-pool policy for one backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocatorPoolPolicy {
    /// Reuse posture for returned buffers.
    pub mode: AllocatorPoolMode,
    /// Maximum cached buffers retained by the pool.
    pub max_cached_buffers: usize,
    /// Maximum cached bytes retained by the pool.
    pub max_cached_bytes: u64,
}

impl AllocatorPoolPolicy {
    /// Creates a disabled allocator pool policy.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            mode: AllocatorPoolMode::Disabled,
            max_cached_buffers: 0,
            max_cached_bytes: 0,
        }
    }

    /// Creates an exact-spec reuse policy with explicit bounds.
    #[must_use]
    pub const fn exact_tensor_spec(max_cached_buffers: usize, max_cached_bytes: u64) -> Self {
        Self {
            mode: AllocatorPoolMode::ExactTensorSpec,
            max_cached_buffers,
            max_cached_bytes,
        }
    }
}

/// Current allocator-pool occupancy for one backend.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocatorPoolState {
    /// Number of buffers currently retained for reuse.
    pub cached_buffers: usize,
    /// Number of bytes currently retained for reuse.
    pub cached_bytes: u64,
}

/// Explicit allocator-pool policy plus current occupancy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocatorPoolReport {
    /// Allocator-pool policy for the backend.
    pub policy: AllocatorPoolPolicy,
    /// Current allocator-pool occupancy.
    pub state: AllocatorPoolState,
}

/// Explicit execution-plan cache policy for one backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlanCachePolicy {
    /// Whether compiled execution plans are cached across requests.
    pub enabled: bool,
    /// Maximum cache entries retained by the backend.
    pub max_cached_entries: usize,
    /// Maximum cache bytes reserved by policy, when bounded.
    pub max_cached_bytes: Option<u64>,
}

impl ExecutionPlanCachePolicy {
    /// Creates a disabled execution-plan cache policy.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            max_cached_entries: 0,
            max_cached_bytes: Some(0),
        }
    }

    /// Creates a bounded enabled execution-plan cache policy.
    #[must_use]
    pub const fn bounded(max_cached_entries: usize, max_cached_bytes: Option<u64>) -> Self {
        Self {
            enabled: true,
            max_cached_entries,
            max_cached_bytes,
        }
    }
}

/// Current execution-plan cache occupancy for one backend.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlanCacheState {
    /// Number of compiled execution plans retained by the backend.
    pub cached_entries: usize,
    /// Estimated bytes retained by the cache.
    pub cached_bytes: u64,
}

/// Explicit execution-plan cache policy plus current occupancy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlanCacheReport {
    /// Execution-plan cache policy for the backend.
    pub policy: ExecutionPlanCachePolicy,
    /// Current execution-plan cache occupancy.
    pub state: ExecutionPlanCacheState,
}

/// Explicit kernel-cache policy for one backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelCachePolicy {
    /// Whether compiled kernels are cached across requests.
    pub enabled: bool,
    /// Maximum cache entries retained by the backend.
    pub max_cached_entries: usize,
    /// Maximum cache bytes reserved by policy, when bounded.
    pub max_cached_bytes: Option<u64>,
}

impl KernelCachePolicy {
    /// Creates a disabled kernel-cache policy.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            max_cached_entries: 0,
            max_cached_bytes: Some(0),
        }
    }

    /// Creates a bounded enabled kernel-cache policy.
    #[must_use]
    pub const fn bounded(max_cached_entries: usize, max_cached_bytes: Option<u64>) -> Self {
        Self {
            enabled: true,
            max_cached_entries,
            max_cached_bytes,
        }
    }
}

/// Current kernel-cache occupancy for one backend.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelCacheState {
    /// Number of compiled cache entries retained by the backend.
    pub cached_entries: usize,
    /// Estimated bytes retained by the cache.
    pub cached_bytes: u64,
}

/// Explicit kernel-cache policy plus current occupancy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelCacheReport {
    /// Kernel-cache policy for the backend.
    pub policy: KernelCachePolicy,
    /// Current kernel-cache occupancy.
    pub state: KernelCacheState,
}

/// Backend-visible device-memory budget reserved around allocator and kernel caches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceMemoryBudget {
    /// Total device-visible memory budget when the backend can report one.
    pub total_bytes: Option<u64>,
    /// Bytes reserved by allocator-pool policy.
    pub allocator_pool_budget_bytes: u64,
    /// Bytes reserved by kernel-cache policy.
    pub kernel_cache_budget_bytes: u64,
    /// Remaining bytes available for execution/model residency after reserved budgets, when known.
    pub available_execution_bytes: Option<u64>,
}

impl DeviceMemoryBudget {
    /// Creates a device-memory budget from explicit total and reserved budgets.
    #[must_use]
    pub fn new(
        total_bytes: Option<u64>,
        allocator_pool_budget_bytes: u64,
        kernel_cache_budget_bytes: u64,
    ) -> Self {
        let reserved = allocator_pool_budget_bytes.saturating_add(kernel_cache_budget_bytes);
        let available_execution_bytes =
            total_bytes.and_then(|total_bytes| total_bytes.checked_sub(reserved));
        Self {
            total_bytes,
            allocator_pool_budget_bytes,
            kernel_cache_budget_bytes,
            available_execution_bytes,
        }
    }
}

/// Explicit backend resource state carried into capability and receipt surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendRuntimeResources {
    /// Explicit execution-plan cache policy and occupancy.
    pub execution_plan_cache: ExecutionPlanCacheReport,
    /// Explicit allocator-pool policy and occupancy.
    pub allocator_pool: AllocatorPoolReport,
    /// Explicit kernel-cache policy and occupancy.
    pub kernel_cache: KernelCacheReport,
    /// Device-memory budget reserved around those runtime-owned caches, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_memory_budget: Option<DeviceMemoryBudget>,
}

/// Current execution-plan cache format version.
pub const EXECUTION_PLAN_CACHE_FORMAT_VERSION: u32 = 1;

/// Current backend kernel-cache format version.
pub const KERNEL_CACHE_FORMAT_VERSION: u32 = 1;

/// Current paged-tensor storage format version.
pub const PAGED_TENSOR_STORAGE_FORMAT_VERSION: u32 = 1;

/// Current shared prefix-cache format version.
pub const PREFIX_CACHE_FORMAT_VERSION: u32 = 1;

/// Current persisted/session KV-state format version.
pub const KV_STATE_FORMAT_VERSION: u32 = 1;

/// Cache family covered by the runtime invalidation policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheKind {
    /// Compiled execution plans retained across warm loads.
    ExecutionPlan,
    /// Backend-managed compiled kernels or pipelines.
    KernelCache,
    /// Artifact-backed paged tensor storage or mappings.
    PagedTensorStorage,
    /// Shared prompt-prefix reuse entries.
    PrefixCache,
    /// Per-session KV state retained across requests.
    KvState,
}

/// Reuse scope for one runtime-owned cache family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheScope {
    /// Reuse is limited to the current host process.
    ProcessLocal,
    /// Reuse spans multiple requests in shared in-memory state.
    SharedAcrossRequests,
    /// Reuse spans multiple requests for one bound session only.
    SessionBound,
    /// Reuse restores access from artifact-backed bytes instead of in-memory copies.
    ArtifactBacked,
}

/// Explicit action taken for one cache family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheAction {
    /// Compatible state was reused as-is.
    Reuse,
    /// State was rebuilt under the current runtime.
    Rebuild,
    /// Reuse was intentionally skipped.
    Bypass,
    /// Incompatible state was explicitly discarded.
    Invalidate,
    /// State was restored from artifact-backed or serialized inputs.
    Restore,
}

/// Explicit trigger that invalidates one cache family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheInvalidationTrigger {
    /// The runtime binary or crate version changed.
    BinaryUpgrade,
    /// The effective backend or its toolchain version changed.
    BackendToolchainUpgrade,
    /// Model-level metadata changed.
    ModelMetadataChange,
    /// Tokenizer behavior or digest changed.
    TokenizerDrift,
    /// Chat-template behavior or digest changed.
    ChatTemplateDrift,
    /// Default BOS/EOS or stop behavior changed.
    GenerationDefaultsDrift,
    /// Quantization family or layout changed.
    QuantizationChange,
    /// The execution-plan cache format changed.
    PlanFormatUpgrade,
    /// The backend kernel-cache format changed.
    KernelFormatUpgrade,
    /// The paged-tensor storage format changed.
    PagedTensorFormatUpgrade,
    /// The shared prefix-cache format changed.
    PrefixCacheFormatUpgrade,
    /// The KV-state format changed.
    KvStateFormatUpgrade,
    /// Cluster routing changed and reuse can no longer be trusted.
    ClusterRouteChange,
    /// Cluster topology or shard placement changed and reuse can no longer be trusted.
    ClusterTopologyChange,
    /// The caller explicitly reset or discarded state.
    ExplicitReset,
}

/// Invalidations and mismatch behavior for one cache family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheStorePolicy {
    /// Reuse scope for the cache family.
    pub scope: CacheScope,
    /// Current runtime-owned format version for the cache family.
    pub format_version: u32,
    /// Action taken when cached state remains compatible.
    pub compatible_action: CacheAction,
    /// Action taken when a required compatibility input changes.
    pub incompatible_action: CacheAction,
    /// Triggers that invalidate this cache family.
    pub invalidates_on: Vec<CacheInvalidationTrigger>,
}

impl CacheStorePolicy {
    /// Creates a cache-store policy from explicit compatibility rules.
    #[must_use]
    pub fn new(
        scope: CacheScope,
        format_version: u32,
        compatible_action: CacheAction,
        incompatible_action: CacheAction,
        invalidates_on: Vec<CacheInvalidationTrigger>,
    ) -> Self {
        Self {
            scope,
            format_version,
            compatible_action,
            incompatible_action,
            invalidates_on,
        }
    }
}

/// Runtime-owned invalidation policy across reusable cache families.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheInvalidationPolicy {
    /// Runtime binary version used to evaluate upgrade invalidation.
    pub runtime_binary_version: String,
    /// Execution-plan cache policy.
    pub execution_plan: CacheStorePolicy,
    /// Backend kernel-cache policy.
    pub kernel_cache: CacheStorePolicy,
    /// Paged tensor storage policy.
    pub paged_tensor_storage: CacheStorePolicy,
    /// Shared prefix-cache policy.
    pub prefix_cache: CacheStorePolicy,
    /// Session KV-state policy.
    pub kv_state: CacheStorePolicy,
}

impl Default for CacheInvalidationPolicy {
    fn default() -> Self {
        default_cache_invalidation_policy()
    }
}

/// Observable cache action for one realized request or restore path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheObservation {
    /// Cache family involved in the action.
    pub kind: CacheKind,
    /// Action taken for that cache family.
    pub action: CacheAction,
    /// Trigger that forced a rebuild, invalidation, or bypass when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<CacheInvalidationTrigger>,
    /// Short machine-readable explanation for the realized action.
    pub detail: String,
}

impl CacheObservation {
    /// Creates a cache observation from explicit action and detail.
    #[must_use]
    pub fn new(kind: CacheKind, action: CacheAction, detail: impl Into<String>) -> Self {
        Self {
            kind,
            action,
            trigger: None,
            detail: detail.into(),
        }
    }

    /// Attaches the invalidation trigger that drove the action.
    #[must_use]
    pub const fn with_trigger(mut self, trigger: CacheInvalidationTrigger) -> Self {
        self.trigger = Some(trigger);
        self
    }
}

/// Whether a request compiled a plan cold or reused one from a warm cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilePathTemperature {
    /// The runtime had to compile a new execution plan for this path.
    ColdCompile,
    /// The runtime reused an already-compiled execution plan.
    WarmReuse,
}

/// Explicit compile-path evidence for a realized execution path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilePathEvidence {
    /// Whether the path compiled cold or reused a warm plan cache entry.
    pub temperature: CompilePathTemperature,
    /// Observable execution-plan cache action for the realized path.
    pub execution_plan_cache: CacheObservation,
    /// Observable kernel-cache action for the realized path.
    pub kernel_cache: CacheObservation,
}

/// Delivery-proof facts surfaced for one realized execution path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionDeliveryProof {
    /// Stable execution-plan digest used for the realized path.
    pub execution_plan_digest: String,
    /// Total kernel or step dispatch count surfaced by the backend path.
    pub kernel_count: usize,
    /// Total bytes moved or written by the backend path.
    pub bytes_moved: u64,
    /// Number of execution-plan cache hits observed during the request path.
    pub plan_cache_hits: usize,
    /// Number of execution-plan cache misses or rebuilds observed during the request path.
    pub plan_cache_misses: usize,
    /// KV-cache growth surfaced for the request path, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_growth: Option<KvCacheGrowth>,
    /// Explicit prefill/decode handoff seam for the realized path, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefill_decode_handoff: Option<PrefillDecodeHandoff>,
    /// Explicit hierarchical KV residency accounting for the realized path, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_residency: Option<KvResidencyAccounting>,
}

/// Provider-facing settlement-linkage inputs derived from a realized execution path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementLinkageInput {
    /// Stable request digest used for settlement correlation.
    pub request_digest: String,
    /// Product family executed for the request.
    pub product_id: String,
    /// Stable model identifier executed for the request.
    pub model_id: String,
    /// Stable served-artifact digest for the realized model/backend path.
    pub served_artifact_digest: String,
    /// Stable execution-plan digest used for the realized path.
    pub execution_plan_digest: String,
    /// Runtime backend that actually executed the work.
    pub runtime_backend: String,
    /// Total kernel or step dispatch count surfaced by the backend path.
    pub kernel_count: usize,
    /// Total bytes moved or written by the backend path.
    pub bytes_moved: u64,
    /// Number of execution-plan cache hits observed during the request path.
    pub plan_cache_hits: usize,
    /// Number of execution-plan cache misses or rebuilds observed during the request path.
    pub plan_cache_misses: usize,
    /// KV-cache growth surfaced for the request path, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_growth: Option<KvCacheGrowth>,
    /// Output token count when the product family emits tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<usize>,
    /// Cluster command/admission provenance retained for settlement correlation, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_provenance: Option<ClusterSettlementProvenanceInput>,
}

/// Terminal receipt posture carried into a signed cluster evidence bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterEvidenceBundleStatus {
    /// The clustered request completed successfully.
    Succeeded,
    /// The clustered request was cancelled by the caller.
    Cancelled,
    /// The clustered request stopped because the client disconnected.
    Disconnected,
    /// The clustered request failed before successful completion.
    Failed,
}

/// Stable export payload for one clustered execution receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterEvidenceBundlePayload {
    /// Product family executed for the request.
    pub product_id: String,
    /// Stable request identifier.
    pub request_id: String,
    /// Stable request digest.
    pub request_digest: String,
    /// Stable model identifier executed for the request.
    pub model_id: String,
    /// Stable model revision executed for the request.
    pub model_revision: String,
    /// Runtime backend that realized the request.
    pub runtime_backend: String,
    /// Stable served-artifact digest for the realized path.
    pub served_artifact_digest: String,
    /// Stable weight-bundle digest for the realized path.
    pub weight_bundle_digest: String,
    /// Terminal receipt posture for the bundled execution.
    pub status: ClusterEvidenceBundleStatus,
    /// Runtime-owned clustered execution evidence for the request path.
    pub cluster_execution: ClusterExecutionContext,
    /// Delivery-proof facts surfaced for the request path, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_proof: Option<ExecutionDeliveryProof>,
    /// Settlement-linkage facts derived from the request path, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settlement_linkage: Option<SettlementLinkageInput>,
    /// Stable digest of the canonical execution-proof bundle for this path, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof_bundle_digest: Option<String>,
    /// Failure detail when the bundled receipt did not succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// Structured runtime diagnostic retained for audit/export, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<LocalRuntimeDiagnostic>,
}

impl ClusterEvidenceBundlePayload {
    /// Creates a new cluster evidence bundle payload from receipt-facing facts.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        product_id: impl Into<String>,
        request_id: impl Into<String>,
        request_digest: impl Into<String>,
        model_id: impl Into<String>,
        model_revision: impl Into<String>,
        runtime_backend: impl Into<String>,
        served_artifact_digest: impl Into<String>,
        weight_bundle_digest: impl Into<String>,
        status: ClusterEvidenceBundleStatus,
        cluster_execution: ClusterExecutionContext,
    ) -> Self {
        Self {
            product_id: product_id.into(),
            request_id: request_id.into(),
            request_digest: request_digest.into(),
            model_id: model_id.into(),
            model_revision: model_revision.into(),
            runtime_backend: runtime_backend.into(),
            served_artifact_digest: served_artifact_digest.into(),
            weight_bundle_digest: weight_bundle_digest.into(),
            status,
            cluster_execution,
            delivery_proof: None,
            settlement_linkage: None,
            proof_bundle_digest: None,
            failure_reason: None,
            diagnostic: None,
        }
    }

    /// Attaches delivery-proof facts to the bundle payload.
    #[must_use]
    pub fn with_delivery_proof(mut self, delivery_proof: ExecutionDeliveryProof) -> Self {
        self.delivery_proof = Some(delivery_proof);
        self
    }

    /// Attaches settlement-linkage facts to the bundle payload.
    #[must_use]
    pub fn with_settlement_linkage(mut self, settlement_linkage: SettlementLinkageInput) -> Self {
        self.settlement_linkage = Some(settlement_linkage);
        self
    }

    /// Attaches a canonical proof-bundle digest to the bundle payload.
    #[must_use]
    pub fn with_proof_bundle_digest(mut self, proof_bundle_digest: impl Into<String>) -> Self {
        self.proof_bundle_digest = Some(proof_bundle_digest.into());
        self
    }

    /// Attaches failure detail to the bundle payload.
    #[must_use]
    pub fn with_failure_reason(mut self, failure_reason: impl Into<String>) -> Self {
        self.failure_reason = Some(failure_reason.into());
        self
    }

    /// Attaches a structured runtime diagnostic to the bundle payload.
    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: LocalRuntimeDiagnostic) -> Self {
        self.diagnostic = Some(diagnostic);
        self
    }

    /// Returns a stable digest for the export payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .unwrap_or_else(|_| unreachable!("cluster evidence bundle should serialize"));
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_evidence_bundle_payload|");
        hasher.update(encoded);
        hex::encode(hasher.finalize())
    }

    /// Signs this payload for later audit or dispute export.
    #[must_use]
    pub fn sign(
        self,
        signer_node_id: impl Into<String>,
        signing_key: &SigningKey,
    ) -> SignedClusterEvidenceBundle {
        let bundle_digest = self.stable_digest();
        let signature = signing_key.sign(&cluster_evidence_bundle_signing_payload(
            bundle_digest.as_str(),
        ));
        SignedClusterEvidenceBundle {
            bundle_digest,
            payload: self,
            signature: ClusterEvidenceBundleSignature {
                signer_node_id: signer_node_id.into(),
                signer_public_key: hex::encode(signing_key.verifying_key().to_bytes()),
                signature_hex: hex::encode(signature.to_bytes()),
            },
        }
    }
}

/// Signed export signature for one bundled cluster execution payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterEvidenceBundleSignature {
    /// Stable node identifier that signed the bundle.
    pub signer_node_id: String,
    /// Hex-encoded Ed25519 verifying key used for the signature.
    pub signer_public_key: String,
    /// Hex-encoded Ed25519 signature over the bundle digest.
    pub signature_hex: String,
}

/// Signed cluster execution evidence bundle suitable for later export.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedClusterEvidenceBundle {
    /// Stable digest of the bundled payload.
    pub bundle_digest: String,
    /// Export payload retained for audit or dispute handling.
    pub payload: ClusterEvidenceBundlePayload,
    /// Signature over the stable payload digest.
    pub signature: ClusterEvidenceBundleSignature,
}

impl SignedClusterEvidenceBundle {
    /// Verifies that the payload digest and signature are both intact.
    pub fn verify(&self) -> Result<(), ClusterEvidenceBundleVerificationError> {
        let expected_digest = self.payload.stable_digest();
        if self.bundle_digest != expected_digest {
            return Err(ClusterEvidenceBundleVerificationError::DigestMismatch {
                expected: expected_digest,
                actual: self.bundle_digest.clone(),
            });
        }
        let verifying_key =
            decode_cluster_evidence_verifying_key(self.signature.signer_public_key.as_str())?;
        let signature = decode_cluster_evidence_signature(self.signature.signature_hex.as_str())?;
        verifying_key
            .verify(
                &cluster_evidence_bundle_signing_payload(self.bundle_digest.as_str()),
                &signature,
            )
            .map_err(|_| ClusterEvidenceBundleVerificationError::SignatureInvalid)?;
        Ok(())
    }
}

/// Verification failure while checking one signed cluster evidence bundle.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClusterEvidenceBundleVerificationError {
    /// The bundled payload digest no longer matches the payload bytes.
    #[error("cluster evidence bundle digest mismatch: expected {expected}, found {actual}")]
    DigestMismatch {
        /// Stable digest recomputed from the payload.
        expected: String,
        /// Stable digest claimed by the signed bundle.
        actual: String,
    },
    /// The bundled public key could not be decoded.
    #[error("cluster evidence bundle signer key is invalid")]
    SignerKeyInvalid,
    /// The bundled signature could not be decoded or did not verify.
    #[error("cluster evidence bundle signature is invalid")]
    SignatureInvalid,
}

fn cluster_evidence_bundle_signing_payload(bundle_digest: &str) -> Vec<u8> {
    let mut payload = Vec::with_capacity(bundle_digest.len() + 31);
    payload.extend_from_slice(b"cluster_evidence_bundle_signature|");
    payload.extend_from_slice(bundle_digest.as_bytes());
    payload
}

fn decode_cluster_evidence_verifying_key(
    signer_public_key: &str,
) -> Result<VerifyingKey, ClusterEvidenceBundleVerificationError> {
    let bytes = hex::decode(signer_public_key)
        .map_err(|_| ClusterEvidenceBundleVerificationError::SignerKeyInvalid)?;
    let verifying_key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ClusterEvidenceBundleVerificationError::SignerKeyInvalid)?;
    VerifyingKey::from_bytes(&verifying_key_bytes)
        .map_err(|_| ClusterEvidenceBundleVerificationError::SignerKeyInvalid)
}

fn decode_cluster_evidence_signature(
    signature_hex: &str,
) -> Result<Signature, ClusterEvidenceBundleVerificationError> {
    let bytes = hex::decode(signature_hex)
        .map_err(|_| ClusterEvidenceBundleVerificationError::SignatureInvalid)?;
    let signature_bytes: [u8; 64] = bytes
        .try_into()
        .map_err(|_| ClusterEvidenceBundleVerificationError::SignatureInvalid)?;
    Ok(Signature::from_bytes(&signature_bytes))
}

/// Returns the current runtime cache invalidation policy.
#[must_use]
pub fn default_cache_invalidation_policy() -> CacheInvalidationPolicy {
    CacheInvalidationPolicy {
        runtime_binary_version: String::from(env!("CARGO_PKG_VERSION")),
        execution_plan: CacheStorePolicy::new(
            CacheScope::ProcessLocal,
            EXECUTION_PLAN_CACHE_FORMAT_VERSION,
            CacheAction::Reuse,
            CacheAction::Rebuild,
            vec![
                CacheInvalidationTrigger::BinaryUpgrade,
                CacheInvalidationTrigger::BackendToolchainUpgrade,
                CacheInvalidationTrigger::ModelMetadataChange,
                CacheInvalidationTrigger::TokenizerDrift,
                CacheInvalidationTrigger::ChatTemplateDrift,
                CacheInvalidationTrigger::GenerationDefaultsDrift,
                CacheInvalidationTrigger::QuantizationChange,
                CacheInvalidationTrigger::PlanFormatUpgrade,
            ],
        ),
        kernel_cache: CacheStorePolicy::new(
            CacheScope::ProcessLocal,
            KERNEL_CACHE_FORMAT_VERSION,
            CacheAction::Reuse,
            CacheAction::Invalidate,
            vec![
                CacheInvalidationTrigger::BinaryUpgrade,
                CacheInvalidationTrigger::BackendToolchainUpgrade,
                CacheInvalidationTrigger::KernelFormatUpgrade,
            ],
        ),
        paged_tensor_storage: CacheStorePolicy::new(
            CacheScope::ArtifactBacked,
            PAGED_TENSOR_STORAGE_FORMAT_VERSION,
            CacheAction::Reuse,
            CacheAction::Restore,
            vec![
                CacheInvalidationTrigger::BinaryUpgrade,
                CacheInvalidationTrigger::ModelMetadataChange,
                CacheInvalidationTrigger::QuantizationChange,
                CacheInvalidationTrigger::PagedTensorFormatUpgrade,
            ],
        ),
        prefix_cache: CacheStorePolicy::new(
            CacheScope::SharedAcrossRequests,
            PREFIX_CACHE_FORMAT_VERSION,
            CacheAction::Reuse,
            CacheAction::Rebuild,
            vec![
                CacheInvalidationTrigger::BinaryUpgrade,
                CacheInvalidationTrigger::BackendToolchainUpgrade,
                CacheInvalidationTrigger::ModelMetadataChange,
                CacheInvalidationTrigger::TokenizerDrift,
                CacheInvalidationTrigger::ChatTemplateDrift,
                CacheInvalidationTrigger::GenerationDefaultsDrift,
                CacheInvalidationTrigger::QuantizationChange,
                CacheInvalidationTrigger::PrefixCacheFormatUpgrade,
            ],
        ),
        kv_state: CacheStorePolicy::new(
            CacheScope::SessionBound,
            KV_STATE_FORMAT_VERSION,
            CacheAction::Reuse,
            CacheAction::Invalidate,
            vec![
                CacheInvalidationTrigger::BinaryUpgrade,
                CacheInvalidationTrigger::BackendToolchainUpgrade,
                CacheInvalidationTrigger::ModelMetadataChange,
                CacheInvalidationTrigger::TokenizerDrift,
                CacheInvalidationTrigger::ChatTemplateDrift,
                CacheInvalidationTrigger::GenerationDefaultsDrift,
                CacheInvalidationTrigger::QuantizationChange,
                CacheInvalidationTrigger::KvStateFormatUpgrade,
            ],
        ),
    }
}

/// How one backend executes a typed extension family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendExtensionExecution {
    /// The backend executes the extension through a reference implementation.
    Reference,
    /// The backend executes the extension through a backend-specific fused/custom kernel.
    BackendSpecialized,
}

/// Explicit support declaration for one backend-extension family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendExtensionSupport {
    /// Backend-extension family.
    pub kind: BackendExtensionKind,
    /// Execution posture for that family.
    pub execution: BackendExtensionExecution,
}

impl BackendExtensionSupport {
    /// Creates reference-path support for one extension family.
    #[must_use]
    pub const fn reference(kind: BackendExtensionKind) -> Self {
        Self {
            kind,
            execution: BackendExtensionExecution::Reference,
        }
    }

    /// Creates backend-specialized support for one extension family.
    #[must_use]
    pub const fn backend_specialized(kind: BackendExtensionKind) -> Self {
        Self {
            kind,
            execution: BackendExtensionExecution::BackendSpecialized,
        }
    }
}

/// Distinct AMD runtime mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdRuntimeMode {
    /// Kernel-mediated AMD KFD posture using the standard `amdgpu` driver stack.
    Kfd,
    /// Explicitly opted-in userspace/AM-driver posture.
    Userspace,
}

/// Whether an AMD mode requires or has satisfied explicit opt-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdOptInStatus {
    /// The backend does not require an explicit opt-in gate.
    NotRequired,
    /// The backend is present but currently disabled until the operator opts in.
    Disabled,
    /// The operator has explicitly enabled the backend.
    Enabled,
}

/// Risk posture for an AMD backend mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdRiskLevel {
    /// Lower-risk operational posture.
    Standard,
    /// Higher-risk posture that needs stronger operator intent.
    Elevated,
}

/// Driver ownership/binding state relevant to AMD recovery posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdDriverBinding {
    /// The kernel `amdgpu` driver still owns the device.
    KernelAmdgpu,
    /// A userspace stack has taken ownership of the device.
    UserspaceClaimed,
    /// Psionic could not determine the binding state.
    Unknown,
}

/// Expected operator-level recovery step for an AMD backend mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdRecoveryAction {
    /// Restart the affected process/runtime first.
    ProcessRestart,
    /// Attempt a kernel-driver reset or recovery path.
    KernelDriverReset,
    /// Rebind or restore the kernel driver after userspace mode.
    RebindKernelDriver,
    /// Reboot the host when the runtime cannot recover in-place.
    RebootHost,
}

/// Stable AMD topology fields relevant to backend discovery and later capability reporting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdTopologyInfo {
    /// Stable architecture label such as `gfx1100`, when known.
    pub architecture: Option<String>,
    /// PCI bus/device/function address, when known.
    pub pci_bdf: Option<String>,
    /// Number of XCC partitions, when known.
    pub xcc_count: Option<u16>,
    /// Number of shader engines, when known.
    pub shader_engine_count: Option<u16>,
    /// Number of compute units, when known.
    pub compute_unit_count: Option<u16>,
    /// Total VRAM bytes, when known.
    pub vram_bytes: Option<u64>,
    /// Host-visible VRAM bytes, when known.
    pub visible_vram_bytes: Option<u64>,
}

/// Stable AMD risk posture derived from the backend/runtime mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdRiskProfile {
    /// High-level risk classification.
    pub level: AmdRiskLevel,
    /// Whether the mode requires explicit operator intent before activation.
    pub requires_explicit_opt_in: bool,
    /// Whether the mode may unbind or otherwise displace the kernel driver.
    pub may_unbind_kernel_driver: bool,
    /// Plain-text warnings the operator should see or preserve in logs.
    pub warnings: Vec<String>,
}

/// Stable AMD recovery posture derived from the backend/runtime mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdRecoveryProfile {
    /// Current or expected driver binding state.
    pub driver_binding: AmdDriverBinding,
    /// Ordered recovery actions Psionic expects the operator/runtime to consider.
    pub expected_actions: Vec<AmdRecoveryAction>,
}

/// AMD-specific device metadata carried through runtime and provider truth surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdDeviceMetadata {
    /// Runtime mode that discovered the device.
    pub mode: AmdRuntimeMode,
    /// Stable topology snapshot.
    pub topology: AmdTopologyInfo,
    /// Risk posture for the selected AMD mode.
    pub risk: AmdRiskProfile,
    /// Recovery posture for the selected AMD mode.
    pub recovery: AmdRecoveryProfile,
}

/// Backend-local AMD discovery report that preserves mode and opt-in truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdBackendReport {
    /// AMD backend mode represented by the report.
    pub mode: AmdRuntimeMode,
    /// Opt-in state for the backend mode.
    pub opt_in: AmdOptInStatus,
    /// Discovered devices for the mode.
    pub devices: Vec<DeviceDescriptor>,
    /// Honest readiness/health for the mode.
    pub health: RuntimeHealth,
}

/// High-level NVIDIA operational risk posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NvidiaRiskLevel {
    /// Lower-risk dedicated compute posture.
    Standard,
    /// Higher-risk posture such as display-attached or MIG-partitioned operation.
    Elevated,
}

/// Stable NVIDIA topology fields relevant to backend discovery and later capability reporting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NvidiaTopologyInfo {
    /// Stable architecture label such as `ada`, when known.
    pub architecture: Option<String>,
    /// Stable CUDA compute capability such as `8.9`, when known.
    pub compute_capability: Option<String>,
    /// PCI bus/device/function address, when known.
    pub pci_bdf: Option<String>,
    /// Number of streaming multiprocessors, when known.
    pub sm_count: Option<u16>,
    /// Total VRAM bytes, when known.
    pub vram_bytes: Option<u64>,
    /// Active MIG profile or partition label, when known.
    pub mig_profile: Option<String>,
}

/// Stable NVIDIA risk posture derived from the current topology and host role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NvidiaRiskProfile {
    /// High-level risk classification.
    pub level: NvidiaRiskLevel,
    /// Whether the GPU is believed to be attached to a live display, when known.
    pub display_attached: Option<bool>,
    /// Whether the device is a MIG partition or otherwise sharing the physical GPU.
    pub mig_partitioned: bool,
    /// Plain-text warnings the operator should preserve in logs or inventory surfaces.
    pub warnings: Vec<String>,
}

/// Expected operator-level recovery step for a CUDA backend/device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NvidiaRecoveryAction {
    /// Restart the affected process/runtime first.
    ProcessRestart,
    /// Attempt a GPU reset when the platform/driver permits it.
    GpuReset,
    /// Reboot the host when the runtime cannot recover in place.
    RebootHost,
}

/// Stable NVIDIA recovery posture derived from the current device and host mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NvidiaRecoveryProfile {
    /// Whether the runtime believes GPU reset is available on this host, when known.
    pub supports_gpu_reset: Option<bool>,
    /// Ordered recovery actions Psionic expects the operator/runtime to consider.
    pub expected_actions: Vec<NvidiaRecoveryAction>,
}

/// NVIDIA-specific device metadata carried through runtime and provider truth surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NvidiaDeviceMetadata {
    /// Stable topology snapshot.
    pub topology: NvidiaTopologyInfo,
    /// Risk posture for the selected NVIDIA device.
    pub risk: NvidiaRiskProfile,
    /// Recovery posture for the selected NVIDIA device.
    pub recovery: NvidiaRecoveryProfile,
}

/// Backend-local NVIDIA discovery report that preserves topology/risk truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NvidiaBackendReport {
    /// Discovered devices for the CUDA backend.
    pub devices: Vec<DeviceDescriptor>,
    /// Honest readiness/health for the backend.
    pub health: RuntimeHealth,
}

/// How a backend handles a quantization mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationExecution {
    /// Execute the quantized representation directly.
    Native,
    /// Dequantize weights to `f32` before execution.
    DequantizeToF32,
}

/// Explicit load/storage posture for a quantized mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationLoadPath {
    /// Weights arrive as ordinary dense `f32` tensors.
    DenseF32,
    /// The runtime loads quantized weights and immediately dequantizes them to `f32`.
    DequantizedF32,
    /// The runtime preserves quantized blocks in backend-owned storage.
    BackendQuantized,
}

/// Runtime support declaration for a quantization mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizationSupport {
    /// Supported quantization mode.
    pub mode: QuantizationMode,
    /// Explicit load/storage path for the quantized weights.
    pub load_path: QuantizationLoadPath,
    /// How the runtime executes that mode.
    pub execution: QuantizationExecution,
}

/// Runtime health state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Device/runtime is ready for work.
    Ready,
    /// Device/runtime can execute but with caveats.
    Degraded,
    /// Device/runtime cannot execute.
    Offline,
}

/// Health report for a runtime or backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeHealth {
    /// Current health status.
    pub status: HealthStatus,
    /// Plain-text explanation.
    pub message: String,
}

/// Default number of recent runtime transitions to retain for observability.
pub const DEFAULT_OBSERVABILITY_HISTORY_LIMIT: usize = 32;

/// Current observed health for one backend in the local runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendHealthObservation {
    /// Stable backend label such as `cpu` or `metal`.
    pub backend: String,
    /// Current observed health status.
    pub status: HealthStatus,
    /// Current observed health message.
    pub message: String,
    /// Timestamp when the backend was last observed.
    pub observed_at_millis: u64,
    /// Timestamp when the backend last changed health/message.
    pub changed_at_millis: u64,
}

/// Explicit runtime transition category surfaced for local observability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTransitionKind {
    /// A model was loaded and begins in the cold state.
    ModelLoadedCold,
    /// A previously cold model became warm after serving its first request.
    ModelBecameWarm,
    /// A model was unloaded or evicted.
    ModelUnloaded,
    /// A backend changed health posture.
    BackendHealthChanged,
}

/// One observable runtime transition for local lifecycle/debug reporting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeTransitionEvent {
    /// Transition category.
    pub kind: RuntimeTransitionKind,
    /// Model involved in the transition, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// Backend involved in the transition, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Previous backend health status when the transition is health-driven.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_status: Option<HealthStatus>,
    /// Current backend health status when the transition is health-driven.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<HealthStatus>,
    /// Human-readable transition detail when useful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Timestamp when the transition was observed.
    pub observed_at_millis: u64,
}

impl RuntimeTransitionEvent {
    /// Creates a model lifecycle transition.
    #[must_use]
    pub fn model(
        kind: RuntimeTransitionKind,
        model_id: impl Into<String>,
        observed_at_millis: u64,
    ) -> Self {
        Self {
            kind,
            model_id: Some(model_id.into()),
            backend: None,
            previous_status: None,
            status: None,
            message: None,
            observed_at_millis,
        }
    }

    /// Creates a backend-health transition.
    #[must_use]
    pub fn backend_health_changed(
        backend: impl Into<String>,
        previous: &RuntimeHealth,
        current: &RuntimeHealth,
        observed_at_millis: u64,
    ) -> Self {
        Self {
            kind: RuntimeTransitionKind::BackendHealthChanged,
            model_id: None,
            backend: Some(backend.into()),
            previous_status: Some(previous.status),
            status: Some(current.status),
            message: Some(current.message.clone()),
            observed_at_millis,
        }
    }
}

/// Bounded log of recent runtime transitions.
#[derive(Clone, Debug)]
pub struct RuntimeTransitionLog {
    limit: usize,
    events: VecDeque<RuntimeTransitionEvent>,
}

impl RuntimeTransitionLog {
    /// Creates a transition log with the default retention limit.
    #[must_use]
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_OBSERVABILITY_HISTORY_LIMIT)
    }

    /// Creates a transition log with an explicit retention limit.
    #[must_use]
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            events: VecDeque::new(),
        }
    }

    /// Records one runtime transition, dropping the oldest retained entry when full.
    pub fn record(&mut self, event: RuntimeTransitionEvent) {
        if self.events.len() == self.limit {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Returns the retained transitions in chronological order.
    #[must_use]
    pub fn snapshot(&self) -> Vec<RuntimeTransitionEvent> {
        self.events.iter().cloned().collect()
    }
}

impl Default for RuntimeTransitionLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks observed backend health and records health-change transitions.
#[derive(Clone, Debug)]
pub struct BackendHealthTracker {
    observed: BTreeMap<String, BackendHealthObservation>,
    changes: RuntimeTransitionLog,
}

impl BackendHealthTracker {
    /// Creates a health tracker with the default transition-history limit.
    #[must_use]
    pub fn new() -> Self {
        Self::with_history_limit(DEFAULT_OBSERVABILITY_HISTORY_LIMIT)
    }

    /// Creates a health tracker with an explicit transition-history limit.
    #[must_use]
    pub fn with_history_limit(limit: usize) -> Self {
        Self {
            observed: BTreeMap::new(),
            changes: RuntimeTransitionLog::with_limit(limit),
        }
    }

    /// Observes current health for one backend and records a change event when it differs.
    pub fn observe(
        &mut self,
        backend: impl Into<String>,
        health: RuntimeHealth,
        observed_at_millis: u64,
    ) -> BackendHealthObservation {
        let backend = backend.into();
        let entry =
            self.observed
                .entry(backend.clone())
                .or_insert_with(|| BackendHealthObservation {
                    backend: backend.clone(),
                    status: health.status,
                    message: health.message.clone(),
                    observed_at_millis,
                    changed_at_millis: observed_at_millis,
                });
        let previous = RuntimeHealth {
            status: entry.status,
            message: entry.message.clone(),
        };
        let changed = previous != health;
        entry.status = health.status;
        entry.message.clone_from(&health.message);
        entry.observed_at_millis = observed_at_millis;
        if changed {
            entry.changed_at_millis = observed_at_millis;
            self.changes
                .record(RuntimeTransitionEvent::backend_health_changed(
                    backend,
                    &previous,
                    &health,
                    observed_at_millis,
                ));
        }
        entry.clone()
    }

    /// Returns the currently observed backend-health rows in stable backend order.
    #[must_use]
    pub fn snapshot(&self) -> Vec<BackendHealthObservation> {
        self.observed.values().cloned().collect()
    }

    /// Returns recent health-change transitions in chronological order.
    #[must_use]
    pub fn recent_changes(&self) -> Vec<RuntimeTransitionEvent> {
        self.changes.snapshot()
    }
}

impl Default for BackendHealthTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Maximum token-history window used when applying repetition-style penalties.
pub const DEFAULT_PENALTY_LOOKBACK: usize = 64;

/// Runtime-owned token-selection strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingStrategy {
    /// Always choose the highest adjusted logit.
    Greedy,
    /// Draw from the adjusted probability distribution.
    Sample,
}

/// High-level determinism posture for one runtime path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeterminismMode {
    /// Permit runtime-owned nondeterministic behavior such as OS-seeded RNG.
    BestEffort,
    /// Require one replayable seeded generator but allow non-kernel determinism gaps elsewhere.
    SeededReplay,
    /// Require one replayable seeded generator plus explicit deterministic-algorithm posture.
    Strict,
}

/// Declared deterministic-algorithm posture for one runtime contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeterministicAlgorithmPolicy {
    /// Deterministic kernels are preferred when available, but not mandatory.
    PreferDeterministic,
    /// The caller requires deterministic kernels or an explicit typed refusal.
    RequireDeterministic,
}

/// Stable generator family for replayable runtime randomness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RngAlgorithmKind {
    /// Runtime replayable `StdRng` seed discipline.
    StdRngV1,
}

/// Stable derivation scope for one replayable generator state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum GeneratorScope {
    /// Root generator for one runtime contract.
    Root,
    /// Decode-token generator on one runtime.
    Decode,
    /// Eval-time generator on one runtime.
    Eval,
    /// Generator derived for one stable local device.
    LocalDevice {
        /// Stable device identifier surfaced by runtime discovery.
        stable_device_id: String,
    },
    /// Generator derived for one distributed rank.
    DistributedRank {
        /// Stable collective or replica-group identifier.
        replica_group: String,
        /// Rank inside that group.
        rank: usize,
        /// Declared world size for the group.
        world_size: usize,
    },
}

/// Replayable generator state for runtime-owned randomness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratorState {
    /// Stable generator family.
    pub algorithm: RngAlgorithmKind,
    /// Stable seed used to reconstruct the generator.
    pub seed: u64,
    /// Number of stochastic draws already consumed from the generator.
    pub draws: u64,
    /// Derivation scope for the generator.
    pub scope: GeneratorScope,
}

impl GeneratorState {
    /// Creates one root runtime generator from a stable seed.
    #[must_use]
    pub const fn root(seed: u64) -> Self {
        Self {
            algorithm: RngAlgorithmKind::StdRngV1,
            seed,
            draws: 0,
            scope: GeneratorScope::Root,
        }
    }

    /// Derives one child generator under a stable scope.
    #[must_use]
    pub fn derive_child(&self, scope: GeneratorScope) -> Self {
        Self {
            algorithm: self.algorithm,
            seed: stable_child_generator_seed(self.seed, &scope),
            draws: 0,
            scope,
        }
    }

    /// Returns a sampler-ready RNG reconstructed from the stored seed and draw count.
    #[must_use]
    pub fn restored_rng(&self) -> StdRng {
        let mut rng = StdRng::seed_from_u64(self.seed);
        for _ in 0..self.draws {
            let _ = rng.random::<f32>();
        }
        rng
    }
}

/// Typed failure from the framework-core determinism contract.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DeterminismContractError {
    /// Deterministic modes require an explicit generator seed.
    #[error("determinism mode `{mode:?}` requires a replayable generator state")]
    MissingGeneratorState {
        /// Determinism mode that required the generator.
        mode: DeterminismMode,
    },
    /// One distributed-rank derivation used invalid bounds.
    #[error(
        "distributed generator derivation requires rank < world_size and non-zero world_size; found rank={rank} world_size={world_size}"
    )]
    InvalidDistributedRank {
        /// Requested rank.
        rank: usize,
        /// Declared world size.
        world_size: usize,
    },
}

/// Runtime-owned determinism contract for replayable randomness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDeterminismContract {
    /// High-level determinism posture.
    pub mode: DeterminismMode,
    /// Deterministic-algorithm posture for the runtime path.
    pub algorithm_policy: DeterministicAlgorithmPolicy,
    /// Replayable generator state when the path is seeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator: Option<GeneratorState>,
}

impl RuntimeDeterminismContract {
    /// Returns a best-effort runtime determinism contract.
    #[must_use]
    pub const fn best_effort() -> Self {
        Self {
            mode: DeterminismMode::BestEffort,
            algorithm_policy: DeterministicAlgorithmPolicy::PreferDeterministic,
            generator: None,
        }
    }

    /// Returns a seeded replay contract from one root seed.
    #[must_use]
    pub const fn seeded(seed: u64) -> Self {
        Self {
            mode: DeterminismMode::SeededReplay,
            algorithm_policy: DeterministicAlgorithmPolicy::PreferDeterministic,
            generator: Some(GeneratorState::root(seed)),
        }
    }

    /// Returns a strict deterministic contract from one root seed.
    #[must_use]
    pub const fn strict(seed: u64) -> Self {
        Self {
            mode: DeterminismMode::Strict,
            algorithm_policy: DeterministicAlgorithmPolicy::RequireDeterministic,
            generator: Some(GeneratorState::root(seed)),
        }
    }

    /// Validates that required seeded state is present for the configured mode.
    pub fn validate(&self) -> Result<(), DeterminismContractError> {
        if self.mode != DeterminismMode::BestEffort && self.generator.is_none() {
            return Err(DeterminismContractError::MissingGeneratorState { mode: self.mode });
        }
        Ok(())
    }

    /// Returns the replayable generator state when the contract requires one.
    pub fn generator(&self) -> Result<&GeneratorState, DeterminismContractError> {
        self.validate()?;
        self.generator
            .as_ref()
            .ok_or(DeterminismContractError::MissingGeneratorState { mode: self.mode })
    }

    /// Derives one stable child generator for a local device.
    pub fn derive_local_device_generator(
        &self,
        stable_device_id: impl Into<String>,
    ) -> Result<GeneratorState, DeterminismContractError> {
        Ok(self.generator()?.derive_child(GeneratorScope::LocalDevice {
            stable_device_id: stable_device_id.into(),
        }))
    }

    /// Derives one stable child generator for a distributed rank.
    pub fn derive_distributed_rank_generator(
        &self,
        replica_group: impl Into<String>,
        rank: usize,
        world_size: usize,
    ) -> Result<GeneratorState, DeterminismContractError> {
        if world_size == 0 || rank >= world_size {
            return Err(DeterminismContractError::InvalidDistributedRank { rank, world_size });
        }
        Ok(self
            .generator()?
            .derive_child(GeneratorScope::DistributedRank {
                replica_group: replica_group.into(),
                rank,
                world_size,
            }))
    }

    /// Captures checkpoint-stable determinism state for later restore.
    pub fn checkpoint_state(
        &self,
        checkpoint: TrainingCheckpointReference,
    ) -> Result<GeneratorCheckpointState, DeterminismContractError> {
        self.validate()?;
        Ok(GeneratorCheckpointState {
            checkpoint,
            determinism: self.clone(),
        })
    }
}

/// Checkpoint-stable determinism snapshot for runtime-owned randomness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratorCheckpointState {
    /// Checkpoint identity bound to the snapshot.
    pub checkpoint: TrainingCheckpointReference,
    /// Determinism contract restored with the checkpoint.
    pub determinism: RuntimeDeterminismContract,
}

impl GeneratorCheckpointState {
    /// Restores the runtime determinism contract from one checkpoint snapshot.
    #[must_use]
    pub fn restore(&self) -> RuntimeDeterminismContract {
        self.determinism.clone()
    }
}

/// Reusable runtime sampling policy for token selection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SamplingPolicy {
    /// Sampling strategy.
    pub strategy: SamplingStrategy,
    /// Temperature override for stochastic sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-k sampling cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,
    /// Top-p / nucleus sampling threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Repeat penalty applied to previously seen tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,
    /// Presence penalty applied once to previously seen tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// Frequency penalty scaled by prior token count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// Deterministic seed for stochastic decode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

impl SamplingPolicy {
    /// Returns the effective temperature after applying runtime defaults.
    #[must_use]
    pub fn effective_temperature(&self) -> f32 {
        self.temperature.unwrap_or(0.8).max(0.0)
    }

    /// Returns the effective top-k cap after applying runtime defaults.
    #[must_use]
    pub fn effective_top_k(&self) -> Option<usize> {
        self.top_k.or(Some(40))
    }

    /// Returns the effective top-p threshold after applying runtime defaults.
    #[must_use]
    pub fn effective_top_p(&self) -> Option<f32> {
        self.top_p.or(Some(0.9))
    }

    /// Returns the effective repeat penalty after applying runtime defaults.
    #[must_use]
    pub fn effective_repeat_penalty(&self) -> f32 {
        self.repeat_penalty.unwrap_or(1.0)
    }

    /// Returns the effective presence penalty after applying runtime defaults.
    #[must_use]
    pub fn effective_presence_penalty(&self) -> f32 {
        self.presence_penalty.unwrap_or(0.0)
    }

    /// Returns the effective frequency penalty after applying runtime defaults.
    #[must_use]
    pub fn effective_frequency_penalty(&self) -> f32 {
        self.frequency_penalty.unwrap_or(0.0)
    }
}

/// Reusable runtime sampler with optional seeded replay.
#[derive(Clone, Debug)]
pub struct TokenSampler {
    policy: SamplingPolicy,
    generator_state: Option<GeneratorState>,
    rng: StdRng,
}

impl TokenSampler {
    /// Creates a token sampler for one runtime policy.
    #[must_use]
    pub fn new(policy: &SamplingPolicy) -> Self {
        let generator_state = policy.seed.map(GeneratorState::root);
        let rng = generator_state
            .as_ref()
            .map_or_else(StdRng::from_os_rng, GeneratorState::restored_rng);
        Self {
            policy: policy.clone(),
            generator_state,
            rng,
        }
    }

    /// Creates a sampler from an explicit runtime determinism contract.
    pub fn from_determinism_contract(
        policy: &SamplingPolicy,
        contract: &RuntimeDeterminismContract,
    ) -> Result<Self, DeterminismContractError> {
        contract.validate()?;
        let generator_state = contract.generator.clone();
        let rng = generator_state
            .as_ref()
            .map_or_else(StdRng::from_os_rng, GeneratorState::restored_rng);
        Ok(Self {
            policy: policy.clone(),
            generator_state,
            rng,
        })
    }

    /// Returns the runtime sampling policy.
    #[must_use]
    pub fn policy(&self) -> &SamplingPolicy {
        &self.policy
    }

    /// Returns the replayable generator state when the sampler is seeded.
    #[must_use]
    pub fn generator_state(&self) -> Option<GeneratorState> {
        self.generator_state.clone()
    }

    /// Selects the next token from logits and prior token history.
    pub fn select_next_token(&mut self, logits: &[f32], history: &[u32]) -> Option<u32> {
        let mut adjusted_logits = logits.to_vec();
        apply_sampling_penalties(&mut adjusted_logits, history, &self.policy);
        if self.policy.strategy == SamplingStrategy::Greedy
            || self.policy.effective_temperature() <= 1e-6
        {
            return select_argmax_token(&adjusted_logits);
        }
        let token = sample_token_index(&mut self.rng, &adjusted_logits, &self.policy);
        if token.is_some() {
            if let Some(generator_state) = self.generator_state.as_mut() {
                generator_state.draws = generator_state.draws.saturating_add(1);
            }
        }
        token
    }
}

fn stable_child_generator_seed(seed: u64, scope: &GeneratorScope) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_runtime_generator_child|");
    hasher.update(seed.to_string().as_bytes());
    match scope {
        GeneratorScope::Root => hasher.update(b"root"),
        GeneratorScope::Decode => hasher.update(b"decode"),
        GeneratorScope::Eval => hasher.update(b"eval"),
        GeneratorScope::LocalDevice { stable_device_id } => {
            hasher.update(b"local_device|");
            hasher.update(stable_device_id.as_bytes());
        }
        GeneratorScope::DistributedRank {
            replica_group,
            rank,
            world_size,
        } => {
            hasher.update(b"distributed_rank|");
            hasher.update(replica_group.as_bytes());
            hasher.update(rank.to_string().as_bytes());
            hasher.update(world_size.to_string().as_bytes());
        }
    }
    let digest = hasher.finalize();
    let mut seed_bytes = [0_u8; 8];
    seed_bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(seed_bytes)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SampleToken {
    id: u32,
    value: f32,
}

/// Applies repeat, presence, and frequency penalties using the bounded runtime history window.
pub fn apply_sampling_penalties(logits: &mut [f32], history: &[u32], policy: &SamplingPolicy) {
    let repeat_penalty = policy.effective_repeat_penalty();
    let presence_penalty = policy.effective_presence_penalty();
    let frequency_penalty = policy.effective_frequency_penalty();
    if (repeat_penalty - 1.0).abs() <= f32::EPSILON
        && presence_penalty.abs() <= f32::EPSILON
        && frequency_penalty.abs() <= f32::EPSILON
    {
        return;
    }

    for (token, count) in token_counts(history, logits.len()) {
        let Some(logit) = logits.get_mut(token as usize) else {
            continue;
        };
        if (repeat_penalty - 1.0).abs() > f32::EPSILON {
            if *logit < 0.0 {
                *logit *= repeat_penalty;
            } else {
                *logit /= repeat_penalty;
            }
        }
        if frequency_penalty.abs() > f32::EPSILON {
            *logit -= frequency_penalty * (count as f32);
        }
        if presence_penalty.abs() > f32::EPSILON {
            *logit -= presence_penalty;
        }
    }
}

/// Selects the highest-logit token index.
#[must_use]
pub fn select_argmax_token(logits: &[f32]) -> Option<u32> {
    logits
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(index, _)| index as u32)
}

fn token_counts(history: &[u32], vocab_size: usize) -> BTreeMap<u32, usize> {
    let start = history.len().saturating_sub(DEFAULT_PENALTY_LOOKBACK);
    let mut counts = BTreeMap::new();
    for &token in &history[start..] {
        if token as usize >= vocab_size {
            continue;
        }
        *counts.entry(token).or_insert(0) += 1;
    }
    counts
}

fn sample_token_index(rng: &mut StdRng, logits: &[f32], policy: &SamplingPolicy) -> Option<u32> {
    let temperature = policy.effective_temperature();
    if temperature <= 1e-6 {
        return select_argmax_token(logits);
    }

    let mut tokens = logits
        .iter()
        .enumerate()
        .map(|(index, value)| SampleToken {
            id: index as u32,
            value: *value,
        })
        .collect::<Vec<_>>();
    top_k(&mut tokens, policy.effective_top_k());
    temperature_scale(&mut tokens, temperature);
    let total = softmax(&mut tokens);
    if !total.is_finite() || total <= 0.0 {
        return None;
    }
    top_p(&mut tokens, policy.effective_top_p());

    let distribution_total = tokens.iter().map(|token| token.value).sum::<f32>();
    if !distribution_total.is_finite() || distribution_total <= 0.0 {
        return None;
    }

    let mut target = rng.random::<f32>() * distribution_total;
    for token in &tokens {
        target -= token.value;
        if target <= 0.0 {
            return Some(token.id);
        }
    }
    tokens.last().map(|token| token.id)
}

fn top_k(tokens: &mut Vec<SampleToken>, top_k: Option<usize>) {
    tokens.sort_by(|left, right| right.value.total_cmp(&left.value));
    let Some(top_k) = top_k else {
        return;
    };
    if top_k > 0 && top_k < tokens.len() {
        tokens.truncate(top_k);
    }
}

fn temperature_scale(tokens: &mut [SampleToken], temperature: f32) {
    let temperature = temperature.max(1e-7);
    for token in tokens {
        token.value /= temperature;
    }
}

fn softmax(tokens: &mut [SampleToken]) -> f32 {
    let Some(max_logit) = tokens
        .iter()
        .map(|token| token.value)
        .max_by(f32::total_cmp)
    else {
        return 0.0;
    };
    let mut sum = 0.0;
    for token in tokens.iter_mut() {
        token.value = (token.value - max_logit).exp();
        sum += token.value;
    }
    if !sum.is_finite() || sum <= 0.0 {
        return sum;
    }
    for token in tokens.iter_mut() {
        token.value /= sum;
    }
    sum
}

fn top_p(tokens: &mut Vec<SampleToken>, top_p: Option<f32>) {
    let Some(top_p) = top_p else {
        return;
    };
    if top_p <= 0.0 || top_p >= 1.0 {
        return;
    }

    let mut cumulative = 0.0;
    let mut keep = tokens.len();
    for (index, token) in tokens.iter().enumerate() {
        cumulative += token.value;
        if cumulative >= top_p {
            keep = index + 1;
            break;
        }
    }
    tokens.truncate(keep.max(1));
}

/// Lifecycle state for a model that is resident in a local runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadedModelState {
    /// The model is still warming/loading.
    Loading,
    /// The model is loaded and available for requests.
    Ready,
}

/// Explicit keepalive and residency truth for one loaded model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedModelResidency {
    /// Current lifecycle state.
    pub state: LoadedModelState,
    /// Number of active requests currently using the model.
    pub active_requests: usize,
    /// Configured keepalive duration in milliseconds.
    pub keep_alive_millis: u64,
    /// Time the current residency was established.
    pub loaded_at_millis: u64,
    /// Most recent time the model was touched by load/warm/request activity.
    pub last_used_at_millis: u64,
    /// Planned expiration time when the model is idle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_millis: Option<u64>,
}

impl LoadedModelResidency {
    /// Creates a loading residency record.
    #[must_use]
    pub fn loading(now_millis: u64, keep_alive_millis: u64) -> Self {
        Self {
            state: LoadedModelState::Loading,
            active_requests: 0,
            keep_alive_millis,
            loaded_at_millis: now_millis,
            last_used_at_millis: now_millis,
            expires_at_millis: Self::idle_expiration(now_millis, keep_alive_millis, 0),
        }
    }

    /// Creates a ready residency record.
    #[must_use]
    pub fn ready(now_millis: u64, keep_alive_millis: u64) -> Self {
        Self {
            state: LoadedModelState::Ready,
            active_requests: 0,
            keep_alive_millis,
            loaded_at_millis: now_millis,
            last_used_at_millis: now_millis,
            expires_at_millis: Self::idle_expiration(now_millis, keep_alive_millis, 0),
        }
    }

    /// Marks the model ready without changing its residency anchor.
    pub fn mark_ready(&mut self, now_millis: u64) {
        self.state = LoadedModelState::Ready;
        self.last_used_at_millis = now_millis;
        self.expires_at_millis =
            Self::idle_expiration(now_millis, self.keep_alive_millis, self.active_requests);
    }

    /// Refreshes keepalive and idle-expiration posture.
    pub fn refresh_keep_alive(&mut self, keep_alive_millis: u64, now_millis: u64) {
        self.keep_alive_millis = keep_alive_millis;
        self.last_used_at_millis = now_millis;
        self.expires_at_millis =
            Self::idle_expiration(now_millis, keep_alive_millis, self.active_requests);
    }

    /// Marks the start of a request using the model.
    pub fn begin_request(&mut self, now_millis: u64) {
        self.active_requests += 1;
        self.last_used_at_millis = now_millis;
        self.expires_at_millis = None;
    }

    /// Marks the completion of a request using the model.
    pub fn finish_request(&mut self, now_millis: u64) {
        if self.active_requests > 0 {
            self.active_requests -= 1;
        }
        self.last_used_at_millis = now_millis;
        self.expires_at_millis =
            Self::idle_expiration(now_millis, self.keep_alive_millis, self.active_requests);
    }

    /// Forces the model to expire immediately once idle.
    pub fn expire_now(&mut self, now_millis: u64) {
        self.keep_alive_millis = 0;
        self.last_used_at_millis = now_millis;
        self.expires_at_millis = Some(now_millis);
    }

    /// Returns whether the model should be unloaded at the provided time.
    #[must_use]
    pub fn is_expired(&self, now_millis: u64) -> bool {
        self.active_requests == 0
            && self
                .expires_at_millis
                .is_some_and(|expires_at_millis| expires_at_millis <= now_millis)
    }

    fn idle_expiration(
        now_millis: u64,
        keep_alive_millis: u64,
        active_requests: usize,
    ) -> Option<u64> {
        if active_requests > 0 {
            None
        } else {
            now_millis.checked_add(keep_alive_millis)
        }
    }
}

/// Explicit resident-memory plan for one served model.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMemoryPlan {
    /// Bytes attributable to model weights.
    pub weights_bytes: u64,
    /// Bytes attributable to the admitted KV-cache posture.
    pub kv_cache_bytes: u64,
    /// Bytes attributable to graph or runtime workspace planning.
    pub graph_bytes: u64,
    /// Resident host-memory bytes the runtime must admit for the model.
    pub resident_host_bytes: u64,
    /// Resident device-memory bytes the runtime must admit for the model.
    pub resident_device_bytes: u64,
}

impl ModelMemoryPlan {
    /// Creates a host-only memory plan for CPU-backed serving.
    #[must_use]
    pub fn host_only(weights_bytes: u64, kv_cache_bytes: u64, graph_bytes: u64) -> Self {
        let resident_host_bytes = weights_bytes
            .saturating_add(kv_cache_bytes)
            .saturating_add(graph_bytes);
        Self {
            weights_bytes,
            kv_cache_bytes,
            graph_bytes,
            resident_host_bytes,
            resident_device_bytes: 0,
        }
    }

    /// Creates a plan with an explicit host/device residency split.
    #[must_use]
    pub fn split_residency(
        weights_bytes: u64,
        kv_cache_bytes: u64,
        graph_bytes: u64,
        resident_host_bytes: u64,
        resident_device_bytes: u64,
    ) -> Self {
        Self {
            weights_bytes,
            kv_cache_bytes,
            graph_bytes,
            resident_host_bytes,
            resident_device_bytes,
        }
    }
}

/// Explicit resident-memory budgets for local-serving admission.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryBudget {
    /// Maximum admitted resident host-memory bytes, when bounded.
    pub resident_host_bytes: Option<u64>,
    /// Maximum admitted resident device-memory bytes, when bounded.
    pub resident_device_bytes: Option<u64>,
}

/// Policy to apply when a new model would exceed admitted residency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidencyPressureAction {
    /// Refuse the new model instead of evicting anything implicitly.
    RefuseNewModel,
    /// Unload the oldest idle models first until the new model fits.
    UnloadIdleOldestFirst,
}

/// Reusable local-serving residency and admission policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelResidencyPolicy {
    /// Maximum number of simultaneously loaded models, when bounded.
    pub max_loaded_models: Option<usize>,
    /// Explicit admitted resident-memory budgets.
    pub memory_budget: MemoryBudget,
    /// What to do when the candidate would exceed the admitted budgets.
    pub pressure_action: ResidencyPressureAction,
}

impl ModelResidencyPolicy {
    /// Creates an explicit unbounded policy.
    #[must_use]
    pub fn unbounded() -> Self {
        Self {
            max_loaded_models: None,
            memory_budget: MemoryBudget::default(),
            pressure_action: ResidencyPressureAction::RefuseNewModel,
        }
    }
}

impl Default for ModelResidencyPolicy {
    fn default() -> Self {
        Self::unbounded()
    }
}

/// Runtime-visible memory state for one loaded model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedModelMemoryState {
    /// Stable model identifier.
    pub model_id: String,
    /// Explicit resident-memory plan for the model.
    pub plan: ModelMemoryPlan,
    /// Number of active requests currently using the model.
    pub active_requests: usize,
    /// Most recent activity time used for idle-eviction ordering.
    pub last_used_at_millis: u64,
}

/// Aggregate resident-memory snapshot for the currently loaded model set.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryResidencySnapshot {
    /// Number of loaded models in the snapshot.
    pub loaded_models: usize,
    /// Aggregate admitted host-memory bytes.
    pub resident_host_bytes: u64,
    /// Aggregate admitted device-memory bytes.
    pub resident_device_bytes: u64,
}

impl MemoryResidencySnapshot {
    /// Builds a snapshot from the currently loaded models.
    #[must_use]
    pub fn from_loaded_models(models: &[LoadedModelMemoryState]) -> Self {
        Self {
            loaded_models: models.len(),
            resident_host_bytes: models
                .iter()
                .map(|model| model.plan.resident_host_bytes)
                .sum(),
            resident_device_bytes: models
                .iter()
                .map(|model| model.plan.resident_device_bytes)
                .sum(),
        }
    }
}

/// How the local serving runtime crosses the backend boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendInterfaceMode {
    /// Backend code runs in the same process as the caller.
    InProcess,
    /// Backend work is isolated behind a dedicated subprocess boundary.
    Subprocess,
}

/// Process boundary that contains a backend or runtime crash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationFailureBoundary {
    /// A crash can take down the shared host process.
    SharedHostProcess,
    /// A crash is contained to a dedicated runtime subprocess.
    DedicatedRuntimeSubprocess,
}

/// State that must be discarded when the runtime performs an isolation reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationResetScope {
    /// Discard loaded-model state and residency.
    LoadedModels,
    /// Discard live generation sessions.
    Sessions,
    /// Discard shared prefix-cache entries.
    PrefixCache,
    /// Discard paged KV state.
    KvState,
    /// Discard backend-owned allocator and kernel-cache state.
    BackendRuntimeResources,
}

/// Recovery action the runtime must take after an isolation-relevant failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationRecoveryAction {
    /// Refuse the affected request and preserve the rest of the runtime state.
    RefuseRequest,
    /// Reset runtime-owned loaded-model, session, cache, and backend-resource state.
    ResetRuntimeState,
    /// Restart only the dedicated runtime subprocess.
    RestartRuntimeSubprocess,
    /// Restart the whole host process because no smaller crash boundary exists.
    RestartHostProcess,
}

/// Explicit isolation policy for local Psionic serving.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalServingIsolationPolicy {
    /// Whether the backend boundary is in-process or subprocess-isolated.
    pub backend_interface_mode: BackendInterfaceMode,
    /// Smallest process boundary that contains a crash.
    pub failure_boundary: IsolationFailureBoundary,
    /// Recovery action for request-local failures such as invalid input or unsupported capability.
    pub request_failure_recovery: IsolationRecoveryAction,
    /// Recovery action for backend execution failures that return control to the caller.
    pub backend_error_recovery: IsolationRecoveryAction,
    /// Recovery action when the backend/runtime crashes outright.
    pub crash_recovery: IsolationRecoveryAction,
    /// State that is considered unsafe and must be discarded during an isolation reset.
    pub reset_scopes: Vec<IsolationResetScope>,
}

impl LocalServingIsolationPolicy {
    /// Returns the current in-process Psionic serving policy.
    #[must_use]
    pub fn in_process_runtime() -> Self {
        Self {
            backend_interface_mode: BackendInterfaceMode::InProcess,
            failure_boundary: IsolationFailureBoundary::SharedHostProcess,
            request_failure_recovery: IsolationRecoveryAction::RefuseRequest,
            backend_error_recovery: IsolationRecoveryAction::ResetRuntimeState,
            crash_recovery: IsolationRecoveryAction::RestartHostProcess,
            reset_scopes: vec![
                IsolationResetScope::LoadedModels,
                IsolationResetScope::Sessions,
                IsolationResetScope::PrefixCache,
                IsolationResetScope::KvState,
                IsolationResetScope::BackendRuntimeResources,
            ],
        }
    }

    /// Returns the target policy if Psionic later isolates execution behind a subprocess.
    #[must_use]
    pub fn subprocess_runtime() -> Self {
        Self {
            backend_interface_mode: BackendInterfaceMode::Subprocess,
            failure_boundary: IsolationFailureBoundary::DedicatedRuntimeSubprocess,
            request_failure_recovery: IsolationRecoveryAction::RefuseRequest,
            backend_error_recovery: IsolationRecoveryAction::RestartRuntimeSubprocess,
            crash_recovery: IsolationRecoveryAction::RestartRuntimeSubprocess,
            reset_scopes: vec![
                IsolationResetScope::LoadedModels,
                IsolationResetScope::Sessions,
                IsolationResetScope::PrefixCache,
                IsolationResetScope::KvState,
                IsolationResetScope::BackendRuntimeResources,
            ],
        }
    }
}

impl Default for LocalServingIsolationPolicy {
    fn default() -> Self {
        Self::in_process_runtime()
    }
}

/// Advertised batch execution posture for one served path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchExecutionPosture {
    /// One request executes at a time without admitted multi-request batching.
    SingleRequestOnly,
    /// Callers may submit one bounded batch in a single request.
    CallerStaticBatch,
    /// The runtime may combine compatible requests into a static shared batch.
    SchedulerStaticBatch,
    /// The runtime supports continuous batching across active requests.
    ContinuousBatch,
}

/// How the runtime admits work before execution starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueDiscipline {
    /// The caller provides backpressure directly; there is no internal queue.
    DirectCallerBackpressure,
    /// The runtime admits work into a first-in/first-out queue.
    Fifo,
}

/// Explicit queueing policy for one served path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuePolicy {
    /// Queue discipline used before execution starts.
    pub discipline: QueueDiscipline,
    /// Maximum concurrently active requests admitted by the runtime.
    pub max_active_requests: usize,
    /// Maximum queued requests admitted behind active execution.
    pub max_queued_requests: usize,
    /// Whether the runtime serializes execution per loaded model.
    pub per_model_serialization: bool,
}

impl QueuePolicy {
    /// Direct caller-owned backpressure with one active request and no internal queue.
    #[must_use]
    pub const fn direct_caller_serial() -> Self {
        Self {
            discipline: QueueDiscipline::DirectCallerBackpressure,
            max_active_requests: 1,
            max_queued_requests: 0,
            per_model_serialization: true,
        }
    }

    /// Runtime-owned FIFO queueing for a shared per-model scheduler.
    #[must_use]
    pub const fn scheduler_fifo(max_active_requests: usize, max_queued_requests: usize) -> Self {
        Self {
            discipline: QueueDiscipline::Fifo,
            max_active_requests,
            max_queued_requests,
            per_model_serialization: true,
        }
    }
}

/// High-level throughput category for one served path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputClass {
    /// Optimized for low-latency single-request execution.
    LatencyOptimized,
    /// Balanced around caller-supplied bounded batches.
    Balanced,
    /// Optimized for throughput via runtime-owned batching.
    ThroughputOptimized,
}

/// Workload class for quantized runtime dispatch decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationDispatchWorkload {
    /// Single-token or otherwise latency-critical decode work.
    LatencyCriticalDecode,
    /// Multi-token batched prefill or bounded batch execution.
    BatchedPrefill,
    /// Grouped expert or fused-id dispatch across quantized experts.
    GroupedExpert,
    /// Collective or shard-side quantized work across multiple devices.
    CollectiveShard,
}

/// Runtime-selected low-level kernel family for one quantized path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationKernelStrategy {
    /// Dense/unquantized fallback path.
    DenseF32,
    /// Native backend int8 kernels remain available.
    NativeInt8,
    /// Native backend block-quantized kernels remain available.
    NativeBlock,
    /// Grouped expert block-quantized kernels remain available.
    GroupedBlock,
    /// The runtime must dequantize one batch before dense execution.
    DequantizePerBatch,
}

/// Machine-checkable runtime input for quantized dispatch planning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizationDispatchRequest {
    /// Weight quantization mode under consideration.
    pub mode: QuantizationMode,
    /// Workload class for the planned execution.
    pub workload: QuantizationDispatchWorkload,
    /// Logical token count or batch width of the work.
    pub logical_tokens: usize,
    /// Matrix width or comparable compute width for the hot kernel.
    pub matrix_columns: usize,
    /// Whether the backend exposes native quantized kernels for the mode.
    pub supports_native_quantized_kernels: bool,
    /// Whether the backend exposes grouped expert / grouped-id dispatch.
    pub supports_grouped_dispatch: bool,
}

impl QuantizationDispatchRequest {
    /// Creates a quantized dispatch request from explicit workload geometry.
    #[must_use]
    pub fn new(
        mode: QuantizationMode,
        workload: QuantizationDispatchWorkload,
        logical_tokens: usize,
        matrix_columns: usize,
    ) -> Self {
        Self {
            mode,
            workload,
            logical_tokens: logical_tokens.max(1),
            matrix_columns: matrix_columns.max(1),
            supports_native_quantized_kernels: false,
            supports_grouped_dispatch: false,
        }
    }

    /// Declares whether native quantized kernels are available for the mode.
    #[must_use]
    pub const fn with_native_quantized_kernels(
        mut self,
        supports_native_quantized_kernels: bool,
    ) -> Self {
        self.supports_native_quantized_kernels = supports_native_quantized_kernels;
        self
    }

    /// Declares whether grouped expert dispatch is available.
    #[must_use]
    pub const fn with_grouped_dispatch(mut self, supports_grouped_dispatch: bool) -> Self {
        self.supports_grouped_dispatch = supports_grouped_dispatch;
        self
    }
}

/// Runtime-selected low-level quantization dispatch decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizationDispatchDecision {
    /// Chosen low-level kernel strategy.
    pub strategy: QuantizationKernelStrategy,
    /// Number of tokens the runtime should tile together on the hot path.
    pub tile_tokens: usize,
    /// Suggested worker width for the kernel family.
    pub worker_width: usize,
    /// Simulated cost units used by repeatable validation harnesses.
    pub estimated_cost_units: u64,
    /// Human-readable detail for the selected strategy.
    pub detail: String,
}

impl QuantizationDispatchDecision {
    /// Chooses a runtime-owned quantization strategy from explicit workload facts.
    #[must_use]
    pub fn advise(request: &QuantizationDispatchRequest) -> Self {
        let logical_tokens = request.logical_tokens.max(1);
        let matrix_columns = request.matrix_columns.max(1);
        let baseline_units = logical_tokens.saturating_mul(matrix_columns) as u64;
        let (strategy, numerator, denominator, tile_tokens, worker_width, detail) = match request
            .mode
        {
            QuantizationMode::None => (
                QuantizationKernelStrategy::DenseF32,
                100_u64,
                100_u64,
                logical_tokens.min(4),
                1,
                String::from("dense path remains truthful for unquantized weights"),
            ),
            QuantizationMode::Int8Symmetric if request.supports_native_quantized_kernels => (
                QuantizationKernelStrategy::NativeInt8,
                62,
                100,
                logical_tokens.min(8),
                2,
                String::from("native int8 kernels avoid dense dequantization staging"),
            ),
            QuantizationMode::GgmlMxfp4
            | QuantizationMode::GgmlQ4_0
            | QuantizationMode::GgmlQ4_1
            | QuantizationMode::GgmlQ8_0
                if request.supports_native_quantized_kernels
                    && request.supports_grouped_dispatch
                    && request.workload == QuantizationDispatchWorkload::GroupedExpert =>
            {
                (
                    QuantizationKernelStrategy::GroupedBlock,
                    44,
                    100,
                    logical_tokens.min(8),
                    4,
                    String::from(
                        "grouped block dispatch keeps expert fan-out on the quantized path",
                    ),
                )
            }
            QuantizationMode::GgmlMxfp4
            | QuantizationMode::GgmlQ4_0
            | QuantizationMode::GgmlQ4_1
            | QuantizationMode::GgmlQ8_0
                if request.supports_native_quantized_kernels =>
            {
                (
                    QuantizationKernelStrategy::NativeBlock,
                    57,
                    100,
                    logical_tokens.min(8),
                    3,
                    String::from(
                        "native block kernels avoid dense expansion for quantized weights",
                    ),
                )
            }
            _ => {
                let latency_sensitive =
                    request.workload == QuantizationDispatchWorkload::LatencyCriticalDecode;
                (
                    QuantizationKernelStrategy::DequantizePerBatch,
                    if latency_sensitive { 84 } else { 72 },
                    100,
                    if latency_sensitive {
                        1
                    } else {
                        logical_tokens.min(8)
                    },
                    2,
                    String::from(
                        "batch-local dequantization is the least-worst fallback without native quantized kernels",
                    ),
                )
            }
        };
        let estimated_cost_units = baseline_units
            .saturating_mul(numerator)
            .div_ceil(denominator.max(1));
        Self {
            strategy,
            tile_tokens: tile_tokens.max(1),
            worker_width: worker_width.max(1),
            estimated_cost_units,
            detail,
        }
    }
}

/// Runtime work class for low-level batching and worker wake decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeWorkClass {
    /// One latency-sensitive decode step.
    DecodeToken,
    /// One batched prefill or compile-time preparation step.
    PrefillBatch,
    /// One trainer-step workload.
    TrainingStep,
    /// One rollout-generation workload.
    RolloutStep,
    /// One evaluation workload.
    EvalStep,
    /// One sandbox execution workload.
    SandboxStep,
    /// One validator-owned verification workload.
    ValidatorStep,
    /// One datastream chunk transfer.
    DatastreamChunk,
    /// One collective or shard synchronization step.
    CollectiveStep,
    /// One checkpoint flush or artifact persistence step.
    CheckpointFlush,
}

/// One runtime work item admitted into the low-level planner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeWorkItem {
    /// High-level work class.
    pub class: RuntimeWorkClass,
    /// Relative compute units used by the planner.
    pub work_units: usize,
    /// Number of bytes moved or touched by the work item.
    pub bytes: u64,
    /// Whether the item is latency-sensitive and should avoid batching delay.
    pub latency_sensitive: bool,
}

impl RuntimeWorkItem {
    /// Creates one runtime work item from explicit class, work units, and byte volume.
    #[must_use]
    pub fn new(class: RuntimeWorkClass, work_units: usize, bytes: u64) -> Self {
        Self {
            class,
            work_units: work_units.max(1),
            bytes,
            latency_sensitive: false,
        }
    }

    /// Marks the work item as latency-sensitive.
    #[must_use]
    pub const fn latency_sensitive(mut self) -> Self {
        self.latency_sensitive = true;
        self
    }
}

/// Runtime-owned batching and parking policy for low-level worker scheduling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDispatchPolicy {
    /// Maximum workers the planner may wake for one batch.
    pub max_workers: usize,
    /// Target work units per batch before the planner emits another wake boundary.
    pub target_batch_work_units: usize,
    /// Maximum bytes admitted into one batch before the planner emits another wake boundary.
    pub max_batch_bytes: u64,
    /// Idle-batch threshold after which workers should be considered parked.
    pub park_after_idle_batches: usize,
}

impl RuntimeDispatchPolicy {
    /// Default policy for latency-critical quantized decode work.
    #[must_use]
    pub fn quantized_decode_default(max_workers: usize) -> Self {
        Self {
            max_workers: max_workers.max(1),
            target_batch_work_units: 2,
            max_batch_bytes: 256 * 1024,
            park_after_idle_batches: 1,
        }
    }

    /// Default policy for streaming dataset/checkpoint delivery.
    #[must_use]
    pub fn data_plane_default(max_workers: usize) -> Self {
        Self {
            max_workers: max_workers.max(1),
            target_batch_work_units: 4,
            max_batch_bytes: 4 * 1024 * 1024,
            park_after_idle_batches: 4,
        }
    }
}

/// One concrete worker batch emitted by the low-level planner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDispatchBatch {
    /// Number of items fused into the batch.
    pub item_count: usize,
    /// Aggregate work units fused into the batch.
    pub total_work_units: usize,
    /// Aggregate byte volume fused into the batch.
    pub total_bytes: u64,
    /// Number of workers the planner would wake for the batch.
    pub worker_count: usize,
}

/// Runtime-owned worker dispatch plan with simulated wake/park costs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDispatchPlan {
    /// Policy used to plan the work.
    pub policy: RuntimeDispatchPolicy,
    /// Concrete worker batches emitted for the supplied work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batches: Vec<RuntimeDispatchBatch>,
    /// Total wake boundaries emitted by the planner.
    pub total_wake_events: usize,
    /// Number of workers likely parked under the idle policy.
    pub parked_workers: usize,
}

impl RuntimeDispatchPlan {
    /// Plans low-level worker batches from the supplied work items.
    #[must_use]
    pub fn plan(policy: RuntimeDispatchPolicy, items: &[RuntimeWorkItem]) -> Self {
        if items.is_empty() {
            return Self {
                parked_workers: policy.max_workers.saturating_sub(1),
                policy,
                batches: Vec::new(),
                total_wake_events: 0,
            };
        }

        let mut batches = Vec::new();
        let mut batch_work_units = 0usize;
        let mut batch_bytes = 0u64;
        let mut batch_items = 0usize;

        for item in items {
            let should_flush = batch_items > 0
                && (item.latency_sensitive
                    || batch_work_units.saturating_add(item.work_units)
                        > policy.target_batch_work_units
                    || batch_bytes.saturating_add(item.bytes) > policy.max_batch_bytes);
            if should_flush {
                batches.push(RuntimeDispatchBatch {
                    item_count: batch_items,
                    total_work_units: batch_work_units,
                    total_bytes: batch_bytes,
                    worker_count: batch_work_units.min(policy.max_workers).max(1),
                });
                batch_work_units = 0;
                batch_bytes = 0;
                batch_items = 0;
            }

            batch_work_units = batch_work_units.saturating_add(item.work_units);
            batch_bytes = batch_bytes.saturating_add(item.bytes);
            batch_items = batch_items.saturating_add(1);

            if item.latency_sensitive {
                batches.push(RuntimeDispatchBatch {
                    item_count: batch_items,
                    total_work_units: batch_work_units,
                    total_bytes: batch_bytes,
                    worker_count: batch_work_units.min(policy.max_workers).max(1),
                });
                batch_work_units = 0;
                batch_bytes = 0;
                batch_items = 0;
            }
        }

        if batch_items > 0 {
            batches.push(RuntimeDispatchBatch {
                item_count: batch_items,
                total_work_units: batch_work_units,
                total_bytes: batch_bytes,
                worker_count: batch_work_units.min(policy.max_workers).max(1),
            });
        }

        let total_wake_events = batches.len();
        let parked_workers = policy
            .max_workers
            .saturating_sub(batches.last().map_or(0, |batch| batch.worker_count));
        Self {
            policy,
            batches,
            total_wake_events,
            parked_workers,
        }
    }

    /// Returns a deterministic cost model for validation harnesses.
    #[must_use]
    pub fn simulated_cost_units(&self) -> u64 {
        let batch_cost = self
            .batches
            .iter()
            .map(|batch| {
                batch.total_work_units as u64
                    + batch.worker_count as u64 * 3
                    + batch.total_bytes.div_ceil(512 * 1024)
            })
            .sum::<u64>();
        batch_cost
            .saturating_add(self.total_wake_events as u64 * 5)
            .saturating_add(self.parked_workers as u64 * self.policy.park_after_idle_batches as u64)
    }

    /// Returns a naive one-item-per-batch baseline for the same work items.
    #[must_use]
    pub fn naive(items: &[RuntimeWorkItem], max_workers: usize) -> Self {
        let max_workers = max_workers.max(1);
        Self {
            policy: RuntimeDispatchPolicy {
                max_workers,
                target_batch_work_units: 1,
                max_batch_bytes: 0,
                park_after_idle_batches: 1,
            },
            batches: items
                .iter()
                .map(|item| RuntimeDispatchBatch {
                    item_count: 1,
                    total_work_units: item.work_units,
                    total_bytes: item.bytes,
                    worker_count: 1.min(max_workers),
                })
                .collect(),
            total_wake_events: items.len(),
            parked_workers: max_workers.saturating_sub(1),
        }
    }
}

/// Repeatable benchmark result for one runtime optimization decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeOptimizationBenchmark {
    /// Stable benchmark scenario identifier.
    pub scenario: String,
    /// Simulated baseline cost units.
    pub baseline_cost_units: u64,
    /// Simulated optimized cost units.
    pub optimized_cost_units: u64,
    /// Improvement in basis points relative to the baseline.
    pub improvement_basis_points: u32,
}

impl RuntimeOptimizationBenchmark {
    /// Creates a benchmark result from baseline and optimized cost units.
    #[must_use]
    pub fn new(
        scenario: impl Into<String>,
        baseline_cost_units: u64,
        optimized_cost_units: u64,
    ) -> Self {
        let improvement_basis_points =
            if baseline_cost_units == 0 || optimized_cost_units >= baseline_cost_units {
                0
            } else {
                let improvement = baseline_cost_units.saturating_sub(optimized_cost_units);
                improvement
                    .saturating_mul(10_000)
                    .div_ceil(baseline_cost_units)
                    .try_into()
                    .unwrap_or(u32::MAX)
            };
        Self {
            scenario: scenario.into(),
            baseline_cost_units,
            optimized_cost_units,
            improvement_basis_points,
        }
    }
}

/// Returns a repeatable quantization-dispatch benchmark for one workload.
#[must_use]
pub fn benchmark_quantization_dispatch(
    request: &QuantizationDispatchRequest,
) -> RuntimeOptimizationBenchmark {
    let baseline = (request
        .logical_tokens
        .saturating_mul(request.matrix_columns)) as u64;
    let optimized = QuantizationDispatchDecision::advise(request).estimated_cost_units;
    RuntimeOptimizationBenchmark::new("quantization_dispatch", baseline, optimized)
}

/// Returns a repeatable worker scheduling benchmark for one workload.
#[must_use]
pub fn benchmark_dispatch_plan(
    scenario: impl Into<String>,
    policy: RuntimeDispatchPolicy,
    items: &[RuntimeWorkItem],
) -> RuntimeOptimizationBenchmark {
    let baseline = RuntimeDispatchPlan::naive(items, policy.max_workers).simulated_cost_units();
    let optimized = RuntimeDispatchPlan::plan(policy, items).simulated_cost_units();
    RuntimeOptimizationBenchmark::new(scenario, baseline, optimized)
}

/// Reusable execution profile for capability and observability surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionCapabilityProfile {
    /// Advertised batch posture for the served path.
    pub batch_posture: BatchExecutionPosture,
    /// Explicit queueing policy for the served path.
    pub queue_policy: QueuePolicy,
    /// High-level throughput class for the served path.
    pub throughput_class: ThroughputClass,
    /// Explicit prompt-prefill/decode split truth for the served path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefill_decode_capability: Option<PrefillDecodeCapability>,
}

impl ExecutionCapabilityProfile {
    /// Single-request, direct-caller execution profile.
    #[must_use]
    pub const fn single_request_latency_optimized() -> Self {
        Self {
            batch_posture: BatchExecutionPosture::SingleRequestOnly,
            queue_policy: QueuePolicy::direct_caller_serial(),
            throughput_class: ThroughputClass::LatencyOptimized,
            prefill_decode_capability: None,
        }
    }

    /// Caller-batched execution profile with direct caller backpressure.
    #[must_use]
    pub const fn caller_static_batch_balanced() -> Self {
        Self {
            batch_posture: BatchExecutionPosture::CallerStaticBatch,
            queue_policy: QueuePolicy::direct_caller_serial(),
            throughput_class: ThroughputClass::Balanced,
            prefill_decode_capability: None,
        }
    }

    /// Runtime-owned continuous batching with explicit FIFO queueing.
    #[must_use]
    pub fn continuous_batch_throughput_optimized(policy: &GenerationSchedulerPolicy) -> Self {
        Self {
            batch_posture: BatchExecutionPosture::ContinuousBatch,
            queue_policy: QueuePolicy::scheduler_fifo(
                policy.max_active_requests,
                policy.max_queued_requests,
            ),
            throughput_class: ThroughputClass::ThroughputOptimized,
            prefill_decode_capability: None,
        }
    }

    /// Attaches explicit prefill/decode split truth for the served path.
    #[must_use]
    pub fn with_prefill_decode_capability(
        mut self,
        prefill_decode_capability: PrefillDecodeCapability,
    ) -> Self {
        self.prefill_decode_capability = Some(prefill_decode_capability);
        self
    }

    /// Returns a stable digest for the execution profile.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"execution_capability_profile|");
        hasher.update(match self.batch_posture {
            BatchExecutionPosture::SingleRequestOnly => b"single_request_only".as_slice(),
            BatchExecutionPosture::CallerStaticBatch => b"caller_static_batch".as_slice(),
            BatchExecutionPosture::SchedulerStaticBatch => b"scheduler_static_batch".as_slice(),
            BatchExecutionPosture::ContinuousBatch => b"continuous_batch".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(match self.queue_policy.discipline {
            QueueDiscipline::DirectCallerBackpressure => b"direct_caller_backpressure".as_slice(),
            QueueDiscipline::Fifo => b"fifo".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(self.queue_policy.max_active_requests.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.queue_policy.max_queued_requests.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(if self.queue_policy.per_model_serialization {
            b"per_model_serialization".as_slice()
        } else {
            b"cross_model_parallel".as_slice()
        });
        hasher.update(b"|");
        hasher.update(match self.throughput_class {
            ThroughputClass::LatencyOptimized => b"latency_optimized".as_slice(),
            ThroughputClass::Balanced => b"balanced".as_slice(),
            ThroughputClass::ThroughputOptimized => b"throughput_optimized".as_slice(),
        });
        if let Some(prefill_decode_capability) = &self.prefill_decode_capability {
            for mode in &prefill_decode_capability.supported_modes {
                hasher.update(b"|prefill_decode_mode|");
                hasher.update(mode.as_str().as_bytes());
            }
            for transport in &prefill_decode_capability.supported_transports {
                hasher.update(b"|prefill_decode_transport|");
                hasher.update(transport.as_str().as_bytes());
            }
            hasher.update(b"|");
            hasher.update(if prefill_decode_capability.exposes_split_metrics {
                b"split_metrics".as_slice()
            } else {
                b"no_split_metrics".as_slice()
            });
            if let Some(detail) = &prefill_decode_capability.detail {
                hasher.update(b"|detail|");
                hasher.update(detail.as_bytes());
            }
        }
        hex::encode(hasher.finalize())
    }
}

impl Default for ExecutionCapabilityProfile {
    fn default() -> Self {
        Self::single_request_latency_optimized()
    }
}

/// Shared scheduler policy for continuous batched text generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationSchedulerPolicy {
    /// Maximum concurrently active requests the scheduler may keep live.
    pub max_active_requests: usize,
    /// Maximum queued requests admitted behind the active set.
    pub max_queued_requests: usize,
    /// Total prompt-prefill tokens the scheduler may execute in one scheduling tick.
    pub max_prefill_tokens_per_tick: usize,
    /// Total decode tokens the scheduler may execute in one scheduling tick.
    pub max_decode_tokens_per_tick: usize,
}

impl GenerationSchedulerPolicy {
    /// Default policy for the first continuous-batching scheduler.
    #[must_use]
    pub const fn continuous_batch_default() -> Self {
        Self {
            max_active_requests: 4,
            max_queued_requests: 32,
            max_prefill_tokens_per_tick: 4,
            max_decode_tokens_per_tick: 8,
        }
    }

    /// Total number of requests that may be admitted into one scheduler cycle.
    #[must_use]
    pub const fn total_request_capacity(&self) -> usize {
        self.max_active_requests + self.max_queued_requests
    }
}

impl Default for GenerationSchedulerPolicy {
    fn default() -> Self {
        Self::continuous_batch_default()
    }
}

/// High-level scheduling class observed for one realized generation path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationSchedulingClass {
    /// The request spent its realized runtime in prompt-prefill work only.
    Prefill,
    /// The request spent its realized runtime in decode work only.
    Decode,
    /// The request was interleaved across both prefill and decode classes.
    MixedPrefillDecode,
    /// The request fell back to one-request-at-a-time execution.
    FallbackSingleRequest,
}

/// Stable reason the scheduler fell back instead of running one request in the shared batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationSchedulerFallbackReason {
    /// The runtime path does not currently expose the shared scheduler.
    UnsupportedRuntime,
    /// The shared queue hit its explicit admitted capacity.
    QueueCapacityExceeded,
    /// The request had to serialize around conflicting request-owned session state.
    SessionSerialization,
}

impl GenerationSchedulerFallbackReason {
    /// Returns the canonical refusal when one scheduler fallback is really an
    /// explicit unsupported or serialization boundary.
    #[must_use]
    pub fn refusal(self) -> Option<PsionicRefusal> {
        match self {
            Self::UnsupportedRuntime => Some(PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedBackendCapability,
                PsionicRefusalScope::Runtime,
                "shared scheduler is unsupported on the selected runtime",
            )),
            Self::SessionSerialization => Some(PsionicRefusal::new(
                PsionicRefusalCode::SerializationIncompatibility,
                PsionicRefusalScope::Runtime,
                "request session state must serialize before this scheduler path can proceed",
            )),
            Self::QueueCapacityExceeded => None,
        }
    }
}

/// Aggregate count for one observed scheduler fallback reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationSchedulerFallbackCount {
    /// Stable fallback reason.
    pub reason: GenerationSchedulerFallbackReason,
    /// Number of requests that observed that reason.
    pub count: usize,
}

/// Aggregate metrics for one realized continuous-batching run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationSchedulerMetrics {
    /// Policy that governed the run.
    pub policy: GenerationSchedulerPolicy,
    /// Number of scheduler ticks executed.
    pub total_cycles: usize,
    /// Number of requests admitted into the scheduler.
    pub total_admitted_requests: usize,
    /// Number of requests completed by the scheduler.
    pub total_completed_requests: usize,
    /// Maximum queued depth observed behind the active set.
    pub max_queue_depth: usize,
    /// Maximum concurrently active requests observed in one tick.
    pub max_batch_size: usize,
    /// Total prompt-prefill tokens executed across all requests.
    pub total_prefill_tokens: usize,
    /// Total decode tokens executed across all requests.
    pub total_decode_tokens: usize,
    /// Aggregate TTFT observed across requests that emitted at least one token.
    pub total_time_to_first_token_ns: u64,
    /// Number of requests contributing TTFT observations.
    pub measured_time_to_first_token_requests: usize,
    /// Aggregate average ITL observed across requests that emitted multiple tokens.
    pub total_inter_token_latency_ns: u64,
    /// Number of requests contributing ITL observations.
    pub measured_inter_token_latency_requests: usize,
    /// Total KV pages allocated while requests were live on the scheduler.
    pub total_kv_pages_allocated: usize,
    /// Total KV pages reclaimed while requests were live on the scheduler.
    pub total_kv_pages_reclaimed: usize,
    /// Total KV bytes allocated while requests were live on the scheduler.
    pub total_kv_bytes_allocated: u64,
    /// Total KV bytes reclaimed while requests were live on the scheduler.
    pub total_kv_bytes_reclaimed: u64,
    /// Peak KV page footprint observed across the active scheduler set.
    pub peak_kv_pages_in_use: usize,
    /// Peak KV byte footprint observed across the active scheduler set.
    pub peak_kv_bytes_in_use: u64,
    /// Last realized scheduling class, when one tick made progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_scheduling_class: Option<GenerationSchedulingClass>,
    /// Stable fallback counts observed during the run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallback_counts: Vec<GenerationSchedulerFallbackCount>,
}

impl GenerationSchedulerMetrics {
    /// Creates an empty metric set for the supplied policy.
    #[must_use]
    pub fn for_policy(policy: GenerationSchedulerPolicy) -> Self {
        Self {
            policy,
            total_cycles: 0,
            total_admitted_requests: 0,
            total_completed_requests: 0,
            max_queue_depth: 0,
            max_batch_size: 0,
            total_prefill_tokens: 0,
            total_decode_tokens: 0,
            total_time_to_first_token_ns: 0,
            measured_time_to_first_token_requests: 0,
            total_inter_token_latency_ns: 0,
            measured_inter_token_latency_requests: 0,
            total_kv_pages_allocated: 0,
            total_kv_pages_reclaimed: 0,
            total_kv_bytes_allocated: 0,
            total_kv_bytes_reclaimed: 0,
            peak_kv_pages_in_use: 0,
            peak_kv_bytes_in_use: 0,
            last_scheduling_class: None,
            fallback_counts: Vec::new(),
        }
    }

    /// Records one fallback reason.
    pub fn record_fallback(&mut self, reason: GenerationSchedulerFallbackReason) {
        if let Some(entry) = self
            .fallback_counts
            .iter_mut()
            .find(|entry| entry.reason == reason)
        {
            entry.count = entry.count.saturating_add(1);
            return;
        }
        self.fallback_counts
            .push(GenerationSchedulerFallbackCount { reason, count: 1 });
        self.fallback_counts.sort_by_key(|entry| entry.reason);
    }
}

impl Default for GenerationSchedulerMetrics {
    fn default() -> Self {
        Self::for_policy(GenerationSchedulerPolicy::default())
    }
}

/// Per-request scheduling receipt attached to one realized generation response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationSchedulerRequestReceipt {
    /// Shared scheduler policy that governed the request.
    pub policy: GenerationSchedulerPolicy,
    /// Realized batch posture for the request path.
    pub batch_posture: BatchExecutionPosture,
    /// Queue depth observed when the request was admitted.
    pub queue_depth_at_admission: usize,
    /// Maximum concurrently active requests observed while this request was live.
    pub max_batch_size_observed: usize,
    /// High-level realized scheduling class for the request.
    pub scheduling_class: GenerationSchedulingClass,
    /// Prompt-prefill tokens executed for the request.
    pub prefill_tokens: usize,
    /// Decode tokens executed for the request.
    pub decode_tokens: usize,
    /// Truthful prefill/decode execution mode for the request path, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefill_decode_mode: Option<PrefillDecodeExecutionMode>,
    /// Explicit prefill/decode handoff seam for the request path, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefill_decode_handoff: Option<PrefillDecodeHandoff>,
    /// Time to first emitted token in nanoseconds, when the request emitted one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_to_first_token_ns: Option<u64>,
    /// Average inter-token latency in nanoseconds, when the request emitted multiple tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inter_token_latency_ns: Option<u64>,
    /// Explicit fallback reason when the request did not stay on the shared scheduler.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<GenerationSchedulerFallbackReason>,
}

/// Explicit isolation boundary for bounded sandbox execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxIsolationBoundary {
    /// Work runs inside the current host process boundary.
    Process,
    /// Work runs inside a container-style process boundary.
    Container,
    /// Work runs inside a VM-style boundary.
    VirtualMachine,
}

/// Root filesystem posture for bounded sandbox execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxFilesystemRoot {
    /// The root filesystem is mounted read-only.
    ReadOnly,
    /// The root filesystem is ephemeral and writable for the job lifetime only.
    EphemeralWritable,
}

/// Explicit filesystem policy for bounded sandbox execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxFilesystemPolicy {
    /// Root filesystem posture for the sandbox.
    pub root: SandboxFilesystemRoot,
    /// Explicit writable mounts exposed to the job.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writable_mounts: Vec<String>,
    /// Maximum bytes the sandbox may write across writable mounts.
    pub max_write_bytes: u64,
}

/// Network posture for bounded sandbox execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxNetworkMode {
    /// No outbound network access is permitted.
    Disabled,
    /// Only explicit destinations are permitted.
    RestrictedEgress,
}

/// Explicit network policy for bounded sandbox execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxNetworkPolicy {
    /// High-level network mode for the sandbox.
    pub mode: SandboxNetworkMode,
    /// Whether loopback access remains available inside the sandbox.
    pub allow_loopback: bool,
    /// Explicit allowed egress destinations when the sandbox permits restricted egress.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_hosts: Vec<String>,
}

/// Explicit process-spawning boundary for bounded sandbox execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxProcessPolicy {
    /// Maximum number of live processes allowed for the job.
    pub max_processes: u32,
    /// Maximum threads allowed within one process.
    pub max_threads_per_process: u32,
    /// Whether privilege escalation inside the sandbox is permitted.
    pub allow_privilege_escalation: bool,
}

/// Explicit resource limits for bounded sandbox execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxResourceLimits {
    /// Maximum wall-clock execution time in milliseconds.
    pub max_wall_time_ms: u64,
    /// Maximum CPU time in milliseconds across the sandbox.
    pub max_cpu_time_ms: u64,
    /// Maximum resident memory for the sandbox.
    pub max_memory_bytes: u64,
    /// Maximum stdout bytes retained in evidence.
    pub max_stdout_bytes: u64,
    /// Maximum stderr bytes retained in evidence.
    pub max_stderr_bytes: u64,
}

/// Accelerator exposure policy for bounded sandbox execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum SandboxAcceleratorAccess {
    /// No accelerator devices are exposed to the sandbox.
    Disabled,
    /// Accelerator devices are exposed under an explicit backend and device bound.
    Allowed {
        /// Backend family exposed to the sandbox.
        runtime_backend: String,
        /// Maximum number of visible devices.
        max_visible_devices: usize,
        /// Allowed performance classes for exposed devices.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        allowed_performance_classes: Vec<DevicePerformanceClass>,
        /// Whether exposed devices must surface stable topology keys.
        require_topology_keys: bool,
    },
}

impl SandboxAcceleratorAccess {
    /// Returns an explicit no-accelerator policy.
    #[must_use]
    pub const fn disabled() -> Self {
        Self::Disabled
    }

    /// Returns an explicit bounded accelerator policy.
    #[must_use]
    pub fn allowed(runtime_backend: impl Into<String>, max_visible_devices: usize) -> Self {
        Self::Allowed {
            runtime_backend: runtime_backend.into(),
            max_visible_devices,
            allowed_performance_classes: vec![
                DevicePerformanceClass::IntegratedAccelerator,
                DevicePerformanceClass::DiscreteAccelerator,
                DevicePerformanceClass::PartitionedAccelerator,
            ],
            require_topology_keys: true,
        }
    }

    /// Returns whether the sandbox expects accelerator visibility at all.
    #[must_use]
    pub const fn requires_accelerator(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }
}

/// Reusable execution profile for bounded sandbox execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionCapabilityProfile {
    /// Dispatch posture for the sandbox job lane.
    pub dispatch_profile: ExecutionCapabilityProfile,
    /// Isolation boundary for the job.
    pub isolation_boundary: SandboxIsolationBoundary,
    /// Filesystem policy for the sandbox.
    pub filesystem: SandboxFilesystemPolicy,
    /// Network policy for the sandbox.
    pub network: SandboxNetworkPolicy,
    /// Process-spawning policy for the sandbox.
    pub process: SandboxProcessPolicy,
    /// Explicit resource limits for the sandbox.
    pub resource_limits: SandboxResourceLimits,
    /// Accelerator exposure policy for the sandbox.
    pub accelerator_access: SandboxAcceleratorAccess,
}

impl SandboxExecutionCapabilityProfile {
    /// Returns a bounded CPU-only sandbox profile.
    #[must_use]
    pub fn bounded_cpu() -> Self {
        Self {
            dispatch_profile: ExecutionCapabilityProfile::single_request_latency_optimized(),
            isolation_boundary: SandboxIsolationBoundary::Container,
            filesystem: SandboxFilesystemPolicy {
                root: SandboxFilesystemRoot::ReadOnly,
                writable_mounts: vec![String::from("/tmp")],
                max_write_bytes: 64 * 1024 * 1024,
            },
            network: SandboxNetworkPolicy {
                mode: SandboxNetworkMode::Disabled,
                allow_loopback: false,
                allowed_hosts: Vec::new(),
            },
            process: SandboxProcessPolicy {
                max_processes: 32,
                max_threads_per_process: 8,
                allow_privilege_escalation: false,
            },
            resource_limits: SandboxResourceLimits {
                max_wall_time_ms: 300_000,
                max_cpu_time_ms: 300_000,
                max_memory_bytes: 2 * 1024 * 1024 * 1024,
                max_stdout_bytes: 1024 * 1024,
                max_stderr_bytes: 1024 * 1024,
            },
            accelerator_access: SandboxAcceleratorAccess::disabled(),
        }
    }

    /// Returns a bounded accelerator-visible sandbox profile.
    #[must_use]
    pub fn bounded_accelerated(
        runtime_backend: impl Into<String>,
        max_visible_devices: usize,
    ) -> Self {
        Self {
            accelerator_access: SandboxAcceleratorAccess::allowed(
                runtime_backend,
                max_visible_devices,
            ),
            ..Self::bounded_cpu()
        }
    }

    /// Returns a stable digest for the bounded sandbox profile.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(match self.dispatch_profile.batch_posture {
            BatchExecutionPosture::SingleRequestOnly => b"single_request_only".as_slice(),
            BatchExecutionPosture::CallerStaticBatch => b"caller_static_batch".as_slice(),
            BatchExecutionPosture::SchedulerStaticBatch => b"scheduler_static_batch".as_slice(),
            BatchExecutionPosture::ContinuousBatch => b"continuous_batch".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(match self.dispatch_profile.throughput_class {
            ThroughputClass::LatencyOptimized => b"latency_optimized".as_slice(),
            ThroughputClass::Balanced => b"balanced".as_slice(),
            ThroughputClass::ThroughputOptimized => b"throughput_optimized".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(
            self.dispatch_profile
                .queue_policy
                .max_active_requests
                .to_string(),
        );
        hasher.update(b"|");
        hasher.update(
            self.dispatch_profile
                .queue_policy
                .max_queued_requests
                .to_string(),
        );
        hasher.update(b"|");
        hasher.update(
            if self.dispatch_profile.queue_policy.per_model_serialization {
                b"1".as_slice()
            } else {
                b"0".as_slice()
            },
        );
        hasher.update(b"|");
        hasher.update(match self.isolation_boundary {
            SandboxIsolationBoundary::Process => b"process".as_slice(),
            SandboxIsolationBoundary::Container => b"container".as_slice(),
            SandboxIsolationBoundary::VirtualMachine => b"virtual_machine".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(match self.filesystem.root {
            SandboxFilesystemRoot::ReadOnly => b"read_only".as_slice(),
            SandboxFilesystemRoot::EphemeralWritable => b"ephemeral_writable".as_slice(),
        });
        hasher.update(b"|");
        for mount in &self.filesystem.writable_mounts {
            hasher.update(mount.as_bytes());
            hasher.update(b"\x1f");
        }
        hasher.update(b"|");
        hasher.update(self.filesystem.max_write_bytes.to_string());
        hasher.update(b"|");
        hasher.update(match self.network.mode {
            SandboxNetworkMode::Disabled => b"disabled".as_slice(),
            SandboxNetworkMode::RestrictedEgress => b"restricted_egress".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(if self.network.allow_loopback {
            b"1".as_slice()
        } else {
            b"0".as_slice()
        });
        hasher.update(b"|");
        for host in &self.network.allowed_hosts {
            hasher.update(host.as_bytes());
            hasher.update(b"\x1f");
        }
        hasher.update(b"|");
        hasher.update(self.process.max_processes.to_string());
        hasher.update(b"|");
        hasher.update(self.process.max_threads_per_process.to_string());
        hasher.update(b"|");
        hasher.update(if self.process.allow_privilege_escalation {
            b"1".as_slice()
        } else {
            b"0".as_slice()
        });
        hasher.update(b"|");
        hasher.update(self.resource_limits.max_wall_time_ms.to_string());
        hasher.update(b"|");
        hasher.update(self.resource_limits.max_cpu_time_ms.to_string());
        hasher.update(b"|");
        hasher.update(self.resource_limits.max_memory_bytes.to_string());
        hasher.update(b"|");
        hasher.update(self.resource_limits.max_stdout_bytes.to_string());
        hasher.update(b"|");
        hasher.update(self.resource_limits.max_stderr_bytes.to_string());
        hasher.update(b"|");
        match &self.accelerator_access {
            SandboxAcceleratorAccess::Disabled => hasher.update(b"disabled"),
            SandboxAcceleratorAccess::Allowed {
                runtime_backend,
                max_visible_devices,
                allowed_performance_classes,
                require_topology_keys,
            } => {
                hasher.update(b"allowed|");
                hasher.update(runtime_backend.as_bytes());
                hasher.update(b"|");
                hasher.update(max_visible_devices.to_string());
                hasher.update(b"|");
                for class in allowed_performance_classes {
                    hasher.update(match class {
                        DevicePerformanceClass::Reference => b"reference".as_slice(),
                        DevicePerformanceClass::IntegratedAccelerator => {
                            b"integrated_accelerator".as_slice()
                        }
                        DevicePerformanceClass::DiscreteAccelerator => {
                            b"discrete_accelerator".as_slice()
                        }
                        DevicePerformanceClass::PartitionedAccelerator => {
                            b"partitioned_accelerator".as_slice()
                        }
                    });
                    hasher.update(b"\x1f");
                }
                hasher.update(b"|");
                hasher.update(if *require_topology_keys {
                    b"1".as_slice()
                } else {
                    b"0".as_slice()
                });
            }
        }
        format!("{:x}", hasher.finalize())
    }
}

/// Stable identity inputs for one sandbox-execution request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionRequestIdentity {
    /// Stable request identifier.
    pub request_id: String,
    /// Stable digest of the bounded sandbox profile used for the request.
    pub sandbox_profile_digest: String,
    /// Stable digest of the command or job spec executed inside the sandbox.
    pub command_digest: String,
    /// Stable digest of the execution environment exposed to the job.
    pub environment_digest: String,
    /// Stable digests for declared input artifacts when the request consumes them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_artifact_digests: Vec<String>,
}

/// Explicit terminal reason for one sandbox-execution request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxExecutionExitKind {
    /// The sandbox job completed successfully.
    Succeeded,
    /// The sandbox job exited with a non-zero exit code.
    NonZeroExit,
    /// The sandbox job exceeded its explicit time budget.
    TimedOut,
    /// The sandbox job was cancelled by the caller.
    Cancelled,
    /// The runtime killed the sandbox job for safety or resource reasons.
    Killed,
    /// The runtime refused the sandbox job before launch under explicit policy.
    RefusedByPolicy,
}

/// Explicit terminal exit facts for one sandbox-execution request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionExit {
    /// Stable terminal reason.
    pub kind: SandboxExecutionExitKind,
    /// Concrete process exit code when one existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Short machine-readable explanation for the realized terminal state.
    pub detail: String,
}

/// Explicit resource summary for one sandbox-execution request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionResourceSummary {
    /// Realized wall-clock runtime in milliseconds.
    pub wall_time_ms: u64,
    /// Realized CPU time in milliseconds.
    pub cpu_time_ms: u64,
    /// Peak resident memory observed for the sandbox.
    pub peak_memory_bytes: u64,
    /// Total bytes written through writable filesystem mounts.
    pub filesystem_write_bytes: u64,
    /// Total stdout bytes produced by the sandbox.
    pub stdout_bytes: u64,
    /// Total stderr bytes produced by the sandbox.
    pub stderr_bytes: u64,
    /// Total network egress bytes observed for the sandbox.
    pub network_egress_bytes: u64,
}

/// Machine-checkable runtime evidence for one sandbox-execution request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionEvidence {
    /// Stable digest for the request identity.
    pub request_digest: String,
    /// Stable digest of the bounded sandbox profile used for the request.
    pub sandbox_profile_digest: String,
    /// Stable digest of the executed command or job spec.
    pub command_digest: String,
    /// Stable digest of the execution environment exposed to the job.
    pub environment_digest: String,
    /// Stable input artifact digests declared for the request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_artifact_digests: Vec<String>,
    /// Stable output artifact digests emitted by the request when applicable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_artifact_digests: Vec<String>,
    /// Explicit terminal status for the sandbox job.
    pub exit: SandboxExecutionExit,
    /// Explicit resource summary for the sandbox job.
    pub resources: SandboxExecutionResourceSummary,
    /// Stable digest of stdout bytes when retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_sha256: Option<String>,
    /// Stable digest of stderr bytes when retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_sha256: Option<String>,
    /// Delivery-proof facts surfaced by the underlying execution runtime when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_proof: Option<ExecutionDeliveryProof>,
}

/// Explicit local-runtime observability snapshot for app cutover and debugging.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRuntimeObservability {
    /// Explicit runtime crash/reset isolation policy.
    pub isolation_policy: LocalServingIsolationPolicy,
    /// Explicit invalidation policy for reusable cache and persisted-state families.
    pub cache_invalidation_policy: CacheInvalidationPolicy,
    /// Explicit batch, queueing, and throughput profile for the served path.
    pub execution_profile: ExecutionCapabilityProfile,
    /// Number of requests currently queued behind active execution.
    pub queue_depth: usize,
    /// Maximum queued requests admitted by policy, when bounded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_capacity: Option<usize>,
    /// Number of active generation sessions currently tracked by the runtime.
    pub active_sessions: usize,
    /// Number of actively executing requests across loaded models.
    pub active_requests: usize,
    /// Aggregate resident-memory footprint for currently loaded models.
    pub memory_footprint: MemoryResidencySnapshot,
    /// Current observed health for the backends participating in the local runtime.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backend_health: Vec<BackendHealthObservation>,
    /// Recent runtime transitions in chronological order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_transitions: Vec<RuntimeTransitionEvent>,
}

/// Explicit reason admission was refused under the active residency policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionRefusalReason {
    /// The candidate would exceed the admitted loaded-model count.
    #[error("loaded model count exceeds the admitted limit")]
    MaxLoadedModels,
    /// The candidate would exceed the admitted host-memory budget.
    #[error("resident host-memory budget exceeded")]
    HostMemoryBudget,
    /// The candidate would exceed the admitted device-memory budget.
    #[error("resident device-memory budget exceeded")]
    DeviceMemoryBudget,
}

/// Refusal details for a candidate model under the active residency policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("admission refused for model `{requested_model_id}`: {reason}")]
pub struct ModelAdmissionRefusal {
    /// Stable candidate model identifier.
    pub requested_model_id: String,
    /// Explicit refusal reason.
    pub reason: AdmissionRefusalReason,
    /// Active residency policy at refusal time.
    pub policy: ModelResidencyPolicy,
    /// Current loaded-model memory state before the attempted admission.
    pub current: MemoryResidencySnapshot,
    /// Requested plan for the candidate model.
    pub requested_plan: ModelMemoryPlan,
    /// Remaining loaded models that blocked the candidate after any evictions.
    pub blocking_models: Vec<String>,
}

/// Admission decision for a candidate model under the active residency policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelAdmissionDecision {
    /// Loaded-model residency before the candidate was considered.
    pub current: MemoryResidencySnapshot,
    /// Loaded-model residency after the candidate is admitted.
    pub admitted: MemoryResidencySnapshot,
    /// Idle models that must be evicted before admitting the candidate.
    pub evicted_models: Vec<String>,
}

/// Plans local-serving admission for one candidate model.
pub fn plan_model_admission(
    loaded: &[LoadedModelMemoryState],
    requested_model_id: &str,
    requested_plan: &ModelMemoryPlan,
    policy: &ModelResidencyPolicy,
) -> Result<ModelAdmissionDecision, ModelAdmissionRefusal> {
    let current = MemoryResidencySnapshot::from_loaded_models(loaded);
    let mut remaining = loaded
        .iter()
        .filter(|model| model.model_id != requested_model_id)
        .cloned()
        .collect::<Vec<_>>();
    let mut evicted_models = Vec::new();
    let mut admitted = snapshot_with_candidate(&remaining, requested_plan);
    if policy_admits_snapshot(policy, &admitted) {
        return Ok(ModelAdmissionDecision {
            current,
            admitted,
            evicted_models,
        });
    }

    if policy.pressure_action == ResidencyPressureAction::UnloadIdleOldestFirst {
        remaining.sort_by_key(|model| {
            (
                usize::from(model.active_requests > 0),
                model.last_used_at_millis,
                model.model_id.clone(),
            )
        });
        while !policy_admits_snapshot(policy, &admitted) {
            let Some(index) = remaining
                .iter()
                .position(|model| model.active_requests == 0)
            else {
                break;
            };
            let evicted = remaining.remove(index);
            evicted_models.push(evicted.model_id);
            admitted = snapshot_with_candidate(&remaining, requested_plan);
        }
        if policy_admits_snapshot(policy, &admitted) {
            return Ok(ModelAdmissionDecision {
                current,
                admitted,
                evicted_models,
            });
        }
    }

    Err(ModelAdmissionRefusal {
        requested_model_id: requested_model_id.to_string(),
        reason: first_policy_violation(policy, &admitted),
        policy: policy.clone(),
        current,
        requested_plan: requested_plan.clone(),
        blocking_models: remaining.into_iter().map(|model| model.model_id).collect(),
    })
}

fn snapshot_with_candidate(
    loaded: &[LoadedModelMemoryState],
    requested_plan: &ModelMemoryPlan,
) -> MemoryResidencySnapshot {
    let mut snapshot = MemoryResidencySnapshot::from_loaded_models(loaded);
    snapshot.loaded_models += 1;
    snapshot.resident_host_bytes = snapshot
        .resident_host_bytes
        .saturating_add(requested_plan.resident_host_bytes);
    snapshot.resident_device_bytes = snapshot
        .resident_device_bytes
        .saturating_add(requested_plan.resident_device_bytes);
    snapshot
}

fn policy_admits_snapshot(
    policy: &ModelResidencyPolicy,
    snapshot: &MemoryResidencySnapshot,
) -> bool {
    if let Some(max_loaded_models) = policy.max_loaded_models {
        if snapshot.loaded_models > max_loaded_models {
            return false;
        }
    }
    if let Some(limit) = policy.memory_budget.resident_host_bytes {
        if snapshot.resident_host_bytes > limit {
            return false;
        }
    }
    if let Some(limit) = policy.memory_budget.resident_device_bytes {
        if snapshot.resident_device_bytes > limit {
            return false;
        }
    }
    true
}

fn first_policy_violation(
    policy: &ModelResidencyPolicy,
    snapshot: &MemoryResidencySnapshot,
) -> AdmissionRefusalReason {
    if let Some(max_loaded_models) = policy.max_loaded_models {
        if snapshot.loaded_models > max_loaded_models {
            return AdmissionRefusalReason::MaxLoadedModels;
        }
    }
    if let Some(limit) = policy.memory_budget.resident_host_bytes {
        if snapshot.resident_host_bytes > limit {
            return AdmissionRefusalReason::HostMemoryBudget;
        }
    }
    if let Some(limit) = policy.memory_budget.resident_device_bytes {
        if snapshot.resident_device_bytes > limit {
            return AdmissionRefusalReason::DeviceMemoryBudget;
        }
    }
    AdmissionRefusalReason::MaxLoadedModels
}

/// Whether KV pages stay bound to one backend/device posture or can move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvCacheDeviceScope {
    /// KV state stays bound to the active backend/device and is not migrated.
    SameDeviceOnly,
    /// KV state may move across devices through an explicit transfer path.
    CrossDeviceExplicit,
}

/// Policy to apply when paged KV growth would exceed the admitted budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvCacheSpillPolicy {
    /// Refuse additional KV growth instead of evicting or spilling silently.
    RefuseNewPages,
    /// Evict older pages to admit new ones.
    EvictOldestPages,
    /// Spill pages to a slower/offloaded tier.
    SpillToHost,
}

/// Stable logical page layout for paged KV state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCachePageLayout {
    /// Maximum supported context tokens for the cache.
    pub max_context_tokens: usize,
    /// Number of tokens stored in one logical page.
    pub tokens_per_page: usize,
    /// Number of bytes consumed per cached token.
    pub bytes_per_token: usize,
    /// Number of bytes consumed by one full logical page.
    pub page_bytes: usize,
    /// Maximum number of pages the cache may own.
    pub max_pages: usize,
}

impl KvCachePageLayout {
    /// Creates a logical page layout from token and byte geometry.
    #[must_use]
    pub fn new(max_context_tokens: usize, tokens_per_page: usize, bytes_per_token: usize) -> Self {
        let max_context_tokens = max_context_tokens.max(1);
        let tokens_per_page = tokens_per_page.max(1);
        let bytes_per_token = bytes_per_token.max(1);
        let max_pages = max_context_tokens.div_ceil(tokens_per_page);
        Self {
            max_context_tokens,
            tokens_per_page,
            bytes_per_token,
            page_bytes: tokens_per_page.saturating_mul(bytes_per_token),
            max_pages,
        }
    }

    /// Returns the number of pages required for the provided token count.
    #[must_use]
    pub fn page_count_for_tokens(&self, tokens: usize) -> usize {
        if tokens == 0 {
            0
        } else {
            tokens.div_ceil(self.tokens_per_page)
        }
    }

    /// Returns the number of bytes required for the provided token count.
    #[must_use]
    pub fn bytes_for_tokens(&self, tokens: usize) -> u64 {
        tokens
            .saturating_mul(self.bytes_per_token)
            .try_into()
            .unwrap_or(u64::MAX)
    }
}

/// Explicit paged-KV policy exposed through runtime and evidence surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCachePolicy {
    /// Whether KV state stays on one backend/device or can move explicitly.
    pub device_scope: KvCacheDeviceScope,
    /// What to do when the page budget would be exceeded.
    pub spill_policy: KvCacheSpillPolicy,
    /// Logical page layout for the cache.
    pub page_layout: KvCachePageLayout,
}

/// Snapshot of current paged-KV usage.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCacheState {
    /// Number of tokens currently cached.
    pub tokens: usize,
    /// Number of bytes currently owned by the cache.
    pub bytes: u64,
    /// Number of pages currently owned by the cache.
    pub pages: usize,
}

impl KvCacheState {
    /// Builds paged-KV state from a logical layout and token count.
    #[must_use]
    pub fn paged(layout: &KvCachePageLayout, tokens: usize) -> Self {
        Self {
            tokens,
            bytes: layout.bytes_for_tokens(tokens),
            pages: layout.page_count_for_tokens(tokens),
        }
    }
}

/// Explicit residency tier for paged-KV state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvResidencyTier {
    /// Active device-resident KV pages.
    Device,
    /// Host-resident KV pages or mirror state.
    Host,
    /// Distributed or externalized KV state.
    Distributed,
}

impl KvResidencyTier {
    /// Returns a stable machine-checkable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Device => "device",
            Self::Host => "host",
            Self::Distributed => "distributed",
        }
    }
}

/// Externalized locator kind for non-local KV tiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvResidencyLocatorKind {
    /// A Psionic datastream manifest/reference backs the distributed tier.
    Datastream,
}

impl KvResidencyLocatorKind {
    /// Returns a stable machine-checkable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Datastream => "datastream",
        }
    }
}

/// Stable external locator for an offloaded or distributed KV tier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvResidencyExternalLocator {
    /// Locator kind used for the externalized tier.
    pub kind: KvResidencyLocatorKind,
    /// Stable locator identifier such as a datastream ID.
    pub locator_id: String,
    /// Stable digest for the locator contract.
    pub locator_digest: String,
    /// Stable object digest referenced by the locator.
    pub object_digest: String,
    /// Total bytes published behind the locator.
    pub total_bytes: u64,
    /// Optional plain-language detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl KvResidencyExternalLocator {
    /// Creates an external locator for one datastream-backed KV tier.
    #[must_use]
    pub fn datastream(
        locator_id: impl Into<String>,
        locator_digest: impl Into<String>,
        object_digest: impl Into<String>,
        total_bytes: u64,
    ) -> Self {
        Self {
            kind: KvResidencyLocatorKind::Datastream,
            locator_id: locator_id.into(),
            locator_digest: locator_digest.into(),
            object_digest: object_digest.into(),
            total_bytes,
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

/// One current residency tier snapshot for paged-KV state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvResidencyTierState {
    /// Tier being described.
    pub tier: KvResidencyTier,
    /// KV state visible in the tier.
    pub state: KvCacheState,
    /// Whether the tier is actively resident in the current runtime path.
    pub resident: bool,
    /// Optional external locator when this tier is offloaded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_locator: Option<KvResidencyExternalLocator>,
    /// Optional plain-language detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl KvResidencyTierState {
    /// Creates a resident tier snapshot.
    #[must_use]
    pub fn resident(tier: KvResidencyTier, state: KvCacheState) -> Self {
        Self {
            tier,
            state,
            resident: true,
            external_locator: None,
            detail: None,
        }
    }

    /// Creates an externalized tier snapshot.
    #[must_use]
    pub fn external(
        tier: KvResidencyTier,
        state: KvCacheState,
        external_locator: KvResidencyExternalLocator,
    ) -> Self {
        Self {
            tier,
            state,
            resident: false,
            external_locator: Some(external_locator),
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

/// Stable movement kind between KV residency tiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvResidencyMovementKind {
    /// Pages or bytes were prefetched into a faster tier before execution.
    Prefetch,
    /// Pages or bytes were written back into a slower tier after execution.
    WriteBack,
    /// Pages or bytes were spilled into a slower tier under capacity pressure.
    Spill,
    /// Pages or bytes were restored from a slower tier into an active tier.
    Restore,
}

impl KvResidencyMovementKind {
    /// Returns a stable machine-checkable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prefetch => "prefetch",
            Self::WriteBack => "write_back",
            Self::Spill => "spill",
            Self::Restore => "restore",
        }
    }
}

/// One explicit tier movement observed for paged-KV state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvResidencyMovement {
    /// Movement kind observed for the request path.
    pub kind: KvResidencyMovementKind,
    /// Tier that produced the pages or bytes.
    pub from_tier: KvResidencyTier,
    /// Tier that received the pages or bytes.
    pub to_tier: KvResidencyTier,
    /// Number of moved pages.
    pub kv_pages: usize,
    /// Number of moved bytes.
    pub kv_bytes: u64,
    /// Movement latency in nanoseconds, when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ns: Option<u64>,
    /// Optional plain-language detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl KvResidencyMovement {
    /// Creates a tier movement.
    #[must_use]
    pub const fn new(
        kind: KvResidencyMovementKind,
        from_tier: KvResidencyTier,
        to_tier: KvResidencyTier,
        kv_pages: usize,
        kv_bytes: u64,
    ) -> Self {
        Self {
            kind,
            from_tier,
            to_tier,
            kv_pages,
            kv_bytes,
            latency_ns: None,
            detail: None,
        }
    }

    /// Attaches movement latency.
    #[must_use]
    pub const fn with_latency_ns(mut self, latency_ns: u64) -> Self {
        self.latency_ns = Some(latency_ns);
        self
    }

    /// Attaches plain-language detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Explicit refusal reason for unsupported or degraded KV residency behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvResidencyRefusalReason {
    /// The requested residency tier is not supported for the realized path.
    TierUnsupported,
    /// The requested spill behavior is not supported for the realized path.
    SpillUnsupported,
    /// The requested prefetch behavior is not supported for the realized path.
    PrefetchUnsupported,
    /// The requested write-back behavior is not supported for the realized path.
    WriteBackUnsupported,
    /// A distributed tier was requested without an external locator contract.
    ExternalLocatorRequired,
}

impl KvResidencyRefusalReason {
    /// Returns a stable machine-checkable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TierUnsupported => "tier_unsupported",
            Self::SpillUnsupported => "spill_unsupported",
            Self::PrefetchUnsupported => "prefetch_unsupported",
            Self::WriteBackUnsupported => "write_back_unsupported",
            Self::ExternalLocatorRequired => "external_locator_required",
        }
    }
}

/// Explicit refusal emitted for unsupported KV residency behavior.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvResidencyRefusal {
    /// Stable refusal reason.
    pub reason: KvResidencyRefusalReason,
    /// Plain-language refusal detail.
    pub detail: String,
}

impl KvResidencyRefusal {
    /// Creates one refusal.
    #[must_use]
    pub fn new(reason: KvResidencyRefusalReason, detail: impl Into<String>) -> Self {
        Self {
            reason,
            detail: detail.into(),
        }
    }
}

/// Explicit residency accounting for paged-KV tiers and movement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvResidencyAccounting {
    /// Device-scope contract used by the realized path.
    pub device_scope: KvCacheDeviceScope,
    /// Spill policy used by the realized path.
    pub spill_policy: KvCacheSpillPolicy,
    /// Visible residency tiers for the request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiers: Vec<KvResidencyTierState>,
    /// Explicit tier movements observed during the request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub movements: Vec<KvResidencyMovement>,
    /// Explicit refusals for unsupported tier behavior.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusals: Vec<KvResidencyRefusal>,
}

impl KvResidencyAccounting {
    /// Creates residency accounting from one realized cache policy.
    #[must_use]
    pub fn from_policy(policy: &KvCachePolicy) -> Self {
        Self {
            device_scope: policy.device_scope,
            spill_policy: policy.spill_policy,
            tiers: Vec::new(),
            movements: Vec::new(),
            refusals: Vec::new(),
        }
    }

    /// Appends one tier snapshot.
    #[must_use]
    pub fn with_tier(mut self, tier: KvResidencyTierState) -> Self {
        self.tiers.push(tier);
        self.tiers.sort_by_key(|tier| tier.tier);
        self
    }

    /// Appends one observed movement.
    #[must_use]
    pub fn with_movement(mut self, movement: KvResidencyMovement) -> Self {
        self.movements.push(movement);
        self
    }

    /// Appends one explicit refusal.
    #[must_use]
    pub fn with_refusal(mut self, refusal: KvResidencyRefusal) -> Self {
        self.refusals.push(refusal);
        self
    }

    /// Returns whether the accounting already exposes one residency tier.
    #[must_use]
    pub fn has_tier(&self, tier: KvResidencyTier) -> bool {
        self.tiers.iter().any(|entry| entry.tier == tier)
    }
}

/// Growth delta between two paged-KV states.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCacheGrowth {
    /// Net token growth between the baseline and current state.
    pub tokens: usize,
    /// Net byte growth between the baseline and current state.
    pub bytes: u64,
    /// Net page growth between the baseline and current state.
    pub pages: usize,
}

/// Stable owner class for one paged-KV residency binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvCacheOwnerClass {
    /// Pages are owned by a durable session cache.
    Session,
    /// Pages are owned by one realized request path.
    Request,
    /// Pages are owned by a shared prefix entry.
    SharedPrefix,
}

/// Stable scheduler-scoped residency binding for one request-owned KV cache.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCacheSchedulerBinding {
    /// Realized batching posture for the request path.
    pub batch_posture: BatchExecutionPosture,
    /// Queue depth observed when the request was admitted, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_depth_at_admission: Option<usize>,
}

/// Stable owner binding for one realized paged-KV cache.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCacheOwnerBinding {
    /// Owner class for the bound cache.
    pub owner_class: KvCacheOwnerClass,
    /// Stable owner identifier.
    pub owner_id: String,
    /// Model identity that owns the current cache image.
    pub model_id: String,
    /// Bound session identifier when the cache is session-scoped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Scheduler binding when the cache is request-owned under a shared queue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<KvCacheSchedulerBinding>,
}

impl KvCacheOwnerBinding {
    /// Creates one owner binding for a realized cache image.
    #[must_use]
    pub fn new(
        owner_class: KvCacheOwnerClass,
        owner_id: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Self {
        Self {
            owner_class,
            owner_id: owner_id.into(),
            model_id: model_id.into(),
            session_id: None,
            scheduler: None,
        }
    }

    /// Binds the cache owner to one durable session identifier.
    #[must_use]
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Binds the cache owner to one scheduler-scoped execution path.
    #[must_use]
    pub fn with_scheduler(mut self, scheduler: KvCacheSchedulerBinding) -> Self {
        self.scheduler = Some(scheduler);
        self
    }
}

/// One logical paged-KV span owned by a realized cache image.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCachePageSpan {
    /// Stable logical page identifier for this cache image.
    pub page_index: usize,
    /// First token position owned by the page.
    pub start_token_position: usize,
    /// Number of resident token slots stored in the page.
    pub token_count: usize,
    /// Number of resident bytes currently owned by the page.
    pub bytes_used: u64,
}

/// Request- or session-owned paged-KV accounting with explicit page movement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCacheOwnershipAccounting {
    /// Stable owner binding for the cache image.
    pub owner: KvCacheOwnerBinding,
    /// Baseline cache state at the start of the observed window.
    pub previous: KvCacheState,
    /// Current cache state at the end of the observed window.
    pub current: KvCacheState,
    /// Net growth between the baseline and current state.
    pub growth: KvCacheGrowth,
    /// Pages allocated during the observed window.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allocated_pages: Vec<KvCachePageSpan>,
    /// Pages reclaimed during the observed window.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reclaimed_pages: Vec<KvCachePageSpan>,
}

impl KvCacheOwnershipAccounting {
    /// Creates ownership accounting from one baseline, current state, and page-event set.
    #[must_use]
    pub fn new(
        owner: KvCacheOwnerBinding,
        previous: KvCacheState,
        current: KvCacheState,
        allocated_pages: Vec<KvCachePageSpan>,
        reclaimed_pages: Vec<KvCachePageSpan>,
    ) -> Self {
        Self {
            growth: KvCacheGrowth {
                tokens: current.tokens.saturating_sub(previous.tokens),
                bytes: current.bytes.saturating_sub(previous.bytes),
                pages: current.pages.saturating_sub(previous.pages),
            },
            owner,
            previous,
            current,
            allocated_pages,
            reclaimed_pages,
        }
    }
}

/// Current paged-KV state plus request-local growth accounting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCacheAccounting {
    /// Current paged-KV state after the request.
    pub current: KvCacheState,
    /// Growth attributable to the request.
    pub growth: KvCacheGrowth,
}

impl KvCacheAccounting {
    /// Creates accounting from a before/after paged-KV snapshot.
    #[must_use]
    pub fn from_states(before: &KvCacheState, current: KvCacheState) -> Self {
        Self {
            growth: KvCacheGrowth {
                tokens: current.tokens.saturating_sub(before.tokens),
                bytes: current.bytes.saturating_sub(before.bytes),
                pages: current.pages.saturating_sub(before.pages),
            },
            current,
        }
    }
}

/// Observable state for shared prompt-prefix reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixCacheState {
    /// No compatible shared prefix cache existed for the request.
    None,
    /// A compatible shared prefix cache was found and reused.
    Hit,
    /// Compatible shared prefix caches existed but none matched the request prefix.
    Miss,
    /// Reuse was intentionally skipped under the current policy.
    Bypassed,
    /// A stale or invalid shared prefix entry was discarded and rebuilt.
    Rebuilt,
}

/// Explicit reuse boundaries for shared prompt-prefix caches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixCacheReusePolicy {
    /// Whether prefixes may be reused across distinct sessions.
    pub shared_across_sessions: bool,
    /// Whether prefixes may be reused across distinct user/security domains.
    pub shared_across_users: bool,
    /// Whether prefixes may be reused across different models or revisions.
    pub shared_across_models: bool,
    /// Whether prefixes may be reused across different backend identities.
    pub shared_across_backends: bool,
    /// Whether prefixes may be reused across different sampler settings.
    pub shared_across_sampler_settings: bool,
}

/// Request-level control over automatic prefix caching.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixCacheMode {
    /// Allow automatic prefix reuse under the active policy.
    #[default]
    Auto,
    /// Opt out of shared prefix reuse and do not record a new shared entry.
    Bypass,
    /// Force invalidation before evaluating the request and rebuild fresh state.
    Invalidate,
}

/// Request-scoped automatic prefix-cache controls.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixCacheControl {
    /// Request-level mode for the shared prefix cache.
    pub mode: PrefixCacheMode,
    /// Explicit tenant or security-domain binding when higher layers have one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
}

impl PrefixCacheControl {
    /// Returns whether the control is the default automatic posture.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.mode == PrefixCacheMode::Auto && self.tenant_id.is_none()
    }
}

/// Explicit reason the runtime refused or bypassed one shared prefix reuse path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixCacheRefusalReason {
    /// The caller opted out of automatic prefix caching.
    RequestOptOut,
    /// The caller forced invalidation before the lookup.
    ForcedInvalidation,
    /// The request tenant/security boundary did not match the cached entry.
    TenantBoundary,
    /// The sampler boundary did not match the cached entry.
    SamplerBoundary,
    /// The request already had session-owned KV state and skipped shared reuse.
    SessionBoundState,
}

/// Stable backend/toolchain identity carried into artifact reproducibility claims.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendToolchainIdentity {
    /// Effective backend label such as `cpu`, `metal`, or `cuda`.
    pub effective_backend: String,
    /// Stable toolchain/version label for the active backend path.
    pub toolchain_version: String,
    /// Stable backend feature flags compiled or selected for the active path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compiled_backend_features: Vec<String>,
    /// Whether the toolchain truth is compile-only or also backed by a live host probe.
    pub probe_state: BackendProbeState,
    /// Stable backend/runtime feature flags observed from the host probe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probed_backend_features: Vec<String>,
}

/// Whether backend/toolchain truth is compile-only or backed by a live host probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendProbeState {
    /// Capability truth comes only from what was compiled into the binary.
    CompiledOnly,
    /// Capability truth is backed by a live host/backend probe.
    CompiledAndProbed,
}

impl BackendToolchainIdentity {
    /// Creates backend/toolchain identity from explicit labels.
    #[must_use]
    pub fn new(
        effective_backend: impl Into<String>,
        toolchain_version: impl Into<String>,
        compiled_backend_features: Vec<String>,
    ) -> Self {
        Self {
            effective_backend: effective_backend.into(),
            toolchain_version: toolchain_version.into(),
            compiled_backend_features,
            probe_state: BackendProbeState::CompiledOnly,
            probed_backend_features: Vec::new(),
        }
    }

    /// Attaches probe-backed runtime feature truth for the active backend path.
    #[must_use]
    pub fn with_probe(
        mut self,
        probe_state: BackendProbeState,
        mut probed_backend_features: Vec<String>,
    ) -> Self {
        probed_backend_features.sort();
        probed_backend_features.dedup();
        self.probe_state = probe_state;
        self.probed_backend_features = probed_backend_features;
        self
    }
}

/// Stable served-artifact identity tuple for one served model/backend path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServedArtifactIdentity {
    /// Stable model identifier.
    pub model_id: String,
    /// Stable model revision.
    pub model_revision: String,
    /// Stable weight-bundle digest for the loaded weights.
    pub weight_bundle_digest: String,
    /// Stable top-level served-artifact digest over all identity inputs below.
    pub served_artifact_digest: String,
    /// Primary model-blob digest when the model came from an external artifact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_blob_digest: Option<String>,
    /// Stable tokenizer digest when tokenization participates in serving behavior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer_digest: Option<String>,
    /// Stable chat-template digest when prompt rendering participates in serving behavior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template_digest: Option<String>,
    /// Stable digest over default generation behavior such as BOS/EOS and default stops.
    pub generation_defaults_digest: String,
    /// Stable weight-format label.
    pub weight_format: String,
    /// Stable quantization family for the served model.
    pub quantization_family: QuantizationMode,
    /// Backend/toolchain identity for the active served path.
    pub backend: BackendToolchainIdentity,
}

impl ServedArtifactIdentity {
    /// Creates a served-artifact identity and computes its stable digest.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model_id: impl Into<String>,
        model_revision: impl Into<String>,
        weight_bundle_digest: impl Into<String>,
        model_blob_digest: Option<String>,
        tokenizer_digest: Option<String>,
        chat_template_digest: Option<String>,
        generation_defaults_digest: impl Into<String>,
        weight_format: impl Into<String>,
        quantization_family: QuantizationMode,
        backend: BackendToolchainIdentity,
    ) -> Self {
        let model_id = model_id.into();
        let model_revision = model_revision.into();
        let weight_bundle_digest = weight_bundle_digest.into();
        let generation_defaults_digest = generation_defaults_digest.into();
        let weight_format = weight_format.into();
        let served_artifact_digest = digest_served_artifact_identity(
            model_id.as_str(),
            model_revision.as_str(),
            weight_bundle_digest.as_str(),
            model_blob_digest.as_deref(),
            tokenizer_digest.as_deref(),
            chat_template_digest.as_deref(),
            generation_defaults_digest.as_str(),
            weight_format.as_str(),
            quantization_family,
            &backend,
        );
        Self {
            model_id,
            model_revision,
            weight_bundle_digest,
            served_artifact_digest,
            model_blob_digest,
            tokenizer_digest,
            chat_template_digest,
            generation_defaults_digest,
            weight_format,
            quantization_family,
            backend,
        }
    }
}

/// Layout class for one pre-sharded model manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShardedModelLayoutKind {
    /// The same served state is replicated across multiple shard entries.
    Replicated,
    /// Layers are partitioned across multiple shard entries.
    LayerSharded,
    /// One tensor axis is partitioned across multiple shard entries.
    TensorSharded,
}

impl ShardedModelLayoutKind {
    /// Returns the matching execution-topology kind.
    #[must_use]
    pub const fn topology_kind(self) -> ExecutionTopologyKind {
        match self {
            Self::Replicated => ExecutionTopologyKind::Replicated,
            Self::LayerSharded => ExecutionTopologyKind::LayerSharded,
            Self::TensorSharded => ExecutionTopologyKind::TensorSharded,
        }
    }

    /// Returns the matching clustered lane.
    #[must_use]
    pub const fn cluster_lane(self) -> ClusterExecutionLane {
        match self {
            Self::Replicated => ClusterExecutionLane::ReplicaRouted,
            Self::LayerSharded => ClusterExecutionLane::LayerSharded,
            Self::TensorSharded => ClusterExecutionLane::TensorSharded,
        }
    }
}

/// One explicit shard artifact inside a pre-sharded model manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardedModelArtifactRef {
    /// Stable shard identifier.
    pub shard_id: usize,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable artifact digest for the shard bytes.
    pub artifact_digest: String,
    /// Logical partition owned by this shard artifact.
    pub partition: ExecutionPartition,
    /// Stable provenance digest for the shard bytes, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_digest: Option<String>,
}

impl ShardedModelArtifactRef {
    /// Creates one shard-artifact reference from stable IDs and partition truth.
    #[must_use]
    pub fn new(
        shard_id: usize,
        artifact_id: impl Into<String>,
        artifact_digest: impl Into<String>,
        partition: ExecutionPartition,
    ) -> Self {
        Self {
            shard_id,
            artifact_id: artifact_id.into(),
            artifact_digest: artifact_digest.into(),
            partition,
            provenance_digest: None,
        }
    }

    /// Attaches a shard provenance digest.
    #[must_use]
    pub fn with_provenance_digest(mut self, provenance_digest: impl Into<String>) -> Self {
        self.provenance_digest = Some(provenance_digest.into());
        self
    }
}

/// Pre-sharded model manifest that binds shard artifacts to a served artifact identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardedModelManifest {
    /// Stable manifest identifier.
    pub manifest_id: String,
    /// Stable served-artifact identity the shard set belongs to.
    pub served_artifact: ServedArtifactIdentity,
    /// Effective backend the shard set targets.
    pub effective_backend: String,
    /// Sharding layout represented by this manifest.
    pub layout: ShardedModelLayoutKind,
    /// Explicit shard artifact set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shards: Vec<ShardedModelArtifactRef>,
}

impl ShardedModelManifest {
    /// Creates an empty sharded-model manifest.
    #[must_use]
    pub fn new(
        manifest_id: impl Into<String>,
        served_artifact: ServedArtifactIdentity,
        layout: ShardedModelLayoutKind,
    ) -> Self {
        let effective_backend = served_artifact.backend.effective_backend.clone();
        Self {
            manifest_id: manifest_id.into(),
            served_artifact,
            effective_backend,
            layout,
            shards: Vec::new(),
        }
    }

    /// Overrides the effective backend label.
    #[must_use]
    pub fn with_effective_backend(mut self, effective_backend: impl Into<String>) -> Self {
        self.effective_backend = effective_backend.into();
        self
    }

    /// Appends one shard artifact reference.
    #[must_use]
    pub fn with_shard(mut self, shard: ShardedModelArtifactRef) -> Self {
        self.shards.push(shard);
        self
    }

    /// Returns the stable digest for this shard manifest.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.manifest_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.served_artifact.served_artifact_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(self.effective_backend.as_bytes());
        hasher.update(b"|");
        hasher.update(match self.layout {
            ShardedModelLayoutKind::Replicated => b"replicated".as_slice(),
            ShardedModelLayoutKind::LayerSharded => b"layer_sharded".as_slice(),
            ShardedModelLayoutKind::TensorSharded => b"tensor_sharded".as_slice(),
        });
        for shard in &self.shards {
            hasher.update(b"|shard|");
            hasher.update(shard.shard_id.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(shard.artifact_id.as_bytes());
            hasher.update(b"|");
            hasher.update(shard.artifact_digest.as_bytes());
            hasher.update(b"|");
            hasher.update(
                shard
                    .provenance_digest
                    .as_deref()
                    .unwrap_or_default()
                    .as_bytes(),
            );
            hasher.update(b"|");
            update_partition_digest(&mut hasher, &shard.partition);
        }
        hex::encode(hasher.finalize())
    }

    /// Returns the stable shard-artifact digests in shard order.
    #[must_use]
    pub fn shard_artifact_digests(&self) -> Vec<&str> {
        let mut shards = self.shards.iter().collect::<Vec<_>>();
        shards.sort_by_key(|shard| shard.shard_id);
        shards
            .into_iter()
            .map(|shard| shard.artifact_digest.as_str())
            .collect()
    }

    /// Validates the manifest structure without any topology assignment.
    pub fn validate(&self) -> Result<(), ShardedModelManifestError> {
        if self.shards.is_empty() {
            return Err(ShardedModelManifestError::EmptyManifest);
        }
        let mut seen = BTreeSet::new();
        for shard in &self.shards {
            if !seen.insert(shard.shard_id) {
                return Err(ShardedModelManifestError::DuplicateShardId {
                    shard_id: shard.shard_id,
                });
            }
            if !partition_matches_layout(&shard.partition, self.layout) {
                return Err(ShardedModelManifestError::PartitionLayoutMismatch {
                    shard_id: shard.shard_id,
                    layout: self.layout,
                });
            }
        }
        for expected_shard_id in 0..self.shards.len() {
            if !seen.contains(&expected_shard_id) {
                return Err(ShardedModelManifestError::MissingShardId {
                    shard_id: expected_shard_id,
                });
            }
        }
        Ok(())
    }

    /// Validates that the manifest matches one realized execution topology.
    pub fn validate_against_topology(
        &self,
        topology: &ExecutionTopologyPlan,
    ) -> Result<(), ShardedModelManifestError> {
        self.validate()?;
        if topology.kind != self.layout.topology_kind() {
            return Err(ShardedModelManifestError::TopologyKindMismatch {
                manifest: self.layout,
                topology: topology.kind,
            });
        }
        if topology.effective_backend != self.effective_backend {
            return Err(ShardedModelManifestError::TopologyBackendMismatch {
                manifest_backend: self.effective_backend.clone(),
                topology_backend: topology.effective_backend.clone(),
            });
        }
        if topology.assignments.len() != self.shards.len() {
            return Err(ShardedModelManifestError::TopologyShardCountMismatch {
                manifest_shards: self.shards.len(),
                topology_shards: topology.assignments.len(),
            });
        }
        let shard_map = self
            .shards
            .iter()
            .map(|shard| (shard.shard_id, shard))
            .collect::<BTreeMap<_, _>>();
        for assignment in &topology.assignments {
            let Some(shard) = shard_map.get(&assignment.shard_id) else {
                return Err(ShardedModelManifestError::TopologyShardMissing {
                    shard_id: assignment.shard_id,
                });
            };
            if shard.partition != assignment.partition {
                return Err(ShardedModelManifestError::TopologyPartitionMismatch {
                    shard_id: assignment.shard_id,
                });
            }
        }
        Ok(())
    }
}

/// Validation failure for one pre-sharded model manifest.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ShardedModelManifestError {
    /// The manifest carried no shard references.
    #[error("sharded model manifest is empty")]
    EmptyManifest,
    /// Two shard references used the same shard ID.
    #[error("sharded model manifest duplicated shard {shard_id}")]
    DuplicateShardId {
        /// Duplicate shard identifier.
        shard_id: usize,
    },
    /// The manifest omitted one shard ID in the expected contiguous range.
    #[error("sharded model manifest is missing shard {shard_id}")]
    MissingShardId {
        /// Missing shard identifier.
        shard_id: usize,
    },
    /// One shard partition did not match the declared layout.
    #[error("shard {shard_id} partition does not match {layout:?} layout")]
    PartitionLayoutMismatch {
        /// Mismatched shard identifier.
        shard_id: usize,
        /// Declared manifest layout.
        layout: ShardedModelLayoutKind,
    },
    /// The manifest belongs to a different served-artifact identity than the caller expected.
    #[error(
        "manifest served artifact `{manifest_served_artifact_digest}` does not match expected `{expected_served_artifact_digest}`"
    )]
    ServedArtifactDigestMismatch {
        /// Manifest served-artifact digest.
        manifest_served_artifact_digest: String,
        /// Caller-expected served-artifact digest.
        expected_served_artifact_digest: String,
    },
    /// The manifest layout and realized topology kind disagree.
    #[error("manifest layout {manifest:?} does not match topology kind {topology:?}")]
    TopologyKindMismatch {
        /// Declared manifest layout.
        manifest: ShardedModelLayoutKind,
        /// Realized topology kind.
        topology: ExecutionTopologyKind,
    },
    /// The manifest and realized topology target different backends.
    #[error(
        "manifest backend `{manifest_backend}` does not match topology backend `{topology_backend}`"
    )]
    TopologyBackendMismatch {
        /// Manifest backend.
        manifest_backend: String,
        /// Realized topology backend.
        topology_backend: String,
    },
    /// The manifest and realized topology disagree on shard count.
    #[error(
        "manifest shard count {manifest_shards} does not match topology shard count {topology_shards}"
    )]
    TopologyShardCountMismatch {
        /// Manifest shard count.
        manifest_shards: usize,
        /// Realized topology shard count.
        topology_shards: usize,
    },
    /// The realized topology referenced a shard not present in the manifest.
    #[error("topology referenced missing manifest shard {shard_id}")]
    TopologyShardMissing {
        /// Missing shard identifier.
        shard_id: usize,
    },
    /// One realized topology partition differed from the manifest.
    #[error("topology partition for shard {shard_id} differs from manifest")]
    TopologyPartitionMismatch {
        /// Mismatched shard identifier.
        shard_id: usize,
    },
}

fn partition_matches_layout(
    partition: &ExecutionPartition,
    layout: ShardedModelLayoutKind,
) -> bool {
    matches!(
        (layout, partition),
        (
            ShardedModelLayoutKind::Replicated,
            ExecutionPartition::Replica { .. } | ExecutionPartition::WholeModel
        ) | (
            ShardedModelLayoutKind::LayerSharded,
            ExecutionPartition::LayerRange { .. }
        ) | (
            ShardedModelLayoutKind::TensorSharded,
            ExecutionPartition::TensorRange { .. }
        )
    )
}

fn update_partition_digest(hasher: &mut Sha256, partition: &ExecutionPartition) {
    match partition {
        ExecutionPartition::WholeModel => hasher.update(b"whole_model"),
        ExecutionPartition::Replica { replica_index } => {
            hasher.update(b"replica|");
            hasher.update(replica_index.to_string().as_bytes());
        }
        ExecutionPartition::LayerRange {
            start_layer,
            end_layer,
        } => {
            hasher.update(b"layer_range|");
            hasher.update(start_layer.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(end_layer.to_string().as_bytes());
        }
        ExecutionPartition::TensorRange { axis, start, end } => {
            hasher.update(b"tensor_range|");
            hasher.update(axis.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(start.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(end.to_string().as_bytes());
        }
    }
}

/// Stable identity tuple for one reusable shared prompt prefix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixCacheIdentity {
    /// Stable served-artifact digest used to validate reusable prefix ownership.
    pub served_artifact_digest: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Stable model revision.
    pub model_revision: String,
    /// Stable weight-bundle digest.
    pub weight_bundle_digest: String,
    /// Tokenizer family label used to produce the prompt tokens.
    pub tokenizer_family: String,
    /// Stable tokenizer digest when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer_digest: Option<String>,
    /// Stable chat-template digest when prompt rendering supplied one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template_digest: Option<String>,
    /// Stable generation-defaults digest when prompt rendering depended on one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_defaults_digest: Option<String>,
    /// Tenant or security-domain binding for the reusable prefix when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    /// Stable sampler digest when reuse depended on request-level decode settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampler_digest: Option<String>,
    /// Stable backend compatibility label required for reuse.
    pub backend_compatibility: String,
    /// Stable digest of the reusable prompt-prefix tokens.
    pub prefix_digest: String,
    /// Number of reusable prompt-prefix tokens represented by the digest.
    pub prefix_tokens: usize,
}

#[allow(clippy::too_many_arguments)]
fn digest_served_artifact_identity(
    model_id: &str,
    model_revision: &str,
    weight_bundle_digest: &str,
    model_blob_digest: Option<&str>,
    tokenizer_digest: Option<&str>,
    chat_template_digest: Option<&str>,
    generation_defaults_digest: &str,
    weight_format: &str,
    quantization_family: QuantizationMode,
    backend: &BackendToolchainIdentity,
) -> String {
    let mut hasher = Sha256::new();
    update_identity_string(&mut hasher, model_id);
    update_identity_string(&mut hasher, model_revision);
    update_identity_string(&mut hasher, weight_bundle_digest);
    update_optional_identity_string(&mut hasher, model_blob_digest);
    update_optional_identity_string(&mut hasher, tokenizer_digest);
    update_optional_identity_string(&mut hasher, chat_template_digest);
    update_identity_string(&mut hasher, generation_defaults_digest);
    update_identity_string(&mut hasher, weight_format);
    update_identity_string(&mut hasher, quantization_family_label(quantization_family));
    update_identity_string(&mut hasher, backend.effective_backend.as_str());
    update_identity_string(&mut hasher, backend.toolchain_version.as_str());
    for feature in &backend.compiled_backend_features {
        update_identity_string(&mut hasher, feature);
    }
    format!("{:x}", hasher.finalize())
}

fn update_identity_string(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn update_optional_identity_string(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            update_identity_string(hasher, value);
        }
        None => hasher.update([0]),
    }
}

const fn quantization_family_label(mode: QuantizationMode) -> &'static str {
    match mode {
        QuantizationMode::None => "none",
        QuantizationMode::Int8Symmetric => "int8_symmetric",
        QuantizationMode::GgmlMxfp4 => "ggml_mxfp4",
        QuantizationMode::GgmlQ4_0 => "ggml_q4_0",
        QuantizationMode::GgmlQ4_1 => "ggml_q4_1",
        QuantizationMode::GgmlQ8_0 => "ggml_q8_0",
    }
}

/// Explicit runtime backend selection and fallback truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendUnavailablePolicy {
    /// Refuse execution instead of switching backends.
    Refuse,
    /// Fall back to a compatible backend explicitly.
    FallbackToCompatibleBackend,
}

/// Explicit degraded-state policy for one served product path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendDegradedPolicy {
    /// Refuse execution when the requested backend is degraded.
    Refuse,
    /// Continue on the same backend explicitly even though it is degraded.
    AllowSameBackend,
}

/// Explicit served-product backend policy for unavailable and degraded states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServedProductBackendPolicy {
    /// Action when the requested backend is unavailable.
    pub unavailable: BackendUnavailablePolicy,
    /// Action when the requested backend is degraded but still executable.
    pub degraded: BackendDegradedPolicy,
}

impl ServedProductBackendPolicy {
    /// Creates a served-product backend policy.
    #[must_use]
    pub const fn new(
        unavailable: BackendUnavailablePolicy,
        degraded: BackendDegradedPolicy,
    ) -> Self {
        Self {
            unavailable,
            degraded,
        }
    }

    /// Returns the default same-backend CPU-style policy.
    #[must_use]
    pub const fn same_backend_only() -> Self {
        Self::new(
            BackendUnavailablePolicy::Refuse,
            BackendDegradedPolicy::AllowSameBackend,
        )
    }

    /// Returns a policy that allows explicit cross-backend fallback.
    #[must_use]
    pub const fn fallback_to_compatible_backend(degraded: BackendDegradedPolicy) -> Self {
        Self::new(
            BackendUnavailablePolicy::FallbackToCompatibleBackend,
            degraded,
        )
    }
}

/// Trigger that forced the runtime to leave the preferred direct path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServedProductFallbackTrigger {
    /// The requested backend was unavailable and could not execute the request directly.
    RequestedBackendUnavailable,
    /// The requested backend was available but degraded.
    RequestedBackendDegraded,
    /// The preferred path was numerically unsafe for the current request.
    NumericalSafetyRisk,
    /// The preferred path could not execute within the admitted memory budget.
    MemoryPressure,
    /// The preferred plan or kernel was not yet available.
    PlanUnavailable,
    /// The backend returned a transient failure and the runtime may retry explicitly.
    TransientBackendFailure,
}

/// Allowed fallback action in the full served-product lattice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServedProductFallbackAction {
    /// Refuse execution rather than changing semantics or backend posture.
    Refuse,
    /// Continue on the same backend explicitly in a degraded mode.
    Degrade,
    /// Replan execution explicitly, potentially onto another backend.
    Replan,
    /// Retry the same request explicitly after a transient failure.
    Retry,
    /// Continue on the same backend using a slower but still-correct path.
    SameBackendSlowPath,
}

/// Full fallback lattice for one served product path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServedProductFallbackLattice {
    /// Action when the requested backend is unavailable.
    pub unavailable: ServedProductFallbackAction,
    /// Action when the requested backend is degraded but still executable.
    pub degraded: ServedProductFallbackAction,
    /// Action when the preferred path is numerically unsafe.
    pub numerical_safety: ServedProductFallbackAction,
    /// Action when the preferred path exceeds the admitted memory budget.
    pub memory_pressure: ServedProductFallbackAction,
    /// Action when the preferred plan or kernel is not yet available.
    pub plan_unavailable: ServedProductFallbackAction,
    /// Action when the backend returns a transient failure.
    pub transient_backend_failure: ServedProductFallbackAction,
}

impl ServedProductFallbackLattice {
    /// Creates a full fallback lattice.
    #[must_use]
    pub const fn new(
        unavailable: ServedProductFallbackAction,
        degraded: ServedProductFallbackAction,
        numerical_safety: ServedProductFallbackAction,
        memory_pressure: ServedProductFallbackAction,
        plan_unavailable: ServedProductFallbackAction,
        transient_backend_failure: ServedProductFallbackAction,
    ) -> Self {
        Self {
            unavailable,
            degraded,
            numerical_safety,
            memory_pressure,
            plan_unavailable,
            transient_backend_failure,
        }
    }

    /// Conservative default lattice for same-backend-only serving.
    #[must_use]
    pub const fn same_backend_only() -> Self {
        Self::new(
            ServedProductFallbackAction::Refuse,
            ServedProductFallbackAction::Degrade,
            ServedProductFallbackAction::Refuse,
            ServedProductFallbackAction::Refuse,
            ServedProductFallbackAction::SameBackendSlowPath,
            ServedProductFallbackAction::Retry,
        )
    }

    /// Default lattice for paths that may explicitly replan onto a compatible backend.
    #[must_use]
    pub const fn fallback_to_compatible_backend(degraded: BackendDegradedPolicy) -> Self {
        Self::new(
            ServedProductFallbackAction::Replan,
            degraded.to_fallback_action(),
            ServedProductFallbackAction::Refuse,
            ServedProductFallbackAction::Replan,
            ServedProductFallbackAction::SameBackendSlowPath,
            ServedProductFallbackAction::Retry,
        )
    }

    /// Derives the full lattice from the narrower backend-selection policy.
    #[must_use]
    pub const fn from_backend_policy(policy: ServedProductBackendPolicy) -> Self {
        let unavailable = match policy.unavailable {
            BackendUnavailablePolicy::Refuse => ServedProductFallbackAction::Refuse,
            BackendUnavailablePolicy::FallbackToCompatibleBackend => {
                ServedProductFallbackAction::Replan
            }
        };
        let degraded = policy.degraded.to_fallback_action();
        Self::new(
            unavailable,
            degraded,
            ServedProductFallbackAction::Refuse,
            unavailable,
            ServedProductFallbackAction::SameBackendSlowPath,
            ServedProductFallbackAction::Retry,
        )
    }

    /// Returns the allowed action for one fallback trigger.
    #[must_use]
    pub const fn action_for(
        self,
        trigger: ServedProductFallbackTrigger,
    ) -> ServedProductFallbackAction {
        match trigger {
            ServedProductFallbackTrigger::RequestedBackendUnavailable => self.unavailable,
            ServedProductFallbackTrigger::RequestedBackendDegraded => self.degraded,
            ServedProductFallbackTrigger::NumericalSafetyRisk => self.numerical_safety,
            ServedProductFallbackTrigger::MemoryPressure => self.memory_pressure,
            ServedProductFallbackTrigger::PlanUnavailable => self.plan_unavailable,
            ServedProductFallbackTrigger::TransientBackendFailure => self.transient_backend_failure,
        }
    }

    /// Returns whether one trigger/action pair is allowed by the lattice.
    #[must_use]
    pub fn allows(
        self,
        trigger: ServedProductFallbackTrigger,
        action: ServedProductFallbackAction,
    ) -> bool {
        self.action_for(trigger) == action
    }
}

impl BackendDegradedPolicy {
    #[must_use]
    const fn to_fallback_action(self) -> ServedProductFallbackAction {
        match self {
            Self::Refuse => ServedProductFallbackAction::Refuse,
            Self::AllowSameBackend => ServedProductFallbackAction::Degrade,
        }
    }
}

/// Actual backend-selection state used by one served request or capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendSelectionState {
    /// The requested backend executes directly with no fallback or degradation.
    Direct,
    /// The requested backend executes explicitly while degraded.
    SameBackendDegraded,
    /// The requested backend executes explicitly on a slower same-backend path.
    SameBackendSlowPath,
    /// Execution switched to a different backend explicitly.
    CrossBackendFallback,
    /// Execution succeeded only after an explicit retry.
    Retried,
    /// Execution was explicitly refused under the active fallback lattice.
    Refused,
}

/// Explicit runtime backend selection and fallback truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendSelection {
    /// Backend the caller or higher-level runtime requested.
    pub requested_backend: String,
    /// Backend that will actually execute the work.
    pub effective_backend: String,
    /// Selected device for the effective backend, when one exists.
    pub selected_device: Option<DeviceDescriptor>,
    /// All concrete devices selected for the effective backend path, when the runtime chose more than one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_devices: Vec<DeviceDescriptor>,
    /// Explicit runtime-owned allocator/kernel-cache/device-budget state for the effective backend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_resources: Option<BackendRuntimeResources>,
    /// Explicit backend-extension families kept outside the base primitive-op list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backend_extensions: Vec<BackendExtensionSupport>,
    /// Supported op labels for the advertised product path.
    pub supported_ops: Vec<String>,
    /// Explicit served-product fallback and degraded-state policy.
    pub policy: ServedProductBackendPolicy,
    /// Full fallback lattice used to decide allowed state transitions.
    pub fallback_lattice: ServedProductFallbackLattice,
    /// Actual selection state under that policy.
    pub selection_state: BackendSelectionState,
    /// Explicit trigger that forced the runtime off the preferred direct path.
    pub fallback_trigger: Option<ServedProductFallbackTrigger>,
    /// Explicit fallback action taken under the lattice.
    pub fallback_action: Option<ServedProductFallbackAction>,
    /// Explicit fallback reason when the effective backend differs from the requested backend.
    pub fallback_reason: Option<String>,
    /// Explicit degraded-state reason when the requested backend still executes while degraded.
    pub degraded_reason: Option<String>,
    /// Retry attempt count when the runtime retried explicitly.
    pub retry_attempt: Option<u32>,
    /// Explicit multi-device or sharded topology when execution uses more than one concrete placement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_topology: Option<ExecutionTopologyPlan>,
    /// Declared clustered execution capability profile advertised before any request executes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_execution_capability_profile: Option<ClusterExecutionCapabilityProfile>,
    /// Published cluster trust posture that bounds advertised cluster capability claims.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_compute_market_trust_assessment: Option<ClusterComputeMarketTrustAssessment>,
}

impl BackendSelection {
    /// Creates a direct backend selection with no fallback.
    #[must_use]
    pub fn direct(
        backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
    ) -> Self {
        Self::direct_with_policy(
            backend,
            selected_device,
            supported_ops,
            ServedProductBackendPolicy::same_backend_only(),
        )
    }

    /// Creates a direct backend selection with an explicit served-product policy.
    #[must_use]
    pub fn direct_with_policy(
        backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
        policy: ServedProductBackendPolicy,
    ) -> Self {
        let backend = backend.into();
        let execution_topology = selected_device.as_ref().map(|device| {
            ExecutionTopologyPlan::single_device(backend.clone(), device.inventory_qualifiers())
        });
        Self {
            requested_backend: backend.clone(),
            effective_backend: backend,
            selected_devices: selected_device.iter().cloned().collect(),
            execution_topology,
            cluster_execution_capability_profile: None,
            cluster_compute_market_trust_assessment: None,
            selected_device,
            runtime_resources: None,
            backend_extensions: Vec::new(),
            supported_ops,
            policy,
            fallback_lattice: ServedProductFallbackLattice::from_backend_policy(policy),
            selection_state: BackendSelectionState::Direct,
            fallback_trigger: None,
            fallback_action: None,
            fallback_reason: None,
            degraded_reason: None,
            retry_attempt: None,
        }
    }

    /// Creates an explicit fallback selection.
    #[must_use]
    pub fn fallback(
        requested_backend: impl Into<String>,
        effective_backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
        fallback_reason: impl Into<String>,
    ) -> Self {
        Self::fallback_with_policy(
            requested_backend,
            effective_backend,
            selected_device,
            supported_ops,
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend,
            ),
            fallback_reason,
        )
    }

    /// Creates an explicit fallback selection with an explicit served-product policy.
    #[must_use]
    pub fn fallback_with_policy(
        requested_backend: impl Into<String>,
        effective_backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
        policy: ServedProductBackendPolicy,
        fallback_reason: impl Into<String>,
    ) -> Self {
        let requested_backend = requested_backend.into();
        let effective_backend = effective_backend.into();
        let execution_topology = selected_device.as_ref().map(|device| {
            ExecutionTopologyPlan::single_device(
                effective_backend.clone(),
                device.inventory_qualifiers(),
            )
        });
        Self {
            requested_backend,
            effective_backend,
            selected_devices: selected_device.iter().cloned().collect(),
            execution_topology,
            cluster_execution_capability_profile: None,
            cluster_compute_market_trust_assessment: None,
            selected_device,
            runtime_resources: None,
            backend_extensions: Vec::new(),
            supported_ops,
            policy,
            fallback_lattice: ServedProductFallbackLattice::from_backend_policy(policy),
            selection_state: BackendSelectionState::CrossBackendFallback,
            fallback_trigger: Some(ServedProductFallbackTrigger::RequestedBackendUnavailable),
            fallback_action: Some(ServedProductFallbackAction::Replan),
            fallback_reason: Some(fallback_reason.into()),
            degraded_reason: None,
            retry_attempt: None,
        }
    }

    /// Creates an explicit same-backend degraded selection.
    #[must_use]
    pub fn degraded(
        backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
        policy: ServedProductBackendPolicy,
        degraded_reason: impl Into<String>,
    ) -> Self {
        let backend = backend.into();
        let execution_topology = selected_device.as_ref().map(|device| {
            ExecutionTopologyPlan::single_device(backend.clone(), device.inventory_qualifiers())
        });
        Self {
            requested_backend: backend.clone(),
            effective_backend: backend,
            selected_devices: selected_device.iter().cloned().collect(),
            execution_topology,
            cluster_execution_capability_profile: None,
            cluster_compute_market_trust_assessment: None,
            selected_device,
            runtime_resources: None,
            backend_extensions: Vec::new(),
            supported_ops,
            policy,
            fallback_lattice: ServedProductFallbackLattice::from_backend_policy(policy),
            selection_state: BackendSelectionState::SameBackendDegraded,
            fallback_trigger: Some(ServedProductFallbackTrigger::RequestedBackendDegraded),
            fallback_action: Some(ServedProductFallbackAction::Degrade),
            fallback_reason: None,
            degraded_reason: Some(degraded_reason.into()),
            retry_attempt: None,
        }
    }

    /// Creates an explicit same-backend slow-path selection.
    #[must_use]
    pub fn slow_path(
        backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
        policy: ServedProductBackendPolicy,
        trigger: ServedProductFallbackTrigger,
        reason: impl Into<String>,
    ) -> Self {
        let backend = backend.into();
        let execution_topology = selected_device.as_ref().map(|device| {
            ExecutionTopologyPlan::single_device(backend.clone(), device.inventory_qualifiers())
        });
        Self {
            requested_backend: backend.clone(),
            effective_backend: backend,
            selected_devices: selected_device.iter().cloned().collect(),
            execution_topology,
            cluster_execution_capability_profile: None,
            cluster_compute_market_trust_assessment: None,
            selected_device,
            runtime_resources: None,
            backend_extensions: Vec::new(),
            supported_ops,
            policy,
            fallback_lattice: ServedProductFallbackLattice::from_backend_policy(policy),
            selection_state: BackendSelectionState::SameBackendSlowPath,
            fallback_trigger: Some(trigger),
            fallback_action: Some(ServedProductFallbackAction::SameBackendSlowPath),
            fallback_reason: Some(reason.into()),
            degraded_reason: None,
            retry_attempt: None,
        }
    }

    /// Creates an explicit retried selection.
    #[must_use]
    pub fn retried(
        backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
        policy: ServedProductBackendPolicy,
        trigger: ServedProductFallbackTrigger,
        retry_attempt: u32,
        reason: impl Into<String>,
    ) -> Self {
        let backend = backend.into();
        let execution_topology = selected_device.as_ref().map(|device| {
            ExecutionTopologyPlan::single_device(backend.clone(), device.inventory_qualifiers())
        });
        Self {
            requested_backend: backend.clone(),
            effective_backend: backend,
            selected_devices: selected_device.iter().cloned().collect(),
            execution_topology,
            cluster_execution_capability_profile: None,
            cluster_compute_market_trust_assessment: None,
            selected_device,
            runtime_resources: None,
            backend_extensions: Vec::new(),
            supported_ops,
            policy,
            fallback_lattice: ServedProductFallbackLattice::from_backend_policy(policy),
            selection_state: BackendSelectionState::Retried,
            fallback_trigger: Some(trigger),
            fallback_action: Some(ServedProductFallbackAction::Retry),
            fallback_reason: Some(reason.into()),
            degraded_reason: None,
            retry_attempt: Some(retry_attempt),
        }
    }

    /// Creates an explicit refused selection.
    #[must_use]
    pub fn refused(
        requested_backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
        policy: ServedProductBackendPolicy,
        trigger: ServedProductFallbackTrigger,
        reason: impl Into<String>,
    ) -> Self {
        let requested_backend = requested_backend.into();
        let execution_topology = selected_device.as_ref().map(|device| {
            ExecutionTopologyPlan::single_device(
                requested_backend.clone(),
                device.inventory_qualifiers(),
            )
        });
        Self {
            requested_backend: requested_backend.clone(),
            effective_backend: requested_backend,
            selected_devices: selected_device.iter().cloned().collect(),
            execution_topology,
            cluster_execution_capability_profile: None,
            cluster_compute_market_trust_assessment: None,
            selected_device,
            runtime_resources: None,
            backend_extensions: Vec::new(),
            supported_ops,
            policy,
            fallback_lattice: ServedProductFallbackLattice::from_backend_policy(policy),
            selection_state: BackendSelectionState::Refused,
            fallback_trigger: Some(trigger),
            fallback_action: Some(ServedProductFallbackAction::Refuse),
            fallback_reason: Some(reason.into()),
            degraded_reason: None,
            retry_attempt: None,
        }
    }

    /// Creates a direct selection from a discovered backend.
    pub fn from_backend<B>(backend: &B, supported_ops: &[&str]) -> Result<Self, RuntimeError>
    where
        B: DeviceDiscovery + ?Sized,
    {
        Ok(Self::direct(
            backend.backend_name(),
            backend.discover_devices()?.into_iter().next(),
            supported_ops
                .iter()
                .map(|label| String::from(*label))
                .collect(),
        )
        .with_runtime_resources(backend.runtime_resources())
        .with_backend_extensions(backend.extension_support()))
    }

    /// Creates a fallback selection to an effective backend discovered at runtime.
    pub fn fallback_to_backend<B>(
        requested_backend: impl Into<String>,
        effective_backend: &B,
        supported_ops: &[&str],
        fallback_reason: impl Into<String>,
    ) -> Result<Self, RuntimeError>
    where
        B: DeviceDiscovery + ?Sized,
    {
        Ok(Self::fallback_with_policy(
            requested_backend,
            effective_backend.backend_name(),
            effective_backend.discover_devices()?.into_iter().next(),
            supported_ops
                .iter()
                .map(|label| String::from(*label))
                .collect(),
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend,
            ),
            fallback_reason,
        )
        .with_runtime_resources(effective_backend.runtime_resources())
        .with_backend_extensions(effective_backend.extension_support()))
    }

    /// Attaches explicit backend runtime resource state.
    #[must_use]
    pub fn with_runtime_resources(
        mut self,
        runtime_resources: Option<BackendRuntimeResources>,
    ) -> Self {
        self.runtime_resources = runtime_resources;
        self
    }

    /// Attaches explicit backend-extension support truth.
    #[must_use]
    pub fn with_backend_extensions(
        mut self,
        backend_extensions: Vec<BackendExtensionSupport>,
    ) -> Self {
        self.backend_extensions = backend_extensions;
        self
    }

    /// Replaces the selected-device set explicitly.
    #[must_use]
    pub fn with_selected_devices(mut self, selected_devices: Vec<DeviceDescriptor>) -> Self {
        self.selected_devices = selected_devices;
        if self.selected_device.is_none() {
            self.selected_device = self.selected_devices.first().cloned();
        }
        if self.execution_topology.is_none() && self.selected_devices.len() == 1 {
            if let Some(device) = self.selected_devices.first() {
                self.execution_topology = Some(ExecutionTopologyPlan::single_device(
                    self.effective_backend.clone(),
                    device.inventory_qualifiers(),
                ));
            }
        }
        self
    }

    /// Attaches an explicit multi-device or sharded execution topology.
    #[must_use]
    pub fn with_execution_topology(
        mut self,
        execution_topology: Option<ExecutionTopologyPlan>,
    ) -> Self {
        self.execution_topology = execution_topology;
        self
    }

    /// Attaches declared clustered execution capability truth for capability-side publication.
    #[must_use]
    pub fn with_cluster_execution_capability_profile(
        mut self,
        cluster_execution_capability_profile: ClusterExecutionCapabilityProfile,
    ) -> Self {
        self.cluster_execution_capability_profile = Some(cluster_execution_capability_profile);
        self
    }

    /// Attaches published cluster trust posture for capability-side publication.
    #[must_use]
    pub fn with_cluster_compute_market_trust_assessment(
        mut self,
        cluster_compute_market_trust_assessment: ClusterComputeMarketTrustAssessment,
    ) -> Self {
        self.cluster_compute_market_trust_assessment =
            Some(cluster_compute_market_trust_assessment);
        self
    }

    /// Returns the primary selected device, preserving legacy single-device callers.
    #[must_use]
    pub fn primary_selected_device(&self) -> Option<&DeviceDescriptor> {
        self.selected_device
            .as_ref()
            .or_else(|| self.selected_devices.first())
    }

    /// Returns all selected devices participating in the effective backend path.
    #[must_use]
    pub fn selected_devices(&self) -> Vec<&DeviceDescriptor> {
        if self.selected_devices.is_empty() {
            self.selected_device.iter().collect()
        } else {
            self.selected_devices.iter().collect()
        }
    }

    /// Returns selected-device inventory qualifiers enriched with current runtime budget truth.
    #[must_use]
    pub fn selected_device_inventory(&self) -> Option<DeviceInventoryQualifiers> {
        let mut qualifiers = self.primary_selected_device()?.inventory_qualifiers();
        qualifiers.free_memory_bytes = self
            .runtime_resources
            .as_ref()
            .and_then(|resources| resources.device_memory_budget.as_ref())
            .and_then(|budget| budget.available_execution_bytes);
        Some(qualifiers)
    }

    /// Returns all selected-device qualifiers enriched with current runtime budget truth.
    #[must_use]
    pub fn selected_devices_inventory(&self) -> Vec<DeviceInventoryQualifiers> {
        let runtime_free_memory = self
            .runtime_resources
            .as_ref()
            .and_then(|resources| resources.device_memory_budget.as_ref())
            .and_then(|budget| budget.available_execution_bytes);
        self.selected_devices()
            .into_iter()
            .map(|device| {
                let mut qualifiers = device.inventory_qualifiers();
                qualifiers.free_memory_bytes = runtime_free_memory;
                qualifiers
            })
            .collect()
    }

    /// Returns the explicit execution topology, deriving the current single-device plan when possible.
    #[must_use]
    pub fn execution_topology_plan(&self) -> Option<ExecutionTopologyPlan> {
        self.execution_topology.clone().or_else(|| {
            self.primary_selected_device().map(|device| {
                ExecutionTopologyPlan::single_device(
                    self.effective_backend.clone(),
                    device.inventory_qualifiers(),
                )
            })
        })
    }
}

impl DeliveredExecutionContext {
    /// Derives delivered execution context from runtime backend selection truth.
    #[must_use]
    pub fn from_backend_selection(selection: &BackendSelection) -> Self {
        Self::new(
            selection.effective_backend.clone(),
            selection.execution_topology_plan(),
            selection.selected_devices_inventory(),
        )
    }
}

/// Minimal execution metrics.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    /// Number of plan steps executed.
    pub steps_executed: usize,
    /// Total kernel or step dispatch count surfaced by the backend path.
    pub kernel_count: usize,
    /// Total bytes moved or written by the backend path.
    pub bytes_moved: u64,
    /// Number of execution-plan cache hits observed during this execution.
    pub plan_cache_hits: usize,
    /// Number of execution-plan cache misses observed during this execution.
    pub plan_cache_misses: usize,
    /// Stable digest of the execution plan used for this run, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_plan_digest: Option<String>,
    /// Explicit warm/cold compile-path evidence for this run, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_path: Option<CompilePathEvidence>,
}

/// Trait for backend-owned buffers.
pub trait BufferHandle {
    /// Returns the buffer tensor spec.
    fn spec(&self) -> &TensorSpec;

    /// Returns the storage posture for the buffer.
    fn storage_kind(&self) -> BufferStorageKind {
        BufferStorageKind::DenseF32
    }

    /// Returns the storage identity and view posture for the buffer when the
    /// backend exposes it.
    fn storage_contract(&self) -> Option<BufferStorageContract> {
        None
    }
}

/// Stable storage identity for one backend-owned buffer family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BufferStorageIdentity(pub u64);

/// Runtime-visible storage identity plus view posture for one buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BufferStorageContract {
    /// Stable logical storage identity.
    pub identity: BufferStorageIdentity,
    /// View posture over that storage.
    pub view_semantics: ViewSemantics,
}

/// Physical residency of a backend-owned buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BufferResidency {
    /// Storage lives in host-managed memory.
    Host,
    /// Storage lives in backend-owned device memory.
    Backend,
}

/// Explicit buffer storage kind surfaced by runtime backends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BufferStorageKind {
    /// Ordinary dense `f32` tensor storage.
    DenseF32,
    /// Dense `f32` storage that came from a quantized source tensor.
    DequantizedF32 {
        /// Source quantization mode that was dequantized.
        source_quantization: QuantizationMode,
    },
    /// Quantized GGML/GGUF block storage that remains quantized.
    QuantizedBlocks {
        /// Quantized storage family.
        mode: QuantizationMode,
        /// Stable GGML block layout.
        layout: QuantizedBlockLayout,
        /// Whether the storage is host- or backend-resident.
        residency: BufferResidency,
    },
}

/// How a runtime load plan sources model artifact bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactStorageKind {
    /// The artifact was copied into an in-memory buffer before planning.
    InMemoryCopy,
    /// The artifact stays backed by a paged local blob.
    PagedLocalBlob,
}

/// Blob family used by a paged local model artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactBlobKind {
    /// Standalone GGUF file discovered on disk.
    GgufFile,
    /// Ollama-managed blob resolved by digest.
    OllamaBlob,
}

/// Actual local read path used for a paged model artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactReadPath {
    /// The artifact bytes are exposed through a memory map.
    MemoryMapped,
    /// The artifact bytes are exposed from a buffered host copy.
    Buffered,
}

/// Runtime-visible storage truth for a model artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelArtifactStorage {
    /// Stable artifact name.
    pub artifact_name: String,
    /// Stable SHA-256 digest of the artifact bytes.
    pub artifact_sha256: String,
    /// High-level storage posture used by the runtime.
    pub storage_kind: ModelArtifactStorageKind,
    /// Blob family when the runtime kept paged local blob storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob_kind: Option<ModelArtifactBlobKind>,
    /// Actual local read path when the runtime kept paged local blob storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_path: Option<ArtifactReadPath>,
    /// Logical page size when the runtime kept paged local blob storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<usize>,
    /// Explicit fallback reason when mmap was preferred but not used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

impl ModelArtifactStorage {
    /// Creates storage truth for an eager in-memory artifact copy.
    #[must_use]
    pub fn in_memory_copy(
        artifact_name: impl Into<String>,
        artifact_sha256: impl Into<String>,
    ) -> Self {
        Self {
            artifact_name: artifact_name.into(),
            artifact_sha256: artifact_sha256.into(),
            storage_kind: ModelArtifactStorageKind::InMemoryCopy,
            blob_kind: None,
            read_path: None,
            page_size: None,
            fallback_reason: None,
        }
    }

    /// Creates storage truth for a paged local blob artifact.
    #[must_use]
    pub fn paged_local_blob(
        artifact_name: impl Into<String>,
        artifact_sha256: impl Into<String>,
        blob_kind: ModelArtifactBlobKind,
        read_path: ArtifactReadPath,
        page_size: usize,
        fallback_reason: Option<String>,
    ) -> Self {
        Self {
            artifact_name: artifact_name.into(),
            artifact_sha256: artifact_sha256.into(),
            storage_kind: ModelArtifactStorageKind::PagedLocalBlob,
            blob_kind: Some(blob_kind),
            read_path: Some(read_path),
            page_size: Some(page_size),
            fallback_reason,
        }
    }
}

/// Runtime-visible paged tensor byte plan derived from a blob-backed artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PagedTensorStoragePlan {
    /// Stable tensor name.
    pub tensor_name: String,
    /// Backing artifact name.
    pub artifact_name: String,
    /// Byte offset inside the artifact.
    pub byte_offset: u64,
    /// Tensor byte length inside the artifact.
    pub byte_length: u64,
    /// Logical page size for reads over the tensor bytes.
    pub page_size: usize,
    /// Total page count for the tensor byte range.
    pub page_count: usize,
}

/// Trait for device discovery.
pub trait DeviceDiscovery {
    /// Returns the backend name.
    fn backend_name(&self) -> BackendName;

    /// Returns discovered devices.
    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError>;

    /// Returns current runtime health.
    fn health(&self) -> RuntimeHealth;

    /// Returns explicit allocator/kernel-cache/device-budget state for the effective backend.
    fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        None
    }

    /// Returns typed backend-extension families supported outside the base primitive-op list.
    fn extension_support(&self) -> Vec<BackendExtensionSupport> {
        Vec::new()
    }
}

/// Trait for backend allocators.
pub trait Allocator {
    /// Concrete buffer type.
    type Buffer: BufferHandle;

    /// Allocates a buffer for a tensor spec.
    fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError>;
}

/// Trait for graph execution.
pub trait ExecutionBackend {
    /// Concrete buffer type.
    type Buffer: BufferHandle;

    /// Executes a compiled plan with host-supplied inputs.
    fn execute(
        &mut self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, Self::Buffer>,
    ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError>;
}

/// Execution result containing output buffers and basic metrics.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionResult<B> {
    /// Materialized outputs by tensor ID.
    pub outputs: BTreeMap<TensorId, B>,
    /// Runtime metrics for the execution.
    pub metrics: ExecutionMetrics,
}

fn sorted_distinct_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort_unstable();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

    use std::collections::BTreeMap;

    use ed25519_dalek::SigningKey;
    use psionic_core::{
        BackendExtensionKind, DType, Device, DeviceKind, PsionicRefusalCode, PsionicRefusalScope,
        QuantizationMode, Shape, TensorSpec,
    };
    use psionic_ir::{ExecutionOp, ExecutionPlan, ExecutionStep};
    use serde_json::json;

    use super::{
        AcceleratorDeliverabilityDifferenceCode, AcceleratorDeliverabilityStatus,
        AcceleratorExecutionRequirement, AdmissionRefusalReason, Allocator, AllocatorPoolPolicy,
        AllocatorPoolReport, AllocatorPoolState, AmdBackendReport, AmdDeviceMetadata,
        AmdDriverBinding, AmdOptInStatus, AmdRecoveryAction, AmdRecoveryProfile, AmdRiskLevel,
        AmdRiskProfile, AmdRuntimeMode, AmdTopologyInfo, ArtifactReadPath, BackendDegradedPolicy,
        BackendExtensionExecution, BackendExtensionSupport, BackendHealthTracker,
        BackendRuntimeResources, BackendSelection, BackendSelectionState, BackendToolchainIdentity,
        BatchExecutionPosture, BufferHandle, BufferResidency, BufferStorageKind, CacheAction,
        CacheInvalidationTrigger, CacheKind, CacheObservation, ClusterAdmissionFactKind,
        ClusterArtifactResidencyDisposition, ClusterCacheCapability, ClusterCacheScope,
        ClusterCacheUsage, ClusterCommandAuthorityScopeEvidence, ClusterCommandProvenanceEvidence,
        ClusterCommitAuthorityEvidence, ClusterCommunicationClass, ClusterCommunicationEligibility,
        ClusterComputeMarketTrustAssessment, ClusterComputeMarketTrustDisposition,
        ClusterComputeMarketTrustRefusalReason, ClusterDiscoveryPosture,
        ClusterEvidenceBundlePayload, ClusterEvidenceBundleStatus,
        ClusterEvidenceBundleVerificationError, ClusterExecutionCapabilityProfile,
        ClusterExecutionContext, ClusterExecutionDisposition, ClusterExecutionLane,
        ClusterFallbackReason, ClusterFallbackStep, ClusterPolicyDigest, ClusterPolicyDigestKind,
        ClusterSelectedNode, ClusterServingSemantics, ClusterSettlementProvenanceInput,
        ClusterTransportClass, ClusterTrustPosture, ClusterWarmRoutePosture,
        DEFAULT_PENALTY_LOOKBACK, DeliveredExecutionContext, DeterminismContractError,
        DeterminismMode, DeterministicAlgorithmPolicy, DeviceDescriptor, DeviceDiscovery,
        DeviceInventoryQualifiers, DeviceMemoryBudget, DeviceMemoryClass, DevicePerformanceClass,
        ExecutionBackend, ExecutionCapabilityProfile, ExecutionDeliveryProof, ExecutionMetrics,
        ExecutionPartition, ExecutionPlanCachePolicy, ExecutionPlanCacheReport,
        ExecutionPlanCacheState, ExecutionResult, ExecutionTopologyKind, ExecutionTopologyPlan,
        GenerationSchedulerFallbackCount, GenerationSchedulerFallbackReason,
        GenerationSchedulerMetrics, GenerationSchedulerPolicy, GeneratorScope, HealthStatus,
        KernelCachePolicy, KernelCacheReport, KernelCacheState, KvCacheAccounting,
        KvCacheDeviceScope, KvCachePageLayout, KvCachePolicy, KvCacheSpillPolicy, KvCacheState,
        KvResidencyAccounting, KvResidencyExternalLocator, KvResidencyMovement,
        KvResidencyMovementKind, KvResidencyTier, KvResidencyTierState, LoadedModelMemoryState,
        LoadedModelResidency, LoadedModelState, LocalRuntimeObservability,
        LocalServingIsolationPolicy, MemoryBudget, MemoryResidencySnapshot, ModelAdmissionDecision,
        ModelArtifactBlobKind, ModelArtifactStorage, ModelArtifactStorageKind, ModelMemoryPlan,
        ModelResidencyPolicy, NvidiaBackendReport, NvidiaDeviceMetadata, NvidiaRecoveryAction,
        NvidiaRecoveryProfile, NvidiaRiskLevel, NvidiaRiskProfile, NvidiaTopologyInfo,
        PagedTensorStoragePlan, PrefixCacheIdentity, PrefixCacheReusePolicy, PrefixCacheState,
        QuantizationDispatchRequest, QuantizationDispatchWorkload, QuantizationExecution,
        QuantizationKernelStrategy, QuantizationLoadPath, QuantizationSupport, QueueDiscipline,
        QueuePolicy, ResidencyPressureAction, RuntimeDeterminismContract, RuntimeDispatchPlan,
        RuntimeDispatchPolicy, RuntimeError, RuntimeHealth, RuntimeTransitionEvent,
        RuntimeTransitionKind, RuntimeTransitionLog, RuntimeWorkClass, RuntimeWorkItem,
        SamplingPolicy, SamplingStrategy, SandboxAcceleratorAccess,
        SandboxExecutionCapabilityProfile, SandboxExecutionEvidence, SandboxExecutionExit,
        SandboxExecutionExitKind, SandboxExecutionRequestIdentity, SandboxExecutionResourceSummary,
        SandboxFilesystemPolicy, SandboxFilesystemRoot, SandboxIsolationBoundary,
        SandboxNetworkMode, SandboxNetworkPolicy, SandboxProcessPolicy, SandboxResourceLimits,
        ServedArtifactIdentity, ServedProductBackendPolicy, ServedProductFallbackAction,
        ServedProductFallbackLattice, ServedProductFallbackTrigger, SettlementLinkageInput,
        ShardedModelArtifactRef, ShardedModelLayoutKind, ShardedModelManifest,
        ShardedModelManifestError, SignedClusterEvidenceBundle, ThroughputClass, TokenSampler,
        TrainingCheckpointAvailability, TrainingCheckpointReference, TrainingCollectiveContext,
        TrainingCollectiveKind, TrainingCollectiveQuantization, TrainingDeviceMeshAxis,
        TrainingDeviceMeshAxisKind, TrainingDeviceMeshContext, TrainingElasticMembershipContext,
        TrainingRecoveryContext, TrainingRecoveryPosture, apply_sampling_penalties,
        benchmark_dispatch_plan, benchmark_quantization_dispatch,
        default_cache_invalidation_policy, plan_model_admission,
    };

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct MockBuffer {
        spec: TensorSpec,
    }

    impl BufferHandle for MockBuffer {
        fn spec(&self) -> &TensorSpec {
            &self.spec
        }
    }

    struct MockRuntime;

    impl DeviceDiscovery for MockRuntime {
        fn backend_name(&self) -> super::BackendName {
            "mock"
        }

        fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
            Ok(vec![DeviceDescriptor {
                backend: String::from("mock"),
                device: Device::cpu(),
                device_name: Some(String::from("mock cpu")),
                supported_dtypes: vec![DType::F32],
                supported_quantization: vec![QuantizationSupport {
                    mode: psionic_core::QuantizationMode::None,
                    load_path: QuantizationLoadPath::DenseF32,
                    execution: QuantizationExecution::Native,
                }],
                memory_capacity_bytes: None,
                unified_memory: Some(true),
                feature_flags: vec![String::from("mock_execution")],
                amd_metadata: None,
                nvidia_metadata: None,
            }])
        }

        fn health(&self) -> RuntimeHealth {
            RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("ready"),
            }
        }

        fn extension_support(&self) -> Vec<BackendExtensionSupport> {
            vec![
                BackendExtensionSupport::reference(BackendExtensionKind::RmsNorm),
                BackendExtensionSupport::backend_specialized(BackendExtensionKind::QuantizedMatmul),
            ]
        }
    }

    impl Allocator for MockRuntime {
        type Buffer = MockBuffer;

        fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError> {
            Ok(MockBuffer { spec: spec.clone() })
        }
    }

    fn sample_cuda_device() -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0"))),
            device_name: Some(String::from("NVIDIA CUDA Test Device")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(16 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: Some(NvidiaDeviceMetadata {
                topology: NvidiaTopologyInfo {
                    architecture: Some(String::from("ada")),
                    compute_capability: Some(String::from("8.9")),
                    pci_bdf: Some(String::from("00000000:01:00.0")),
                    sm_count: Some(76),
                    vram_bytes: Some(16 * 1024 * 1024 * 1024),
                    mig_profile: None,
                },
                risk: NvidiaRiskProfile {
                    level: NvidiaRiskLevel::Standard,
                    display_attached: Some(false),
                    mig_partitioned: false,
                    warnings: Vec::new(),
                },
                recovery: NvidiaRecoveryProfile {
                    supports_gpu_reset: Some(true),
                    expected_actions: vec![
                        NvidiaRecoveryAction::ProcessRestart,
                        NvidiaRecoveryAction::GpuReset,
                        NvidiaRecoveryAction::RebootHost,
                    ],
                },
            }),
        }
    }

    impl ExecutionBackend for MockRuntime {
        type Buffer = MockBuffer;

        fn execute(
            &mut self,
            plan: &ExecutionPlan,
            _inputs: &BTreeMap<psionic_core::TensorId, Self::Buffer>,
        ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
            Ok(ExecutionResult {
                outputs: BTreeMap::new(),
                metrics: ExecutionMetrics {
                    steps_executed: plan.steps.len(),
                    kernel_count: plan.steps.len(),
                    bytes_moved: 0,
                    plan_cache_hits: 0,
                    plan_cache_misses: 0,
                    execution_plan_digest: None,
                    compile_path: None,
                },
            })
        }
    }

    #[test]
    fn mock_runtime_reports_device_and_executes_plan() -> Result<(), RuntimeError> {
        let mut runtime = MockRuntime;
        let devices = runtime.discover_devices()?;
        if devices.len() != 1 {
            return Err(RuntimeError::Backend(format!(
                "expected 1 discovered device, found {}",
                devices.len()
            )));
        }
        if runtime.health().status != HealthStatus::Ready {
            return Err(RuntimeError::Backend(String::from(
                "expected mock runtime health to be ready",
            )));
        }

        let spec = TensorSpec::new(Shape::new(vec![1, 2]), DType::F32, Device::cpu());
        let buffer = runtime.allocate(&spec)?;
        let mut inputs = BTreeMap::new();
        inputs.insert(psionic_core::TensorId(0), buffer);

        let plan = ExecutionPlan {
            graph_digest: String::from("digest"),
            steps: vec![ExecutionStep {
                output: psionic_core::TensorId(1),
                op: ExecutionOp::Add,
                spec: TensorSpec::new(Shape::new(vec![1, 2]), DType::F32, Device::cpu()),
                inputs: vec![psionic_core::TensorId(0)],
            }],
            outputs: vec![psionic_core::TensorId(1)],
        };

        let result = runtime.execute(&plan, &inputs)?;
        if result.metrics.steps_executed != 1 {
            return Err(RuntimeError::Backend(format!(
                "expected 1 executed step, found {}",
                result.metrics.steps_executed
            )));
        }
        Ok(())
    }

    #[test]
    fn backend_selection_helpers_capture_direct_and_fallback_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let direct = BackendSelection::from_backend(&MockRuntime, &["input", "matmul"])?;
        assert_eq!(direct.requested_backend, "mock");
        assert_eq!(direct.effective_backend, "mock");
        assert_eq!(
            direct.supported_ops,
            vec![String::from("input"), String::from("matmul")]
        );
        assert_eq!(
            direct.policy,
            ServedProductBackendPolicy::same_backend_only()
        );
        assert_eq!(
            direct.fallback_lattice,
            ServedProductFallbackLattice::same_backend_only()
        );
        assert_eq!(direct.selection_state, BackendSelectionState::Direct);
        assert!(direct.fallback_trigger.is_none());
        assert!(direct.fallback_action.is_none());
        assert!(direct.fallback_reason.is_none());
        assert!(direct.degraded_reason.is_none());
        assert!(direct.retry_attempt.is_none());
        assert_eq!(
            serde_json::to_value(&direct)?,
            json!({
                "requested_backend": "mock",
                "effective_backend": "mock",
                "selected_device": {
                    "backend": "mock",
                    "device": {
                        "kind": "Cpu",
                        "ordinal": 0,
                        "label": "cpu:0"
                    },
                    "device_name": "mock cpu",
                    "supported_dtypes": ["F32"],
                    "supported_quantization": [{
                        "mode": "none",
                        "load_path": "dense_f32",
                        "execution": "native"
                    }],
                    "memory_capacity_bytes": null,
                    "unified_memory": true,
                    "feature_flags": ["mock_execution"]
                },
                "selected_devices": [{
                    "backend": "mock",
                    "device": {
                        "kind": "Cpu",
                        "ordinal": 0,
                        "label": "cpu:0"
                    },
                    "device_name": "mock cpu",
                    "supported_dtypes": ["F32"],
                    "supported_quantization": [{
                        "mode": "none",
                        "load_path": "dense_f32",
                        "execution": "native"
                    }],
                    "memory_capacity_bytes": null,
                    "unified_memory": true,
                    "feature_flags": ["mock_execution"]
                }],
                "backend_extensions": [
                    {
                        "kind": "rms_norm",
                        "execution": "reference"
                    },
                    {
                        "kind": "quantized_matmul",
                        "execution": "backend_specialized"
                    }
                ],
                "supported_ops": ["input", "matmul"],
                "policy": {
                    "unavailable": "refuse",
                    "degraded": "allow_same_backend"
                },
                "fallback_lattice": {
                    "unavailable": "refuse",
                    "degraded": "degrade",
                    "numerical_safety": "refuse",
                    "memory_pressure": "refuse",
                    "plan_unavailable": "same_backend_slow_path",
                    "transient_backend_failure": "retry"
                },
                "selection_state": "direct",
                "fallback_trigger": null,
                "fallback_action": null,
                "fallback_reason": null,
                "degraded_reason": null,
                "retry_attempt": null,
                "execution_topology": {
                    "effective_backend": "mock",
                    "kind": "single_device",
                    "assignments": [{
                        "shard_id": 0,
                        "device": {
                            "stable_device_id": "cpu:0",
                            "placement_index": 0
                        },
                        "partition": {
                            "kind": "whole_model"
                        }
                    }]
                }
            })
        );

        let fallback = BackendSelection::fallback_to_backend(
            "metal",
            &MockRuntime,
            &["input", "matmul"],
            "metal backend unavailable: offline",
        )?;
        assert_eq!(fallback.requested_backend, "metal");
        assert_eq!(fallback.effective_backend, "mock");
        assert_eq!(
            fallback.policy,
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend
            )
        );
        assert_eq!(
            fallback.fallback_lattice,
            ServedProductFallbackLattice::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend
            )
        );
        assert_eq!(
            fallback.selection_state,
            BackendSelectionState::CrossBackendFallback
        );
        assert_eq!(
            fallback.fallback_trigger,
            Some(ServedProductFallbackTrigger::RequestedBackendUnavailable)
        );
        assert_eq!(
            fallback.fallback_action,
            Some(ServedProductFallbackAction::Replan)
        );
        assert_eq!(
            fallback.fallback_reason.as_deref(),
            Some("metal backend unavailable: offline")
        );
        assert!(fallback.degraded_reason.is_none());
        assert!(fallback.retry_attempt.is_none());

        let degraded = BackendSelection::degraded(
            "metal",
            direct.selected_device.clone(),
            vec![String::from("input"), String::from("matmul")],
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend,
            ),
            "metal backend degraded: legacy-family device",
        );
        assert_eq!(degraded.requested_backend, "metal");
        assert_eq!(degraded.effective_backend, "metal");
        assert_eq!(
            degraded.policy,
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend
            )
        );
        assert_eq!(
            degraded.fallback_lattice,
            ServedProductFallbackLattice::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend
            )
        );
        assert_eq!(
            degraded.selection_state,
            BackendSelectionState::SameBackendDegraded
        );
        assert_eq!(
            degraded.fallback_trigger,
            Some(ServedProductFallbackTrigger::RequestedBackendDegraded)
        );
        assert_eq!(
            degraded.fallback_action,
            Some(ServedProductFallbackAction::Degrade)
        );
        assert!(degraded.fallback_reason.is_none());
        assert_eq!(
            degraded.degraded_reason.as_deref(),
            Some("metal backend degraded: legacy-family device")
        );
        assert!(degraded.retry_attempt.is_none());
        Ok(())
    }

    #[test]
    fn fallback_lattice_supports_refuse_replan_retry_and_same_backend_slow_path() {
        let lattice = ServedProductFallbackLattice::fallback_to_compatible_backend(
            BackendDegradedPolicy::AllowSameBackend,
        );
        assert!(lattice.allows(
            ServedProductFallbackTrigger::RequestedBackendUnavailable,
            ServedProductFallbackAction::Replan
        ));
        assert!(lattice.allows(
            ServedProductFallbackTrigger::RequestedBackendDegraded,
            ServedProductFallbackAction::Degrade
        ));
        assert!(lattice.allows(
            ServedProductFallbackTrigger::PlanUnavailable,
            ServedProductFallbackAction::SameBackendSlowPath
        ));
        assert!(lattice.allows(
            ServedProductFallbackTrigger::TransientBackendFailure,
            ServedProductFallbackAction::Retry
        ));
        assert!(lattice.allows(
            ServedProductFallbackTrigger::NumericalSafetyRisk,
            ServedProductFallbackAction::Refuse
        ));
        assert!(!lattice.allows(
            ServedProductFallbackTrigger::NumericalSafetyRisk,
            ServedProductFallbackAction::Replan
        ));
    }

    #[test]
    fn backend_selection_can_surface_refusal_retry_and_same_backend_slow_path() {
        let policy = ServedProductBackendPolicy::fallback_to_compatible_backend(
            BackendDegradedPolicy::AllowSameBackend,
        );
        let slow_path = BackendSelection::slow_path(
            "cuda",
            Some(sample_cuda_device()),
            vec![String::from("matmul")],
            policy,
            ServedProductFallbackTrigger::PlanUnavailable,
            "kernel fusion not compiled yet; using unfused path",
        );
        assert_eq!(
            slow_path.selection_state,
            BackendSelectionState::SameBackendSlowPath
        );
        assert_eq!(
            slow_path.fallback_action,
            Some(ServedProductFallbackAction::SameBackendSlowPath)
        );

        let retried = BackendSelection::retried(
            "cuda",
            Some(sample_cuda_device()),
            vec![String::from("matmul")],
            policy,
            ServedProductFallbackTrigger::TransientBackendFailure,
            1,
            "retry after transient launch timeout",
        );
        assert_eq!(retried.selection_state, BackendSelectionState::Retried);
        assert_eq!(
            retried.fallback_action,
            Some(ServedProductFallbackAction::Retry)
        );
        assert_eq!(retried.retry_attempt, Some(1));

        let refused = BackendSelection::refused(
            "cuda",
            Some(sample_cuda_device()),
            vec![String::from("matmul")],
            policy,
            ServedProductFallbackTrigger::NumericalSafetyRisk,
            "tensor core path failed numerical safety gate",
        );
        assert_eq!(refused.selection_state, BackendSelectionState::Refused);
        assert_eq!(
            refused.fallback_action,
            Some(ServedProductFallbackAction::Refuse)
        );
        assert_eq!(
            refused.fallback_trigger,
            Some(ServedProductFallbackTrigger::NumericalSafetyRisk)
        );
    }

    #[test]
    fn backend_selection_can_carry_cuda_backend_identity() {
        let selection = BackendSelection::direct(
            "cuda",
            Some(sample_cuda_device()),
            vec![String::from("probe_only")],
        );
        assert_eq!(selection.requested_backend, "cuda");
        assert_eq!(selection.effective_backend, "cuda");
        assert_eq!(
            selection
                .selected_device
                .as_ref()
                .map(|descriptor| descriptor.device.kind()),
            Some(DeviceKind::Cuda)
        );
        assert_eq!(selection.selection_state, BackendSelectionState::Direct);
    }

    #[test]
    fn quantization_support_surfaces_storage_path_and_pending_execution_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let support = QuantizationSupport {
            mode: psionic_core::QuantizationMode::GgmlQ4_0,
            load_path: QuantizationLoadPath::BackendQuantized,
            execution: QuantizationExecution::DequantizeToF32,
        };

        assert_eq!(
            serde_json::to_value(&support)?,
            json!({
                "mode": "ggml_q4_0",
                "load_path": "backend_quantized",
                "execution": "dequantize_to_f32"
            })
        );
        Ok(())
    }

    #[test]
    fn backend_extension_support_serializes_stably() -> Result<(), Box<dyn std::error::Error>> {
        let support = BackendExtensionSupport {
            kind: BackendExtensionKind::RotaryEmbedding,
            execution: BackendExtensionExecution::BackendSpecialized,
        };

        assert_eq!(
            serde_json::to_value(&support)?,
            json!({
                "kind": "rotary_embedding",
                "execution": "backend_specialized"
            })
        );
        Ok(())
    }

    #[test]
    fn buffer_handles_can_distinguish_quantized_storage_from_dequantized_fallback() {
        #[derive(Clone, Debug, PartialEq, Eq)]
        struct QuantizedMockBuffer {
            spec: TensorSpec,
        }

        impl BufferHandle for QuantizedMockBuffer {
            fn spec(&self) -> &TensorSpec {
                &self.spec
            }

            fn storage_kind(&self) -> BufferStorageKind {
                BufferStorageKind::QuantizedBlocks {
                    mode: psionic_core::QuantizationMode::GgmlQ8_0,
                    layout: psionic_core::QuantizedBlockLayout::new(32, 34, 2),
                    residency: BufferResidency::Backend,
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        struct DequantizedMockBuffer {
            spec: TensorSpec,
        }

        impl BufferHandle for DequantizedMockBuffer {
            fn spec(&self) -> &TensorSpec {
                &self.spec
            }

            fn storage_kind(&self) -> BufferStorageKind {
                BufferStorageKind::DequantizedF32 {
                    source_quantization: psionic_core::QuantizationMode::GgmlQ8_0,
                }
            }
        }

        let spec = TensorSpec::new(Shape::new(vec![64]), DType::F32, Device::cpu());
        let quantized = QuantizedMockBuffer { spec: spec.clone() };
        let dequantized = DequantizedMockBuffer { spec };

        assert_eq!(
            quantized.storage_kind(),
            BufferStorageKind::QuantizedBlocks {
                mode: psionic_core::QuantizationMode::GgmlQ8_0,
                layout: psionic_core::QuantizedBlockLayout::new(32, 34, 2),
                residency: BufferResidency::Backend,
            }
        );
        assert_eq!(
            dequantized.storage_kind(),
            BufferStorageKind::DequantizedF32 {
                source_quantization: psionic_core::QuantizationMode::GgmlQ8_0,
            }
        );
    }

    #[test]
    fn runtime_model_storage_truth_distinguishes_paged_blobs_from_copies()
    -> Result<(), Box<dyn std::error::Error>> {
        let copy = ModelArtifactStorage::in_memory_copy("weights.gguf", "abcd");
        let paged = ModelArtifactStorage::paged_local_blob(
            "weights.gguf",
            "abcd",
            ModelArtifactBlobKind::OllamaBlob,
            ArtifactReadPath::MemoryMapped,
            4096,
            Some(String::from("mmap preferred and available")),
        );

        assert_eq!(copy.storage_kind, ModelArtifactStorageKind::InMemoryCopy);
        assert_eq!(
            serde_json::to_value(&copy)?,
            json!({
                "artifact_name": "weights.gguf",
                "artifact_sha256": "abcd",
                "storage_kind": "in_memory_copy"
            })
        );
        assert_eq!(paged.storage_kind, ModelArtifactStorageKind::PagedLocalBlob);
        assert_eq!(
            serde_json::to_value(&paged)?,
            json!({
                "artifact_name": "weights.gguf",
                "artifact_sha256": "abcd",
                "storage_kind": "paged_local_blob",
                "blob_kind": "ollama_blob",
                "read_path": "memory_mapped",
                "page_size": 4096,
                "fallback_reason": "mmap preferred and available"
            })
        );
        Ok(())
    }

    #[test]
    fn paged_tensor_storage_plan_serializes_byte_window_and_page_counts()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = PagedTensorStoragePlan {
            tensor_name: String::from("blk.0.attn_q.weight"),
            artifact_name: String::from("weights.gguf"),
            byte_offset: 8192,
            byte_length: 16384,
            page_size: 4096,
            page_count: 4,
        };

        assert_eq!(
            serde_json::to_value(&plan)?,
            json!({
                "tensor_name": "blk.0.attn_q.weight",
                "artifact_name": "weights.gguf",
                "byte_offset": 8192,
                "byte_length": 16384,
                "page_size": 4096,
                "page_count": 4
            })
        );
        Ok(())
    }

    #[test]
    fn loaded_model_residency_tracks_keepalive_and_request_activity() {
        let mut residency = LoadedModelResidency::loading(1_000, 5_000);
        assert_eq!(residency.state, LoadedModelState::Loading);
        assert_eq!(residency.expires_at_millis, Some(6_000));

        residency.mark_ready(1_500);
        assert_eq!(residency.state, LoadedModelState::Ready);
        assert_eq!(residency.expires_at_millis, Some(6_500));

        residency.begin_request(2_000);
        assert_eq!(residency.active_requests, 1);
        assert_eq!(residency.expires_at_millis, None);

        residency.finish_request(3_000);
        assert_eq!(residency.active_requests, 0);
        assert_eq!(residency.expires_at_millis, Some(8_000));
        assert!(!residency.is_expired(7_999));
        assert!(residency.is_expired(8_000));

        residency.refresh_keep_alive(0, 8_500);
        assert_eq!(residency.expires_at_millis, Some(8_500));
        assert!(residency.is_expired(8_500));
    }

    #[test]
    fn residency_policy_and_memory_plan_serialize_stably() -> Result<(), Box<dyn std::error::Error>>
    {
        let plan = ModelMemoryPlan::split_residency(1024, 256, 128, 384, 2048);
        let policy = ModelResidencyPolicy {
            max_loaded_models: Some(2),
            memory_budget: MemoryBudget {
                resident_host_bytes: Some(4096),
                resident_device_bytes: Some(8192),
            },
            pressure_action: ResidencyPressureAction::UnloadIdleOldestFirst,
        };

        assert_eq!(
            serde_json::to_value(&(plan, policy))?,
            json!([
                {
                    "weights_bytes": 1024,
                    "kv_cache_bytes": 256,
                    "graph_bytes": 128,
                    "resident_host_bytes": 384,
                    "resident_device_bytes": 2048
                },
                {
                    "max_loaded_models": 2,
                    "memory_budget": {
                        "resident_host_bytes": 4096,
                        "resident_device_bytes": 8192
                    },
                    "pressure_action": "unload_idle_oldest_first"
                }
            ])
        );
        Ok(())
    }

    #[test]
    fn model_admission_can_evict_oldest_idle_model_to_fit_budget() {
        let loaded = vec![
            LoadedModelMemoryState {
                model_id: String::from("alpha"),
                plan: ModelMemoryPlan::host_only(256, 128, 0),
                active_requests: 0,
                last_used_at_millis: 1_000,
            },
            LoadedModelMemoryState {
                model_id: String::from("beta"),
                plan: ModelMemoryPlan::host_only(256, 128, 0),
                active_requests: 0,
                last_used_at_millis: 2_000,
            },
        ];
        let policy = ModelResidencyPolicy {
            max_loaded_models: Some(2),
            memory_budget: MemoryBudget {
                resident_host_bytes: Some(800),
                resident_device_bytes: None,
            },
            pressure_action: ResidencyPressureAction::UnloadIdleOldestFirst,
        };

        let decision = plan_model_admission(
            &loaded,
            "gamma",
            &ModelMemoryPlan::host_only(256, 128, 0),
            &policy,
        );
        assert!(decision.is_ok());
        let Ok(decision) = decision else {
            return;
        };

        assert_eq!(
            decision,
            ModelAdmissionDecision {
                current: MemoryResidencySnapshot {
                    loaded_models: 2,
                    resident_host_bytes: 768,
                    resident_device_bytes: 0,
                },
                admitted: MemoryResidencySnapshot {
                    loaded_models: 2,
                    resident_host_bytes: 768,
                    resident_device_bytes: 0,
                },
                evicted_models: vec![String::from("alpha")],
            }
        );
    }

    #[test]
    fn model_admission_refuses_when_only_active_models_block_the_budget() {
        let loaded = vec![LoadedModelMemoryState {
            model_id: String::from("alpha"),
            plan: ModelMemoryPlan::host_only(256, 128, 0),
            active_requests: 1,
            last_used_at_millis: 1_000,
        }];
        let policy = ModelResidencyPolicy {
            max_loaded_models: Some(1),
            memory_budget: MemoryBudget {
                resident_host_bytes: Some(512),
                resident_device_bytes: None,
            },
            pressure_action: ResidencyPressureAction::UnloadIdleOldestFirst,
        };

        let refusal = plan_model_admission(
            &loaded,
            "beta",
            &ModelMemoryPlan::host_only(256, 128, 0),
            &policy,
        );
        assert!(refusal.is_err());
        let Err(refusal) = refusal else {
            return;
        };

        assert_eq!(refusal.reason, AdmissionRefusalReason::MaxLoadedModels);
        assert_eq!(refusal.blocking_models, vec![String::from("alpha")]);
        assert_eq!(
            refusal.current,
            MemoryResidencySnapshot {
                loaded_models: 1,
                resident_host_bytes: 384,
                resident_device_bytes: 0,
            }
        );
    }

    #[test]
    fn kv_page_layout_reports_page_and_byte_geometry() {
        let layout = KvCachePageLayout::new(9, 4, 32);
        assert_eq!(layout.page_bytes, 128);
        assert_eq!(layout.max_pages, 3);
        assert_eq!(layout.page_count_for_tokens(0), 0);
        assert_eq!(layout.page_count_for_tokens(1), 1);
        assert_eq!(layout.page_count_for_tokens(4), 1);
        assert_eq!(layout.page_count_for_tokens(5), 2);
        assert_eq!(layout.bytes_for_tokens(3), 96);
    }

    #[test]
    fn kv_cache_state_and_growth_serialize_stably() -> Result<(), Box<dyn std::error::Error>> {
        let policy = KvCachePolicy {
            device_scope: KvCacheDeviceScope::SameDeviceOnly,
            spill_policy: KvCacheSpillPolicy::RefuseNewPages,
            page_layout: KvCachePageLayout::new(8, 4, 64),
        };
        let before = KvCacheState::paged(&policy.page_layout, 3);
        let current = KvCacheState::paged(&policy.page_layout, 6);
        let accounting = KvCacheAccounting::from_states(&before, current.clone());

        assert_eq!(
            serde_json::to_value(&policy)?,
            json!({
                "device_scope": "same_device_only",
                "spill_policy": "refuse_new_pages",
                "page_layout": {
                    "max_context_tokens": 8,
                    "tokens_per_page": 4,
                    "bytes_per_token": 64,
                    "page_bytes": 256,
                    "max_pages": 2
                }
            })
        );
        assert_eq!(
            serde_json::to_value(&accounting)?,
            json!({
                "current": {
                    "tokens": 6,
                    "bytes": 384,
                    "pages": 2
                },
                "growth": {
                    "tokens": 3,
                    "bytes": 192,
                    "pages": 1
                }
            })
        );
        Ok(())
    }

    #[test]
    fn kv_residency_accounting_serializes_stably() -> Result<(), Box<dyn std::error::Error>> {
        let policy = KvCachePolicy {
            device_scope: KvCacheDeviceScope::CrossDeviceExplicit,
            spill_policy: KvCacheSpillPolicy::SpillToHost,
            page_layout: KvCachePageLayout::new(32, 8, 64),
        };
        let accounting = KvResidencyAccounting::from_policy(&policy)
            .with_tier(
                KvResidencyTierState::resident(
                    KvResidencyTier::Host,
                    KvCacheState::paged(&policy.page_layout, 16),
                )
                .with_detail("host mirror stays hot for resumed decode"),
            )
            .with_tier(
                KvResidencyTierState::external(
                    KvResidencyTier::Distributed,
                    KvCacheState::paged(&policy.page_layout, 16),
                    KvResidencyExternalLocator::datastream(
                        "kv-cache-stream-17",
                        "manifest-digest-17",
                        "object-digest-17",
                        1024,
                    )
                    .with_detail("checkpoint-backed distributed tier"),
                )
                .with_detail("distributed checkpoint kept restorable KV pages"),
            )
            .with_movement(
                KvResidencyMovement::new(
                    KvResidencyMovementKind::Spill,
                    KvResidencyTier::Host,
                    KvResidencyTier::Distributed,
                    2,
                    1024,
                )
                .with_detail("host tier spilled cold pages into the datastream tier"),
            )
            .with_movement(
                KvResidencyMovement::new(
                    KvResidencyMovementKind::Restore,
                    KvResidencyTier::Distributed,
                    KvResidencyTier::Host,
                    2,
                    1024,
                )
                .with_detail("replayed a restorable host tier from the distributed locator"),
            );

        assert!(accounting.has_tier(KvResidencyTier::Host));
        assert!(accounting.has_tier(KvResidencyTier::Distributed));
        assert_eq!(
            serde_json::to_value(&accounting)?,
            json!({
                "device_scope": "cross_device_explicit",
                "spill_policy": "spill_to_host",
                "tiers": [
                    {
                        "tier": "host",
                        "state": {
                            "tokens": 16,
                            "bytes": 1024,
                            "pages": 2
                        },
                        "resident": true,
                        "detail": "host mirror stays hot for resumed decode"
                    },
                    {
                        "tier": "distributed",
                        "state": {
                            "tokens": 16,
                            "bytes": 1024,
                            "pages": 2
                        },
                        "resident": false,
                        "external_locator": {
                            "kind": "datastream",
                            "locator_id": "kv-cache-stream-17",
                            "locator_digest": "manifest-digest-17",
                            "object_digest": "object-digest-17",
                            "total_bytes": 1024,
                            "detail": "checkpoint-backed distributed tier"
                        },
                        "detail": "distributed checkpoint kept restorable KV pages"
                    }
                ],
                "movements": [
                    {
                        "kind": "spill",
                        "from_tier": "host",
                        "to_tier": "distributed",
                        "kv_pages": 2,
                        "kv_bytes": 1024,
                        "detail": "host tier spilled cold pages into the datastream tier"
                    },
                    {
                        "kind": "restore",
                        "from_tier": "distributed",
                        "to_tier": "host",
                        "kv_pages": 2,
                        "kv_bytes": 1024,
                        "detail": "replayed a restorable host tier from the distributed locator"
                    }
                ]
            })
        );
        Ok(())
    }

    #[test]
    fn prefix_cache_identity_and_policy_serialize_stably() -> Result<(), Box<dyn std::error::Error>>
    {
        let policy = PrefixCacheReusePolicy {
            shared_across_sessions: true,
            shared_across_users: false,
            shared_across_models: false,
            shared_across_backends: false,
            shared_across_sampler_settings: false,
        };
        let identity = PrefixCacheIdentity {
            served_artifact_digest: String::from("served-artifact-digest"),
            model_id: String::from("fixture-word-decoder-v0"),
            model_revision: String::from("v0"),
            weight_bundle_digest: String::from("bundle-digest"),
            tokenizer_family: String::from("fixture_wordpiece"),
            tokenizer_digest: Some(String::from("tokenizer-digest")),
            chat_template_digest: None,
            generation_defaults_digest: None,
            tenant_id: None,
            sampler_digest: None,
            backend_compatibility: String::from("cpu"),
            prefix_digest: String::from("prefix-digest"),
            prefix_tokens: 3,
        };

        assert_eq!(
            serde_json::to_value(&policy)?,
            json!({
                "shared_across_sessions": true,
                "shared_across_users": false,
                "shared_across_models": false,
                "shared_across_backends": false,
                "shared_across_sampler_settings": false
            })
        );
        assert_eq!(
            serde_json::to_value(&(PrefixCacheState::Hit, identity))?,
            json!([
                "hit",
                {
                    "served_artifact_digest": "served-artifact-digest",
                    "model_id": "fixture-word-decoder-v0",
                    "model_revision": "v0",
                    "weight_bundle_digest": "bundle-digest",
                    "tokenizer_family": "fixture_wordpiece",
                    "tokenizer_digest": "tokenizer-digest",
                    "backend_compatibility": "cpu",
                    "prefix_digest": "prefix-digest",
                    "prefix_tokens": 3
                }
            ])
        );
        Ok(())
    }

    #[test]
    fn served_artifact_identity_digest_changes_when_identity_changes() {
        let baseline = ServedArtifactIdentity::new(
            "fixture-word-decoder-v0",
            "v0",
            "bundle-digest",
            Some(String::from("model-blob-digest")),
            Some(String::from("tokenizer-digest")),
            Some(String::from("template-digest")),
            "defaults-digest",
            "gguf",
            QuantizationMode::GgmlQ4_0,
            BackendToolchainIdentity::new("cuda", "cuda@0.1.0", vec![]),
        );

        let changed_template = ServedArtifactIdentity::new(
            "fixture-word-decoder-v0",
            "v0",
            "bundle-digest",
            Some(String::from("model-blob-digest")),
            Some(String::from("tokenizer-digest")),
            Some(String::from("different-template-digest")),
            "defaults-digest",
            "gguf",
            QuantizationMode::GgmlQ4_0,
            BackendToolchainIdentity::new("cuda", "cuda@0.1.0", vec![]),
        );

        let changed_backend = ServedArtifactIdentity::new(
            "fixture-word-decoder-v0",
            "v0",
            "bundle-digest",
            Some(String::from("model-blob-digest")),
            Some(String::from("tokenizer-digest")),
            Some(String::from("template-digest")),
            "defaults-digest",
            "gguf",
            QuantizationMode::GgmlQ4_0,
            BackendToolchainIdentity::new("cpu", "cpu@0.1.0", vec![]),
        );

        assert_ne!(
            baseline.served_artifact_digest,
            changed_template.served_artifact_digest
        );
        assert_ne!(
            baseline.served_artifact_digest,
            changed_backend.served_artifact_digest
        );
    }

    #[test]
    fn sharded_model_manifest_validates_replicated_layer_and_tensor_topologies()
    -> Result<(), Box<dyn std::error::Error>> {
        let served_artifact = ServedArtifactIdentity::new(
            "fixture-word-decoder-v0",
            "v0",
            "bundle-digest",
            Some(String::from("model-blob-digest")),
            Some(String::from("tokenizer-digest")),
            Some(String::from("template-digest")),
            "defaults-digest",
            "gguf",
            QuantizationMode::GgmlQ4_0,
            BackendToolchainIdentity::new("cuda", "cuda@0.1.0", vec![]),
        );

        let replicated = ShardedModelManifest::new(
            "replicated-manifest",
            served_artifact.clone(),
            ShardedModelLayoutKind::Replicated,
        )
        .with_shard(ShardedModelArtifactRef::new(
            0,
            "decoder.replica0",
            "replica-digest-0",
            ExecutionPartition::Replica { replica_index: 0 },
        ))
        .with_shard(ShardedModelArtifactRef::new(
            1,
            "decoder.replica1",
            "replica-digest-1",
            ExecutionPartition::Replica { replica_index: 1 },
        ));
        let replicated_topology = ExecutionTopologyPlan {
            effective_backend: String::from("cuda"),
            kind: ExecutionTopologyKind::Replicated,
            assignments: vec![
                super::ExecutionShardAssignment {
                    shard_id: 0,
                    device: super::ExecutionDevicePlacement {
                        stable_device_id: String::from("cuda:0"),
                        topology_key: None,
                        placement_index: 0,
                    },
                    partition: ExecutionPartition::Replica { replica_index: 0 },
                },
                super::ExecutionShardAssignment {
                    shard_id: 1,
                    device: super::ExecutionDevicePlacement {
                        stable_device_id: String::from("cuda:1"),
                        topology_key: None,
                        placement_index: 1,
                    },
                    partition: ExecutionPartition::Replica { replica_index: 1 },
                },
            ],
        };
        replicated.validate_against_topology(&replicated_topology)?;

        let layer_sharded = ShardedModelManifest::new(
            "layer-manifest",
            served_artifact.clone(),
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
                end_layer: 40,
            },
        ));
        let layer_topology = ExecutionTopologyPlan {
            effective_backend: String::from("cuda"),
            kind: ExecutionTopologyKind::LayerSharded,
            assignments: vec![
                super::ExecutionShardAssignment {
                    shard_id: 0,
                    device: super::ExecutionDevicePlacement {
                        stable_device_id: String::from("cuda:0"),
                        topology_key: None,
                        placement_index: 0,
                    },
                    partition: ExecutionPartition::LayerRange {
                        start_layer: 0,
                        end_layer: 20,
                    },
                },
                super::ExecutionShardAssignment {
                    shard_id: 1,
                    device: super::ExecutionDevicePlacement {
                        stable_device_id: String::from("cuda:1"),
                        topology_key: None,
                        placement_index: 1,
                    },
                    partition: ExecutionPartition::LayerRange {
                        start_layer: 20,
                        end_layer: 40,
                    },
                },
            ],
        };
        layer_sharded.validate_against_topology(&layer_topology)?;

        let tensor_sharded = ShardedModelManifest::new(
            "tensor-manifest",
            served_artifact,
            ShardedModelLayoutKind::TensorSharded,
        )
        .with_shard(ShardedModelArtifactRef::new(
            0,
            "decoder.tensor0_32",
            "tensor-digest-0",
            ExecutionPartition::TensorRange {
                axis: 1,
                start: 0,
                end: 32,
            },
        ))
        .with_shard(ShardedModelArtifactRef::new(
            1,
            "decoder.tensor32_64",
            "tensor-digest-1",
            ExecutionPartition::TensorRange {
                axis: 1,
                start: 32,
                end: 64,
            },
        ));
        let tensor_topology = ExecutionTopologyPlan {
            effective_backend: String::from("cuda"),
            kind: ExecutionTopologyKind::TensorSharded,
            assignments: vec![
                super::ExecutionShardAssignment {
                    shard_id: 0,
                    device: super::ExecutionDevicePlacement {
                        stable_device_id: String::from("cuda:0"),
                        topology_key: None,
                        placement_index: 0,
                    },
                    partition: ExecutionPartition::TensorRange {
                        axis: 1,
                        start: 0,
                        end: 32,
                    },
                },
                super::ExecutionShardAssignment {
                    shard_id: 1,
                    device: super::ExecutionDevicePlacement {
                        stable_device_id: String::from("cuda:1"),
                        topology_key: None,
                        placement_index: 1,
                    },
                    partition: ExecutionPartition::TensorRange {
                        axis: 1,
                        start: 32,
                        end: 64,
                    },
                },
            ],
        };
        tensor_sharded.validate_against_topology(&tensor_topology)?;
        Ok(())
    }

    #[test]
    fn sharded_model_manifest_refuses_partition_mismatch() {
        let served_artifact = ServedArtifactIdentity::new(
            "fixture-word-decoder-v0",
            "v0",
            "bundle-digest",
            Some(String::from("model-blob-digest")),
            Some(String::from("tokenizer-digest")),
            Some(String::from("template-digest")),
            "defaults-digest",
            "gguf",
            QuantizationMode::GgmlQ4_0,
            BackendToolchainIdentity::new("cuda", "cuda@0.1.0", vec![]),
        );
        let manifest = ShardedModelManifest::new(
            "layer-manifest",
            served_artifact,
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
                end_layer: 40,
            },
        ));
        let mismatched_topology = ExecutionTopologyPlan {
            effective_backend: String::from("cuda"),
            kind: ExecutionTopologyKind::LayerSharded,
            assignments: vec![
                super::ExecutionShardAssignment {
                    shard_id: 0,
                    device: super::ExecutionDevicePlacement {
                        stable_device_id: String::from("cuda:0"),
                        topology_key: None,
                        placement_index: 0,
                    },
                    partition: ExecutionPartition::LayerRange {
                        start_layer: 0,
                        end_layer: 24,
                    },
                },
                super::ExecutionShardAssignment {
                    shard_id: 1,
                    device: super::ExecutionDevicePlacement {
                        stable_device_id: String::from("cuda:1"),
                        topology_key: None,
                        placement_index: 1,
                    },
                    partition: ExecutionPartition::LayerRange {
                        start_layer: 24,
                        end_layer: 40,
                    },
                },
            ],
        };

        let error = manifest
            .validate_against_topology(&mismatched_topology)
            .expect_err("partition mismatch should refuse");
        assert_eq!(
            error,
            ShardedModelManifestError::TopologyPartitionMismatch { shard_id: 0 }
        );
    }

    #[test]
    fn sampling_policy_serializes_supported_generation_controls()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy = SamplingPolicy {
            strategy: SamplingStrategy::Sample,
            temperature: Some(0.7),
            top_k: Some(32),
            top_p: Some(0.85),
            repeat_penalty: Some(1.2),
            presence_penalty: Some(0.4),
            frequency_penalty: Some(0.3),
            seed: Some(17),
        };
        let encoded = serde_json::to_value(&policy)?;

        assert_eq!(encoded["strategy"], "sample");
        assert!((encoded["temperature"].as_f64().unwrap_or_default() - 0.7).abs() < 1e-6);
        assert_eq!(encoded["top_k"], 32);
        assert!((encoded["top_p"].as_f64().unwrap_or_default() - 0.85).abs() < 1e-6);
        assert!((encoded["repeat_penalty"].as_f64().unwrap_or_default() - 1.2).abs() < 1e-6);
        assert!((encoded["presence_penalty"].as_f64().unwrap_or_default() - 0.4).abs() < 1e-6);
        assert!((encoded["frequency_penalty"].as_f64().unwrap_or_default() - 0.3).abs() < 1e-6);
        assert_eq!(encoded["seed"], 17);
        Ok(())
    }

    #[test]
    fn seeded_token_sampler_replays_draws() {
        let policy = SamplingPolicy {
            strategy: SamplingStrategy::Sample,
            temperature: Some(0.9),
            top_k: Some(3),
            top_p: Some(0.95),
            repeat_penalty: None,
            presence_penalty: None,
            frequency_penalty: None,
            seed: Some(42),
        };
        let logits = vec![3.0, 2.9, 2.8];
        let history = Vec::new();
        let mut left = TokenSampler::new(&policy);
        let mut right = TokenSampler::new(&policy);

        let mut left_draws = Vec::new();
        let mut right_draws = Vec::new();
        for _ in 0..4 {
            let left_sample = left.select_next_token(&logits, &history);
            assert!(left_sample.is_some());
            if let Some(left_sample) = left_sample {
                left_draws.push(left_sample);
            }

            let right_sample = right.select_next_token(&logits, &history);
            assert!(right_sample.is_some());
            if let Some(right_sample) = right_sample {
                right_draws.push(right_sample);
            }
        }

        assert_eq!(left_draws, right_draws);
    }

    #[test]
    fn strict_determinism_contract_refuses_missing_generator_state() {
        let contract = RuntimeDeterminismContract {
            mode: DeterminismMode::Strict,
            algorithm_policy: DeterministicAlgorithmPolicy::RequireDeterministic,
            generator: None,
        };

        assert_eq!(
            contract.validate(),
            Err(DeterminismContractError::MissingGeneratorState {
                mode: DeterminismMode::Strict,
            })
        );
    }

    #[test]
    fn runtime_determinism_contract_derives_stable_local_and_distributed_generators()
    -> Result<(), Box<dyn std::error::Error>> {
        let contract = RuntimeDeterminismContract::strict(17);

        let local_a = contract.derive_local_device_generator("cuda:1")?;
        let local_a_again = contract.derive_local_device_generator("cuda:1")?;
        let local_b = contract.derive_local_device_generator("cuda:2")?;
        assert_eq!(local_a, local_a_again);
        assert_ne!(local_a.seed, local_b.seed);
        assert_eq!(
            local_a.scope,
            GeneratorScope::LocalDevice {
                stable_device_id: String::from("cuda:1"),
            }
        );

        let rank0 = contract.derive_distributed_rank_generator("tensor_parallel", 0, 2)?;
        let rank1 = contract.derive_distributed_rank_generator("tensor_parallel", 1, 2)?;
        assert_ne!(rank0.seed, rank1.seed);
        assert_eq!(
            rank0.scope,
            GeneratorScope::DistributedRank {
                replica_group: String::from("tensor_parallel"),
                rank: 0,
                world_size: 2,
            }
        );
        assert_eq!(
            contract.derive_distributed_rank_generator("tensor_parallel", 2, 2),
            Err(DeterminismContractError::InvalidDistributedRank {
                rank: 2,
                world_size: 2,
            })
        );
        Ok(())
    }

    #[test]
    fn token_sampler_generator_state_restores_after_checkpoint()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy = SamplingPolicy {
            strategy: SamplingStrategy::Sample,
            temperature: Some(0.9),
            top_k: Some(3),
            top_p: Some(0.95),
            repeat_penalty: None,
            presence_penalty: None,
            frequency_penalty: None,
            seed: None,
        };
        let logits = vec![3.0, 2.9, 2.8];
        let history = Vec::new();
        let contract = RuntimeDeterminismContract::strict(42);
        let mut sampler = TokenSampler::from_determinism_contract(&policy, &contract)?;

        let first = sampler.select_next_token(&logits, &history);
        assert!(first.is_some());
        let checkpoint_contract = RuntimeDeterminismContract {
            generator: sampler.generator_state(),
            ..contract.clone()
        };
        let checkpoint = TrainingCheckpointReference::new(
            "train.decoder",
            "checkpoint-stream",
            "manifest-1",
            "object-1",
            "node-a",
            7,
            "cluster-state-1",
            "topology-1",
            1000,
        )
        .with_checkpoint_ref("step-7")
        .with_step(7)
        .with_durable_at_ms(1010);
        let snapshot = checkpoint_contract.checkpoint_state(checkpoint)?;
        let restored = snapshot.restore();
        let mut resumed = TokenSampler::from_determinism_contract(&policy, &restored)?;

        let original_next = sampler.select_next_token(&logits, &history);
        let restored_next = resumed.select_next_token(&logits, &history);
        assert_eq!(original_next, restored_next);
        Ok(())
    }

    #[test]
    fn sampling_penalties_honor_the_bounded_lookback_window() {
        let policy = SamplingPolicy {
            strategy: SamplingStrategy::Greedy,
            temperature: None,
            top_k: None,
            top_p: None,
            repeat_penalty: Some(1.0),
            presence_penalty: Some(0.0),
            frequency_penalty: Some(1.0),
            seed: None,
        };
        let mut logits = vec![0.0, 10.0];
        let mut history = vec![1u32; DEFAULT_PENALTY_LOOKBACK];
        history.insert(0, 0);

        apply_sampling_penalties(&mut logits, &history, &policy);

        assert_eq!(logits[0], 0.0);
        assert_eq!(logits[1], 10.0 - (DEFAULT_PENALTY_LOOKBACK as f32));
    }

    #[test]
    fn amd_backend_model_serializes_mode_topology_risk_and_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let device = DeviceDescriptor {
            backend: String::from("amd_userspace"),
            device: Device::new(
                psionic_core::DeviceKind::AmdUserspace,
                0,
                Some(String::from("amd_userspace:0")),
            ),
            device_name: Some(String::from("AMD Radeon Test")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(24 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("userspace_opt_in")],
            amd_metadata: Some(AmdDeviceMetadata {
                mode: AmdRuntimeMode::Userspace,
                topology: AmdTopologyInfo {
                    architecture: Some(String::from("gfx1100")),
                    pci_bdf: Some(String::from("0000:03:00.0")),
                    xcc_count: Some(1),
                    shader_engine_count: Some(4),
                    compute_unit_count: Some(60),
                    vram_bytes: Some(24 * 1024 * 1024 * 1024),
                    visible_vram_bytes: Some(16 * 1024 * 1024 * 1024),
                },
                risk: AmdRiskProfile {
                    level: AmdRiskLevel::Elevated,
                    requires_explicit_opt_in: true,
                    may_unbind_kernel_driver: true,
                    warnings: vec![String::from(
                        "userspace mode may require unloading or rebinding amdgpu",
                    )],
                },
                recovery: AmdRecoveryProfile {
                    driver_binding: AmdDriverBinding::UserspaceClaimed,
                    expected_actions: vec![
                        AmdRecoveryAction::ProcessRestart,
                        AmdRecoveryAction::RebindKernelDriver,
                    ],
                },
            }),
            nvidia_metadata: None,
        };
        let report = AmdBackendReport {
            mode: AmdRuntimeMode::Userspace,
            opt_in: AmdOptInStatus::Enabled,
            devices: vec![device],
            health: RuntimeHealth {
                status: HealthStatus::Degraded,
                message: String::from("amdgpu is still loaded; userspace mode not yet ready"),
            },
        };

        assert_eq!(
            serde_json::to_value(&report)?,
            json!({
                "mode": "userspace",
                "opt_in": "enabled",
                "devices": [{
                    "backend": "amd_userspace",
                    "device": {
                        "kind": "AmdUserspace",
                        "ordinal": 0,
                        "label": "amd_userspace:0"
                    },
                    "device_name": "AMD Radeon Test",
                    "supported_dtypes": ["F32"],
                    "supported_quantization": [],
                    "memory_capacity_bytes": 25769803776u64,
                    "unified_memory": false,
                    "feature_flags": ["userspace_opt_in"],
                    "amd_metadata": {
                        "mode": "userspace",
                        "topology": {
                            "architecture": "gfx1100",
                            "pci_bdf": "0000:03:00.0",
                            "xcc_count": 1,
                            "shader_engine_count": 4,
                            "compute_unit_count": 60,
                            "vram_bytes": 25769803776u64,
                            "visible_vram_bytes": 17179869184u64
                        },
                        "risk": {
                            "level": "elevated",
                            "requires_explicit_opt_in": true,
                            "may_unbind_kernel_driver": true,
                            "warnings": [
                                "userspace mode may require unloading or rebinding amdgpu"
                            ]
                        },
                        "recovery": {
                            "driver_binding": "userspace_claimed",
                            "expected_actions": ["process_restart", "rebind_kernel_driver"]
                        }
                    }
                }],
                "health": {
                    "status": "Degraded",
                    "message": "amdgpu is still loaded; userspace mode not yet ready"
                }
            })
        );
        Ok(())
    }

    #[test]
    fn nvidia_backend_model_serializes_topology_risk_and_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let device = DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(
                psionic_core::DeviceKind::Cuda,
                0,
                Some(String::from("cuda:0")),
            ),
            device_name: Some(String::from("NVIDIA GeForce RTX 4080")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(16 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: Some(NvidiaDeviceMetadata {
                topology: NvidiaTopologyInfo {
                    architecture: Some(String::from("ada")),
                    compute_capability: Some(String::from("8.9")),
                    pci_bdf: Some(String::from("00000000:01:00.0")),
                    sm_count: Some(76),
                    vram_bytes: Some(16 * 1024 * 1024 * 1024),
                    mig_profile: None,
                },
                risk: NvidiaRiskProfile {
                    level: NvidiaRiskLevel::Elevated,
                    display_attached: Some(true),
                    mig_partitioned: false,
                    warnings: vec![String::from(
                        "display-attached NVIDIA devices may show variable latency under local desktop load",
                    )],
                },
                recovery: NvidiaRecoveryProfile {
                    supports_gpu_reset: Some(true),
                    expected_actions: vec![
                        NvidiaRecoveryAction::ProcessRestart,
                        NvidiaRecoveryAction::GpuReset,
                        NvidiaRecoveryAction::RebootHost,
                    ],
                },
            }),
        };
        let report = NvidiaBackendReport {
            devices: vec![device],
            health: RuntimeHealth {
                status: HealthStatus::Degraded,
                message: String::from(
                    "cuda detected a display-attached GPU; provider execution should keep latency caveats explicit",
                ),
            },
        };

        assert_eq!(
            serde_json::to_value(&report)?,
            json!({
                "devices": [{
                    "backend": "cuda",
                    "device": {
                        "kind": "Cuda",
                        "ordinal": 0,
                        "label": "cuda:0"
                    },
                    "device_name": "NVIDIA GeForce RTX 4080",
                    "supported_dtypes": ["F32"],
                    "supported_quantization": [],
                    "memory_capacity_bytes": 17179869184u64,
                    "unified_memory": false,
                    "feature_flags": ["cuda_architecture_surface"],
                    "nvidia_metadata": {
                        "topology": {
                            "architecture": "ada",
                            "compute_capability": "8.9",
                            "pci_bdf": "00000000:01:00.0",
                            "sm_count": 76,
                            "vram_bytes": 17179869184u64,
                            "mig_profile": null
                        },
                        "risk": {
                            "level": "elevated",
                            "display_attached": true,
                            "mig_partitioned": false,
                            "warnings": [
                                "display-attached NVIDIA devices may show variable latency under local desktop load"
                            ]
                        },
                        "recovery": {
                            "supports_gpu_reset": true,
                            "expected_actions": ["process_restart", "gpu_reset", "reboot_host"]
                        }
                    }
                }],
                "health": {
                    "status": "Degraded",
                    "message": "cuda detected a display-attached GPU; provider execution should keep latency caveats explicit"
                }
            })
        );
        Ok(())
    }

    #[test]
    fn backend_runtime_resources_serialize_stably() -> Result<(), Box<dyn std::error::Error>> {
        let selection = BackendSelection::direct("metal", None, vec![String::from("matmul")])
            .with_runtime_resources(Some(BackendRuntimeResources {
                execution_plan_cache: ExecutionPlanCacheReport {
                    policy: ExecutionPlanCachePolicy::bounded(4, Some(256)),
                    state: ExecutionPlanCacheState {
                        cached_entries: 1,
                        cached_bytes: 96,
                    },
                },
                allocator_pool: AllocatorPoolReport {
                    policy: AllocatorPoolPolicy::exact_tensor_spec(8, 1024),
                    state: AllocatorPoolState {
                        cached_buffers: 2,
                        cached_bytes: 256,
                    },
                },
                kernel_cache: KernelCacheReport {
                    policy: KernelCachePolicy::bounded(1, Some(128)),
                    state: KernelCacheState {
                        cached_entries: 1,
                        cached_bytes: 64,
                    },
                },
                device_memory_budget: Some(DeviceMemoryBudget::new(Some(4096), 1024, 128)),
            }));

        let value = serde_json::to_value(selection)?;
        assert_eq!(
            value["runtime_resources"]["execution_plan_cache"]["state"]["cached_entries"],
            json!(1)
        );
        assert_eq!(
            value["runtime_resources"]["allocator_pool"]["policy"]["max_cached_buffers"],
            json!(8)
        );
        assert_eq!(
            value["runtime_resources"]["execution_plan_cache"]["state"]["cached_bytes"],
            json!(96)
        );
        assert_eq!(
            value["runtime_resources"]["kernel_cache"]["state"]["cached_entries"],
            json!(1)
        );
        assert_eq!(
            value["runtime_resources"]["device_memory_budget"]["available_execution_bytes"],
            json!(2944)
        );
        Ok(())
    }

    #[test]
    fn backend_selection_selected_device_inventory_uses_runtime_budget_bytes() {
        let selection = BackendSelection::direct(
            "cuda",
            Some(sample_cuda_device()),
            vec![String::from("matmul")],
        )
        .with_runtime_resources(Some(BackendRuntimeResources {
            execution_plan_cache: ExecutionPlanCacheReport {
                policy: ExecutionPlanCachePolicy::disabled(),
                state: ExecutionPlanCacheState::default(),
            },
            allocator_pool: AllocatorPoolReport {
                policy: AllocatorPoolPolicy::disabled(),
                state: AllocatorPoolState {
                    cached_buffers: 0,
                    cached_bytes: 0,
                },
            },
            kernel_cache: KernelCacheReport {
                policy: KernelCachePolicy::disabled(),
                state: KernelCacheState {
                    cached_entries: 0,
                    cached_bytes: 0,
                },
            },
            device_memory_budget: Some(DeviceMemoryBudget::new(
                Some(16 * 1024 * 1024 * 1024),
                4 * 1024 * 1024 * 1024,
                1024,
            )),
        }));

        assert_eq!(
            selection.selected_device_inventory(),
            Some(DeviceInventoryQualifiers {
                stable_device_id: String::from("00000000:01:00.0"),
                topology_key: Some(String::from("00000000:01:00.0")),
                performance_class: DevicePerformanceClass::DiscreteAccelerator,
                memory_class: DeviceMemoryClass::DedicatedDevice,
                total_memory_bytes: Some(16 * 1024 * 1024 * 1024),
                free_memory_bytes: Some(
                    (16 * 1024 * 1024 * 1024) - (4 * 1024 * 1024 * 1024) - 1024
                ),
            })
        );
    }

    #[test]
    fn backend_selection_can_surface_multi_device_topology_truth() {
        let first = sample_cuda_device();
        let mut second = sample_cuda_device();
        second.device = Device::new(DeviceKind::Cuda, 1, Some(String::from("cuda:1")));
        second.device_name = Some(String::from("NVIDIA CUDA Test Device 1"));
        let metadata = second
            .nvidia_metadata
            .as_mut()
            .expect("sample cuda metadata");
        metadata.topology.pci_bdf = Some(String::from("00000000:02:00.0"));

        let selection =
            BackendSelection::direct("cuda", Some(first.clone()), vec![String::from("matmul")])
                .with_selected_devices(vec![first.clone(), second.clone()])
                .with_execution_topology(Some(ExecutionTopologyPlan::layer_sharded(
                    "cuda",
                    vec![
                        (first.inventory_qualifiers(), 0, 20),
                        (second.inventory_qualifiers(), 20, 40),
                    ],
                )));

        assert_eq!(selection.selected_devices().len(), 2);
        assert_eq!(selection.selected_devices_inventory().len(), 2);
        assert_eq!(
            selection
                .execution_topology_plan()
                .as_ref()
                .map(|plan| plan.kind),
            Some(ExecutionTopologyKind::LayerSharded)
        );
        assert_eq!(
            selection
                .execution_topology_plan()
                .as_ref()
                .map(|plan| plan.assignments.len()),
            Some(2)
        );
    }

    #[test]
    fn backend_selection_can_publish_declared_cluster_capability_profile_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let capability_profile = ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![
                ClusterExecutionLane::RemoteWholeRequest,
                ClusterExecutionLane::ReplicaRouted,
            ])
            .with_detail(
                "backend `cuda` declares remote whole-request and replica-routed cluster support",
            );
        let selection = BackendSelection::direct(
            "cuda",
            Some(sample_cuda_device()),
            vec![String::from("matmul")],
        )
        .with_cluster_execution_capability_profile(capability_profile.clone());

        assert_eq!(
            selection.cluster_execution_capability_profile,
            Some(capability_profile.clone())
        );

        let value = serde_json::to_value(&selection)?;
        assert_eq!(
            value["cluster_execution_capability_profile"]["runtime_backend"],
            json!("cuda")
        );
        assert_eq!(
            value["cluster_execution_capability_profile"]["supported_lanes"],
            json!(["remote_whole_request", "replica_routed"])
        );
        assert_eq!(
            value["cluster_execution_capability_profile"]["supported_communication_classes"],
            json!(["remote_dispatch", "replica_routing"])
        );

        let decoded: BackendSelection = serde_json::from_value(value)?;
        assert_eq!(decoded, selection);
        Ok(())
    }

    #[test]
    fn backend_selection_can_publish_cluster_compute_market_trust_assessment_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let trust_assessment = ClusterComputeMarketTrustAssessment {
            posture: ClusterTrustPosture::TrustedLanSharedAdmission,
            discovery_posture: ClusterDiscoveryPosture::TrustedLanSeedPeers,
            trust_policy_digest: String::from("trust-policy-digest"),
            disposition: ClusterComputeMarketTrustDisposition::Refused,
            refusal_reasons: vec![
                ClusterComputeMarketTrustRefusalReason::TrustedLanSharedAdmissionOnly,
                ClusterComputeMarketTrustRefusalReason::MissingAuthenticatedTransport,
            ],
        };
        let selection = BackendSelection::direct(
            "cuda",
            Some(sample_cuda_device()),
            vec![String::from("matmul")],
        )
        .with_cluster_compute_market_trust_assessment(trust_assessment.clone());

        assert_eq!(
            selection.cluster_compute_market_trust_assessment,
            Some(trust_assessment.clone())
        );

        let value = serde_json::to_value(&selection)?;
        assert_eq!(
            value["cluster_compute_market_trust_assessment"]["posture"],
            json!("trusted_lan_shared_admission")
        );
        assert_eq!(
            value["cluster_compute_market_trust_assessment"]["disposition"],
            json!("refused")
        );
        assert_eq!(
            value["cluster_compute_market_trust_assessment"]["refusal_reasons"],
            json!([
                "trusted_lan_shared_admission_only",
                "missing_authenticated_transport"
            ])
        );

        let decoded: BackendSelection = serde_json::from_value(value)?;
        assert_eq!(decoded, selection);
        Ok(())
    }

    #[test]
    fn delivered_execution_context_can_carry_cluster_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let device = sample_cuda_device().inventory_qualifiers();
        let capability_profile = ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![
                ClusterExecutionLane::RemoteWholeRequest,
                ClusterExecutionLane::ReplicaRouted,
            ])
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
                .invalidates_on_route_change()
                .with_detail("cache reuse only remains truthful when routing stays pinned"),
            )
            .with_detail(
                "backend `cuda` declares whole-request remote dispatch on ready cluster nodes",
            );
        let cluster_execution = ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::TrustedLanDatagram,
            ClusterExecutionDisposition::RemoteWholeRequest,
        )
        .with_communication_eligibility(
            capability_profile
                .lane_communication_eligibility(ClusterExecutionLane::RemoteWholeRequest),
        )
        .with_artifact_residency_digest("artifact-residency-digest")
        .with_commit_authority(ClusterCommitAuthorityEvidence::new(
            "coordinator-a",
            7,
            41,
            "authority-fence-token",
            "authority-digest",
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Authority,
            "authority-digest",
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Placement,
            "placement-policy-digest",
        ))
        .with_command_provenance(vec![
            ClusterCommandProvenanceEvidence::new(
                ClusterAdmissionFactKind::SchedulerMembership,
                "scheduler-node",
                ClusterCommandAuthorityScopeEvidence::SelfNode,
                "scheduler-membership-command",
                "scheduler-membership-auth",
                "command-authorization-policy",
            )
            .with_target_node_id("scheduler-node"),
            ClusterCommandProvenanceEvidence::new(
                ClusterAdmissionFactKind::Leadership,
                "coordinator-a",
                ClusterCommandAuthorityScopeEvidence::ProposedLeader,
                "leadership-command",
                "leadership-auth",
                "command-authorization-policy",
            ),
        ])
        .with_selected_nodes(vec![
            ClusterSelectedNode::new("worker-a", "cuda")
                .with_role("worker")
                .with_topology_digest("node-topology-digest")
                .with_served_artifact_digest("served-artifact-digest")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
        ])
        .with_serving_semantics(
            ClusterServingSemantics::new(
                ClusterExecutionLane::ReplicaRouted,
                ExecutionCapabilityProfile::single_request_latency_optimized(),
                ClusterWarmRoutePosture::RoutePinned,
            )
            .with_detail(
                "replica-routed serving kept the request pinned to one warm replica identity",
            ),
        )
        .with_clustered_cache_usage(
            ClusterCacheUsage::new(
                ClusterExecutionLane::ReplicaRouted,
                ClusterCacheScope::ReplicaLocal,
                ClusterCacheScope::ReplicaLocal,
                CacheAction::Reuse,
                CacheAction::Reuse,
            )
            .with_invalidation_trigger(CacheInvalidationTrigger::ClusterRouteChange)
            .with_detail("request replay stayed pinned to the same warm replica"),
        )
        .with_fallback(
            ClusterFallbackStep::new("worker-a", ClusterFallbackReason::BackendDegraded)
                .from_node("worker-b")
                .with_detail("rerouted after health downgrade"),
        )
        .with_degraded_reason("healthy replica substituted after backend degradation");
        let delivered = DeliveredExecutionContext::new(
            "cuda",
            Some(ExecutionTopologyPlan::single_device("cuda", device.clone())),
            vec![device],
        )
        .with_cluster_execution(cluster_execution.clone());

        let encoded = serde_json::to_value(&delivered)?;
        assert_eq!(
            encoded["cluster_execution"]["cluster_state_digest"],
            json!("cluster-state-digest")
        );
        assert_eq!(
            encoded["cluster_execution"]["commit_authority"]["fence_token"],
            json!("authority-fence-token")
        );
        assert_eq!(
            encoded["cluster_execution"]["selected_nodes"][0]["artifact_residency"],
            json!("resident")
        );
        assert_eq!(
            encoded["cluster_execution"]["communication_eligibility"]["required_class"],
            json!("remote_dispatch")
        );
        assert_eq!(
            encoded["cluster_execution"]["clustered_cache_usage"]["prefix_scope"],
            json!("replica_local")
        );
        assert_eq!(
            encoded["cluster_execution"]["clustered_cache_usage"]["invalidation_trigger"],
            json!("cluster_route_change")
        );
        assert_eq!(
            encoded["cluster_execution"]["serving_semantics"]["lane"],
            json!("replica_routed")
        );
        assert_eq!(
            encoded["cluster_execution"]["serving_semantics"]["warm_route_posture"],
            json!("route_pinned")
        );
        assert!(
            encoded["cluster_execution"]["communication_eligibility"]["capability_profile_digest"]
                .as_str()
                .is_some()
        );
        assert_eq!(
            encoded["cluster_execution"]["command_provenance"][0]["fact_kind"],
            json!("scheduler_membership")
        );
        assert_eq!(
            encoded["cluster_execution"]["command_provenance"][1]["authority_scope"],
            json!("proposed_leader")
        );
        assert_eq!(
            serde_json::from_value::<DeliveredExecutionContext>(encoded)?,
            delivered
        );
        assert_eq!(delivered.cluster_execution, Some(cluster_execution));
        Ok(())
    }

    #[test]
    fn delivered_execution_context_can_carry_training_recovery_context()
    -> Result<(), Box<dyn std::error::Error>> {
        let training_recovery = TrainingRecoveryContext::new(
            TrainingRecoveryPosture::ElasticReconfiguration,
            TrainingCheckpointAvailability::Durable,
            TrainingElasticMembershipContext::new(
                4,
                "cluster-state-digest",
                "topology-digest",
                vec![String::from("worker-a"), String::from("worker-b")],
            )
            .with_joining_node_ids(vec![String::from("worker-c")]),
        )
        .with_latest_checkpoint(
            TrainingCheckpointReference::new(
                "train.decoder",
                "checkpoint-stream",
                "manifest-digest",
                "object-digest",
                "worker-a",
                4,
                "cluster-state-digest",
                "topology-digest",
                1_000,
            )
            .with_checkpoint_ref("step-32")
            .with_step(32)
            .with_durable_at_ms(1_250),
        )
        .with_recovering_node_ids(vec![String::from("worker-b")])
        .with_late_joiner_node_ids(vec![String::from("worker-c")])
        .with_requested_at_ms(1_500)
        .with_detail("checkpoint-backed recovery is widening the world size");
        let cluster_execution = ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::TrustedLanDatagram,
            ClusterExecutionDisposition::RemoteWholeRequest,
        )
        .with_training_recovery(training_recovery.clone());
        let delivered = DeliveredExecutionContext::new("cuda", None, Vec::new())
            .with_cluster_execution(cluster_execution.clone());

        let encoded = serde_json::to_value(&delivered)?;
        assert_eq!(
            encoded["cluster_execution"]["training_recovery"]["posture"],
            json!("elastic_reconfiguration")
        );
        assert_eq!(
            encoded["cluster_execution"]["training_recovery"]["latest_checkpoint"]["checkpoint_family"],
            json!("train.decoder")
        );
        assert_eq!(
            encoded["cluster_execution"]["training_recovery"]["late_joiner_node_ids"][0],
            json!("worker-c")
        );
        assert_eq!(
            encoded["cluster_execution"]["training_recovery"]["elastic_membership"]["membership_epoch"],
            json!(4)
        );
        assert_eq!(
            serde_json::from_value::<DeliveredExecutionContext>(encoded)?,
            delivered
        );
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .and_then(|value| value.training_recovery.clone()),
            Some(training_recovery)
        );
        Ok(())
    }

    #[test]
    fn delivered_execution_context_can_carry_training_collective_context()
    -> Result<(), Box<dyn std::error::Error>> {
        let membership = TrainingElasticMembershipContext::new(
            7,
            "cluster-state-digest",
            "topology-digest",
            vec![String::from("worker-a"), String::from("worker-b")],
        )
        .with_joining_node_ids(vec![String::from("worker-c")]);
        let collective = TrainingCollectiveContext::new(
            TrainingDeviceMeshContext::new(
                "mesh-train",
                3,
                "cuda",
                ClusterCommunicationClass::TensorCollectiveMesh,
                membership,
                vec![String::from("worker-a"), String::from("worker-b")],
            )
            .with_axes(vec![
                TrainingDeviceMeshAxis::new("dp", TrainingDeviceMeshAxisKind::DataParallel, 1),
                TrainingDeviceMeshAxis::new("tp", TrainingDeviceMeshAxisKind::TensorParallel, 2)
                    .with_collective_group_size(2),
            ]),
            TrainingCollectiveKind::AllReduce,
            TrainingCollectiveQuantization::Int8Symmetric,
            16 * 1024 * 1024,
            4 * 1024 * 1024,
            2,
        )
        .with_benchmark("collective-benchmark", 2_400, 55)
        .with_detail("int8 all-reduce is benchmark-approved for this tensor-parallel mesh");
        let cluster_execution = ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::TrustedLanDatagram,
            ClusterExecutionDisposition::Sharded,
        )
        .with_training_collective(collective.clone());
        let delivered = DeliveredExecutionContext::new("cuda", None, Vec::new())
            .with_cluster_execution(cluster_execution.clone());

        let encoded = serde_json::to_value(&delivered)?;
        assert_eq!(
            encoded["cluster_execution"]["training_collective"]["kind"],
            json!("all_reduce")
        );
        assert_eq!(
            encoded["cluster_execution"]["training_collective"]["quantization"],
            json!("int8_symmetric")
        );
        assert_eq!(
            encoded["cluster_execution"]["training_collective"]["device_mesh"]["mesh_revision"],
            json!(3)
        );
        assert_eq!(
            encoded["cluster_execution"]["training_collective"]["device_mesh"]["axes"][1]["kind"],
            json!("tensor_parallel")
        );
        assert_eq!(
            serde_json::from_value::<DeliveredExecutionContext>(encoded)?,
            delivered
        );
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .and_then(|value| value.training_collective.clone()),
            Some(collective)
        );
        Ok(())
    }

    #[test]
    fn cluster_execution_capability_profile_round_trips_with_stable_digest() {
        let profile = ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![
                ClusterExecutionLane::RemoteWholeRequest,
                ClusterExecutionLane::ReplicaRouted,
                ClusterExecutionLane::LayerSharded,
                ClusterExecutionLane::TensorSharded,
            ])
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
                .invalidates_on_route_change()
                .with_detail("replica-routed reuse remains truthful only on one warm replica"),
            )
            .with_detail("cluster-capable CUDA backend profile");
        let encoded = match serde_json::to_string(&profile) {
            Ok(value) => value,
            Err(error) => panic!("cluster capability profile should encode: {error}"),
        };
        let encoded_again = match serde_json::to_string(&profile) {
            Ok(value) => value,
            Err(error) => panic!("cluster capability profile should encode repeatably: {error}"),
        };
        let decoded: ClusterExecutionCapabilityProfile = match serde_json::from_str(&encoded) {
            Ok(value) => value,
            Err(error) => panic!("cluster capability profile should decode: {error}"),
        };
        assert_eq!(encoded, encoded_again);
        assert_eq!(decoded, profile);
        assert_eq!(decoded.stable_digest(), profile.stable_digest());
        assert!(profile.supports_lane(ClusterExecutionLane::LayerSharded));
        assert_eq!(
            profile
                .clustered_cache_capability(ClusterExecutionLane::ReplicaRouted)
                .map(|capability| capability.prefix_scope),
            Some(ClusterCacheScope::ReplicaLocal)
        );
        assert_eq!(
            profile
                .serving_semantics_capability(ClusterExecutionLane::ReplicaRouted)
                .map(|capability| capability.warm_route_posture),
            Some(ClusterWarmRoutePosture::RoutePinned)
        );
        assert!(profile.supports_communication_class(ClusterCommunicationClass::LayerShardHandoff));
        assert!(
            profile.supports_communication_class(ClusterCommunicationClass::TensorCollectiveMesh)
        );
    }

    #[test]
    fn cluster_execution_capability_profile_digest_changes_when_lane_support_changes() {
        let replicated_only = ClusterExecutionCapabilityProfile::new("cpu")
            .with_supported_lanes(vec![
                ClusterExecutionLane::RemoteWholeRequest,
                ClusterExecutionLane::ReplicaRouted,
            ])
            .with_detail("cpu supports dispatch and replica routing only");
        let layer_sharded = ClusterExecutionCapabilityProfile::new("cpu")
            .with_supported_lanes(vec![
                ClusterExecutionLane::RemoteWholeRequest,
                ClusterExecutionLane::ReplicaRouted,
                ClusterExecutionLane::LayerSharded,
            ])
            .with_detail("cpu profile changed");
        assert_ne!(
            replicated_only.stable_digest(),
            layer_sharded.stable_digest()
        );
        assert!(!replicated_only.supports_lane(ClusterExecutionLane::LayerSharded));
        assert!(layer_sharded.supports_lane(ClusterExecutionLane::LayerSharded));
    }

    #[test]
    fn cluster_execution_capability_profile_digest_changes_when_cache_truth_changes() {
        let replica_local = ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![ClusterExecutionLane::ReplicaRouted])
            .with_clustered_cache_capability(ClusterCacheCapability::new(
                ClusterExecutionLane::ReplicaRouted,
                ClusterCacheScope::ReplicaLocal,
                ClusterCacheScope::ReplicaLocal,
            ));
        let route_invalidating = ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![ClusterExecutionLane::ReplicaRouted])
            .with_clustered_cache_capability(
                ClusterCacheCapability::new(
                    ClusterExecutionLane::ReplicaRouted,
                    ClusterCacheScope::ReplicaLocal,
                    ClusterCacheScope::ReplicaLocal,
                )
                .invalidates_on_route_change(),
            );

        assert_ne!(
            replica_local.stable_digest(),
            route_invalidating.stable_digest()
        );
    }

    #[test]
    fn cluster_execution_capability_profile_digest_changes_when_serving_semantics_change() {
        let ready_node_selection = ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![ClusterExecutionLane::RemoteWholeRequest])
            .with_serving_semantics_capability(ClusterServingSemantics::new(
                ClusterExecutionLane::RemoteWholeRequest,
                ExecutionCapabilityProfile::single_request_latency_optimized(),
                ClusterWarmRoutePosture::ReadyNodeSelection,
            ));
        let route_pinned = ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![ClusterExecutionLane::RemoteWholeRequest])
            .with_serving_semantics_capability(ClusterServingSemantics::new(
                ClusterExecutionLane::RemoteWholeRequest,
                ExecutionCapabilityProfile::single_request_latency_optimized(),
                ClusterWarmRoutePosture::RoutePinned,
            ));

        assert_ne!(
            ready_node_selection.stable_digest(),
            route_pinned.stable_digest()
        );
    }

    #[test]
    fn cluster_execution_capability_profile_digest_changes_when_residency_truth_changes() {
        let host_only = ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![ClusterExecutionLane::ReplicaRouted])
            .with_clustered_cache_capability(
                ClusterCacheCapability::new(
                    ClusterExecutionLane::ReplicaRouted,
                    ClusterCacheScope::ReplicaLocal,
                    ClusterCacheScope::ReplicaLocal,
                )
                .with_residency_tier(KvResidencyTier::Host),
            );
        let host_and_device = ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![ClusterExecutionLane::ReplicaRouted])
            .with_clustered_cache_capability(
                ClusterCacheCapability::new(
                    ClusterExecutionLane::ReplicaRouted,
                    ClusterCacheScope::ReplicaLocal,
                    ClusterCacheScope::ReplicaLocal,
                )
                .with_residency_tiers(vec![KvResidencyTier::Host, KvResidencyTier::Device]),
            );

        assert_ne!(host_only.stable_digest(), host_and_device.stable_digest());
        assert_eq!(
            host_and_device
                .clustered_cache_capability(ClusterExecutionLane::ReplicaRouted)
                .map(|capability| capability.supported_residency_tiers.clone()),
            Some(vec![KvResidencyTier::Device, KvResidencyTier::Host])
        );
    }

    #[test]
    fn cluster_compute_market_trust_assessment_round_trips_with_stable_digest() {
        let assessment = ClusterComputeMarketTrustAssessment {
            posture: ClusterTrustPosture::AttestedConfiguredPeers,
            discovery_posture: ClusterDiscoveryPosture::ExplicitWiderNetworkRequested,
            trust_policy_digest: String::from("trust-policy-digest"),
            disposition: ClusterComputeMarketTrustDisposition::Eligible,
            refusal_reasons: Vec::new(),
        };
        let encoded = match serde_json::to_string(&assessment) {
            Ok(value) => value,
            Err(error) => panic!("cluster trust assessment should encode: {error}"),
        };
        let encoded_again = match serde_json::to_string(&assessment) {
            Ok(value) => value,
            Err(error) => {
                panic!("cluster trust assessment should encode repeatably: {error}")
            }
        };
        let decoded: ClusterComputeMarketTrustAssessment = match serde_json::from_str(&encoded) {
            Ok(value) => value,
            Err(error) => panic!("cluster trust assessment should decode: {error}"),
        };

        assert_eq!(encoded, encoded_again);
        assert_eq!(decoded, assessment);
        assert_eq!(decoded.stable_digest(), assessment.stable_digest());
    }

    #[test]
    fn cluster_compute_market_trust_assessment_digest_changes_with_refusal_shape() {
        let trusted_lan = ClusterComputeMarketTrustAssessment {
            posture: ClusterTrustPosture::TrustedLanSharedAdmission,
            discovery_posture: ClusterDiscoveryPosture::TrustedLanSeedPeers,
            trust_policy_digest: String::from("trust-policy-a"),
            disposition: ClusterComputeMarketTrustDisposition::Refused,
            refusal_reasons: vec![
                ClusterComputeMarketTrustRefusalReason::TrustedLanSharedAdmissionOnly,
                ClusterComputeMarketTrustRefusalReason::MissingAuthenticatedTransport,
            ],
        };
        let attested = ClusterComputeMarketTrustAssessment {
            posture: ClusterTrustPosture::AttestedConfiguredPeers,
            discovery_posture: ClusterDiscoveryPosture::OperatorManagedConfiguredPeers,
            trust_policy_digest: String::from("trust-policy-a"),
            disposition: ClusterComputeMarketTrustDisposition::Refused,
            refusal_reasons: vec![
                ClusterComputeMarketTrustRefusalReason::MissingNonLanDiscoveryPosture,
            ],
        };

        assert_ne!(trusted_lan.stable_digest(), attested.stable_digest());
    }

    #[test]
    fn communication_eligibility_can_be_derived_from_capability_profile() {
        let profile = ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![
                ClusterExecutionLane::RemoteWholeRequest,
                ClusterExecutionLane::ReplicaRouted,
                ClusterExecutionLane::LayerSharded,
            ])
            .with_detail("declared CUDA profile");

        let layer_eligibility = ClusterCommunicationEligibility::from_capability_profile(
            &profile,
            ClusterCommunicationClass::LayerShardHandoff,
        );
        let tensor_eligibility = ClusterCommunicationEligibility::from_capability_profile(
            &profile,
            ClusterCommunicationClass::TensorCollectiveMesh,
        );

        assert!(layer_eligibility.eligible);
        assert_eq!(
            layer_eligibility.supported_classes,
            vec![
                ClusterCommunicationClass::RemoteDispatch,
                ClusterCommunicationClass::ReplicaRouting,
                ClusterCommunicationClass::LayerShardHandoff,
            ]
        );
        assert_eq!(
            layer_eligibility.detail.as_deref(),
            Some("declared CUDA profile")
        );
        assert_eq!(
            layer_eligibility.capability_profile_digest.as_deref(),
            Some(profile.stable_digest().as_str())
        );
        assert!(!tensor_eligibility.eligible);
        assert_eq!(
            tensor_eligibility.required_class,
            ClusterCommunicationClass::TensorCollectiveMesh
        );
    }

    #[test]
    fn lane_communication_eligibility_refuses_undeclared_lane_even_when_profile_exists() {
        let profile = ClusterExecutionCapabilityProfile::new("cpu")
            .with_supported_lanes(vec![ClusterExecutionLane::RemoteWholeRequest])
            .with_detail("cpu does not declare replica routing");
        let eligibility = ClusterCommunicationEligibility::from_capability_profile_lane(
            &profile,
            ClusterExecutionLane::ReplicaRouted,
        );

        assert!(!eligibility.eligible);
        assert_eq!(
            eligibility.required_class,
            ClusterCommunicationClass::ReplicaRouting
        );
        assert_eq!(
            eligibility.capability_profile_digest.as_deref(),
            Some(profile.stable_digest().as_str())
        );
    }

    #[test]
    fn signed_cluster_evidence_bundle_round_trips_and_verifies()
    -> Result<(), Box<dyn std::error::Error>> {
        let cluster_execution = ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::TrustedLanDatagram,
            ClusterExecutionDisposition::RemoteWholeRequest,
        )
        .with_commit_authority(ClusterCommitAuthorityEvidence::new(
            "coordinator-a",
            7,
            41,
            "authority-fence-token",
            "authority-digest",
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Authority,
            "authority-digest",
        ))
        .with_command_provenance(vec![ClusterCommandProvenanceEvidence::new(
            ClusterAdmissionFactKind::SchedulerMembership,
            "scheduler-node",
            ClusterCommandAuthorityScopeEvidence::SelfNode,
            "scheduler-membership-command",
            "scheduler-membership-auth",
            "command-authorization-policy",
        )])
        .with_selected_nodes(vec![
            ClusterSelectedNode::new("worker-a", "cuda")
                .with_role("worker")
                .with_served_artifact_digest("served-artifact-digest")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
        ]);
        let settlement_linkage = SettlementLinkageInput {
            request_digest: String::from("request-digest"),
            product_id: String::from("text_generation"),
            model_id: String::from("fixture-decoder-v0"),
            served_artifact_digest: String::from("served-artifact-digest"),
            execution_plan_digest: String::from("execution-plan-digest"),
            runtime_backend: String::from("cuda"),
            kernel_count: 4,
            bytes_moved: 1024,
            plan_cache_hits: 1,
            plan_cache_misses: 0,
            kv_growth: None,
            output_tokens: Some(2),
            cluster_provenance: ClusterSettlementProvenanceInput::from_cluster_execution(
                &cluster_execution,
            ),
        };
        let payload = ClusterEvidenceBundlePayload::new(
            "text_generation",
            "request-1",
            "request-digest",
            "fixture-decoder-v0",
            "v0",
            "cuda",
            "served-artifact-digest",
            "weight-bundle-digest",
            ClusterEvidenceBundleStatus::Succeeded,
            cluster_execution,
        )
        .with_delivery_proof(ExecutionDeliveryProof {
            execution_plan_digest: String::from("execution-plan-digest"),
            kernel_count: 4,
            bytes_moved: 1024,
            plan_cache_hits: 1,
            plan_cache_misses: 0,
            kv_growth: None,
            prefill_decode_handoff: None,
            kv_residency: None,
        })
        .with_settlement_linkage(settlement_linkage);
        let signing_key = SigningKey::from_bytes(&[17; 32]);

        let bundle = payload.clone().sign("scheduler-node", &signing_key);

        assert_eq!(bundle.bundle_digest, payload.stable_digest());
        assert!(bundle.verify().is_ok(), "bundle should verify");
        let encoded = serde_json::to_value(&bundle)?;
        assert_eq!(
            encoded["signature"]["signer_node_id"],
            json!("scheduler-node")
        );
        assert_eq!(
            encoded["payload"]["cluster_execution"]["commit_authority"]["authority_digest"],
            json!("authority-digest")
        );
        assert_eq!(
            serde_json::from_value::<SignedClusterEvidenceBundle>(encoded)?,
            bundle
        );
        Ok(())
    }

    #[test]
    fn signed_cluster_evidence_bundle_refuses_tampered_payload() {
        let payload = ClusterEvidenceBundlePayload::new(
            "text_generation",
            "request-1",
            "request-digest",
            "fixture-decoder-v0",
            "v0",
            "cuda",
            "served-artifact-digest",
            "weight-bundle-digest",
            ClusterEvidenceBundleStatus::Succeeded,
            ClusterExecutionContext::new(
                "cluster-alpha",
                "cluster-state-digest",
                "cluster-topology-digest",
                "scheduler-node",
                ClusterTransportClass::TrustedLanDatagram,
                ClusterExecutionDisposition::RemoteWholeRequest,
            ),
        );
        let signing_key = SigningKey::from_bytes(&[19; 32]);
        let mut bundle = payload.sign("scheduler-node", &signing_key);
        bundle.payload.request_digest = String::from("tampered-request-digest");

        let verification = bundle.verify();

        assert!(
            matches!(
                verification,
                Err(ClusterEvidenceBundleVerificationError::DigestMismatch { .. })
            ),
            "tampered bundle should be refused"
        );
    }

    #[test]
    fn delivered_execution_context_prefers_replicated_cluster_topology()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = sample_cuda_device().inventory_qualifiers();
        let mut second_device = sample_cuda_device();
        second_device.device = Device::new(DeviceKind::Cuda, 1, Some(String::from("cuda:1")));
        second_device.device_name = Some(String::from("NVIDIA CUDA Test Device 1"));
        let second = second_device.inventory_qualifiers();
        let cluster_execution = ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::TrustedLanDatagram,
            ClusterExecutionDisposition::ReplicaRouted,
        )
        .with_replica_state_digest("replica-state-digest")
        .with_execution_topology(ExecutionTopologyPlan::replicated(
            "cuda",
            vec![first.clone(), second.clone()],
        ))
        .with_selected_nodes(vec![
            ClusterSelectedNode::new("worker-a", "cuda").with_device_inventory(first.clone()),
            ClusterSelectedNode::new("worker-b", "cuda").with_device_inventory(second.clone()),
        ])
        .with_replica_nodes(vec![
            crate::ClusterReplicaNode::new(
                0,
                ClusterSelectedNode::new("worker-a", "cuda").with_device_inventory(first.clone()),
                crate::ClusterReplicaWarmState::Warm,
                crate::ClusterReplicaRoutingDisposition::Selected,
            )
            .with_load(2, 0),
            crate::ClusterReplicaNode::new(
                1,
                ClusterSelectedNode::new("worker-b", "cuda").with_device_inventory(second.clone()),
                crate::ClusterReplicaWarmState::Warm,
                crate::ClusterReplicaRoutingDisposition::WarmStandby,
            )
            .with_load(0, 0),
        ]);
        let delivered = DeliveredExecutionContext::new(
            "cuda",
            Some(ExecutionTopologyPlan::single_device("cuda", first.clone())),
            vec![first.clone()],
        )
        .with_cluster_execution(cluster_execution.clone());

        assert_eq!(
            delivered.execution_topology.as_ref().map(|plan| plan.kind),
            Some(ExecutionTopologyKind::Replicated)
        );
        assert_eq!(delivered.selected_devices, vec![first, second]);
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .and_then(|cluster| cluster.replica_state_digest.as_deref()),
            Some("replica-state-digest")
        );
        assert_eq!(delivered.cluster_execution, Some(cluster_execution));
        Ok(())
    }

    #[test]
    fn delivered_execution_context_surfaces_layer_sharded_handoffs()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = sample_cuda_device().inventory_qualifiers();
        let mut second_device = sample_cuda_device();
        second_device.device = Device::new(DeviceKind::Cuda, 1, Some(String::from("cuda:1")));
        second_device.device_name = Some(String::from("NVIDIA CUDA Test Device 1"));
        let second = second_device.inventory_qualifiers();
        let cluster_execution = ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::Mixed,
            ClusterExecutionDisposition::Sharded,
        )
        .with_communication_eligibility(
            crate::ClusterCommunicationEligibility::new(
                "cuda",
                crate::ClusterCommunicationClass::LayerShardHandoff,
            )
            .with_supported_classes(vec![
                crate::ClusterCommunicationClass::RemoteDispatch,
                crate::ClusterCommunicationClass::ReplicaRouting,
                crate::ClusterCommunicationClass::LayerShardHandoff,
            ])
            .with_detail(
                "backend `cuda` supports layer-sharded cluster handoff under explicit stream-capable transport policy",
            ),
        )
        .with_execution_topology(ExecutionTopologyPlan::layer_sharded(
            "cuda",
            vec![(first.clone(), 0, 20), (second.clone(), 20, 40)],
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Sharding,
            "sharding-policy-digest",
        ))
        .with_selected_nodes(vec![
            ClusterSelectedNode::new("worker-a", "cuda").with_device_inventory(first.clone()),
            ClusterSelectedNode::new("worker-b", "cuda").with_device_inventory(second.clone()),
        ])
        .with_shard_handoffs(vec![
            crate::ClusterShardHandoff::new(
                0,
                1,
                "worker-a",
                "worker-b",
                crate::ClusterShardHandoffKind::Activation,
                ClusterTransportClass::TrustedLanStream,
                20,
                8192,
            )
            .with_detail("forward activations across the shard boundary"),
            crate::ClusterShardHandoff::new(
                0,
                1,
                "worker-a",
                "worker-b",
                crate::ClusterShardHandoffKind::KvCache,
                ClusterTransportClass::TrustedLanStream,
                20,
                4096,
            )
            .with_detail("forward kv cache across the shard boundary"),
        ]);
        let delivered = DeliveredExecutionContext::new(
            "cuda",
            Some(ExecutionTopologyPlan::single_device("cuda", first.clone())),
            vec![first.clone()],
        )
        .with_cluster_execution(cluster_execution.clone());

        assert_eq!(
            delivered.execution_topology.as_ref().map(|plan| plan.kind),
            Some(ExecutionTopologyKind::LayerSharded)
        );
        assert_eq!(delivered.selected_devices, vec![first, second]);
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .map(|cluster| cluster.shard_handoffs.len()),
            Some(2)
        );
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .map(|cluster| cluster.shard_handoffs[0].kind),
            Some(crate::ClusterShardHandoffKind::Activation)
        );
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .and_then(|cluster| cluster.communication_eligibility.as_ref())
                .map(|eligibility| eligibility.required_class),
            Some(crate::ClusterCommunicationClass::LayerShardHandoff)
        );
        Ok(())
    }

    #[test]
    fn delivered_execution_context_surfaces_tensor_sharded_collectives()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = sample_cuda_device().inventory_qualifiers();
        let mut second_device = sample_cuda_device();
        second_device.device = Device::new(DeviceKind::Cuda, 1, Some(String::from("cuda:1")));
        second_device.device_name = Some(String::from("NVIDIA CUDA Test Device 1"));
        let second = second_device.inventory_qualifiers();
        let cluster_execution = ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::Mixed,
            ClusterExecutionDisposition::Sharded,
        )
        .with_communication_eligibility(
            crate::ClusterCommunicationEligibility::new(
                "cuda",
                crate::ClusterCommunicationClass::TensorCollectiveMesh,
            )
            .with_supported_classes(vec![
                crate::ClusterCommunicationClass::RemoteDispatch,
                crate::ClusterCommunicationClass::ReplicaRouting,
                crate::ClusterCommunicationClass::LayerShardHandoff,
                crate::ClusterCommunicationClass::TensorCollectiveMesh,
            ])
            .with_detail(
                "backend `cuda` supports tensor collectives under explicit low-latency mesh transport policy",
            ),
        )
        .with_execution_topology(ExecutionTopologyPlan::tensor_sharded(
            "cuda",
            1,
            vec![(first.clone(), 0, 32), (second.clone(), 32, 64)],
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Sharding,
            "tensor-sharding-policy-digest",
        ))
        .with_selected_nodes(vec![
            ClusterSelectedNode::new("worker-a", "cuda").with_device_inventory(first.clone()),
            ClusterSelectedNode::new("worker-b", "cuda").with_device_inventory(second.clone()),
        ])
        .with_shard_handoffs(vec![
            crate::ClusterShardHandoff::new(
                0,
                1,
                "worker-a",
                "worker-b",
                crate::ClusterShardHandoffKind::TensorCollective,
                ClusterTransportClass::TrustedLanStream,
                0,
                16_384,
            )
            .with_tensor_partition(1, 0, 32)
            .with_detail("synchronize tensor shard [0..32) on axis 1 across the CUDA mesh"),
        ]);
        let delivered = DeliveredExecutionContext::new(
            "cuda",
            Some(ExecutionTopologyPlan::single_device("cuda", first.clone())),
            vec![first.clone()],
        )
        .with_cluster_execution(cluster_execution.clone());

        assert_eq!(
            delivered.execution_topology.as_ref().map(|plan| plan.kind),
            Some(ExecutionTopologyKind::TensorSharded)
        );
        assert_eq!(delivered.selected_devices, vec![first, second]);
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .map(|cluster| cluster.shard_handoffs.len()),
            Some(1)
        );
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .map(|cluster| cluster.shard_handoffs[0].kind),
            Some(crate::ClusterShardHandoffKind::TensorCollective)
        );
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .and_then(|cluster| cluster.shard_handoffs[0].tensor_axis),
            Some(1)
        );
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .and_then(|cluster| cluster.shard_handoffs[0].tensor_range_start),
            Some(0)
        );
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .and_then(|cluster| cluster.shard_handoffs[0].tensor_range_end),
            Some(32)
        );
        assert_eq!(
            delivered
                .cluster_execution
                .as_ref()
                .and_then(|cluster| cluster.communication_eligibility.as_ref())
                .map(|eligibility| eligibility.required_class),
            Some(crate::ClusterCommunicationClass::TensorCollectiveMesh)
        );
        Ok(())
    }

    #[test]
    fn accelerator_requirements_can_match_exact_delivery() {
        let selection = BackendSelection::direct(
            "cuda",
            Some(sample_cuda_device()),
            vec![String::from("matmul")],
        );
        let report = AcceleratorExecutionRequirement::new("cuda", 1)
            .with_topology_kind(ExecutionTopologyKind::SingleDevice)
            .with_minimum_performance_class(DevicePerformanceClass::DiscreteAccelerator)
            .with_minimum_memory_class(DeviceMemoryClass::DedicatedDevice)
            .with_minimum_total_memory_bytes(16 * 1024 * 1024 * 1024)
            .evaluate(DeliveredExecutionContext::from_backend_selection(
                &selection,
            ));

        assert_eq!(report.status, AcceleratorDeliverabilityStatus::Exact);
        assert!(report.differences.is_empty());
    }

    #[test]
    fn accelerator_requirements_mark_topology_change_as_compatible_substitution() {
        let first = sample_cuda_device();
        let mut second = sample_cuda_device();
        second.device = Device::new(DeviceKind::Cuda, 1, Some(String::from("cuda:1")));
        second.device_name = Some(String::from("NVIDIA CUDA Test Device 1"));
        let metadata = second
            .nvidia_metadata
            .as_mut()
            .expect("sample cuda metadata");
        metadata.topology.pci_bdf = Some(String::from("00000000:02:00.0"));
        let selection =
            BackendSelection::direct("cuda", Some(first.clone()), vec![String::from("matmul")])
                .with_selected_devices(vec![first.clone(), second.clone()])
                .with_execution_topology(Some(ExecutionTopologyPlan::layer_sharded(
                    "cuda",
                    vec![
                        (first.inventory_qualifiers(), 0, 20),
                        (second.inventory_qualifiers(), 20, 40),
                    ],
                )));

        let report = AcceleratorExecutionRequirement::new("cuda", 1)
            .with_topology_kind(ExecutionTopologyKind::SingleDevice)
            .with_minimum_performance_class(DevicePerformanceClass::DiscreteAccelerator)
            .with_minimum_total_memory_bytes(16 * 1024 * 1024 * 1024)
            .evaluate(DeliveredExecutionContext::from_backend_selection(
                &selection,
            ));

        assert_eq!(
            report.status,
            AcceleratorDeliverabilityStatus::CompatibleSubstitution
        );
        assert_eq!(
            report.differences.first().map(|value| value.code),
            Some(AcceleratorDeliverabilityDifferenceCode::TopologyKindChanged)
        );
    }

    #[test]
    fn accelerator_requirements_can_detect_underdelivery() {
        let selection = BackendSelection::direct(
            "cuda",
            Some(sample_cuda_device()),
            vec![String::from("matmul")],
        );
        let report = AcceleratorExecutionRequirement::new("cuda", 2)
            .with_required_topology_keys(vec![String::from("00000000:02:00.0")])
            .with_minimum_total_memory_bytes(32 * 1024 * 1024 * 1024)
            .evaluate(DeliveredExecutionContext::from_backend_selection(
                &selection,
            ));

        assert_eq!(
            report.status,
            AcceleratorDeliverabilityStatus::Underdelivered
        );
        assert!(report.differences.iter().any(|difference| {
            difference.code == AcceleratorDeliverabilityDifferenceCode::DeviceCountReduced
        }));
        assert!(report.differences.iter().any(|difference| {
            difference.code == AcceleratorDeliverabilityDifferenceCode::TopologyKeyMissing
        }));
        assert!(report.differences.iter().any(|difference| {
            difference.code == AcceleratorDeliverabilityDifferenceCode::TotalMemoryReduced
        }));
    }

    #[test]
    fn device_inventory_qualifiers_mark_mig_devices_as_partitioned() {
        let mut device = sample_cuda_device();
        let metadata = device
            .nvidia_metadata
            .as_mut()
            .expect("sample cuda metadata");
        metadata.topology.mig_profile = Some(String::from("1g.10gb"));
        metadata.risk.mig_partitioned = true;

        assert_eq!(
            device.inventory_qualifiers().performance_class,
            DevicePerformanceClass::PartitionedAccelerator
        );
    }

    #[test]
    fn backend_health_tracker_records_only_real_changes() {
        let mut tracker = BackendHealthTracker::with_history_limit(2);
        let first = tracker.observe(
            "metal",
            RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("ready"),
            },
            10,
        );
        assert_eq!(first.changed_at_millis, 10);
        assert!(tracker.recent_changes().is_empty());

        tracker.observe(
            "metal",
            RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("ready"),
            },
            15,
        );
        assert!(tracker.recent_changes().is_empty());

        let changed = tracker.observe(
            "metal",
            RuntimeHealth {
                status: HealthStatus::Degraded,
                message: String::from("reduced throughput"),
            },
            20,
        );
        assert_eq!(changed.changed_at_millis, 20);
        assert_eq!(changed.observed_at_millis, 20);
        assert_eq!(changed.status, HealthStatus::Degraded);

        assert_eq!(
            tracker.recent_changes(),
            vec![RuntimeTransitionEvent {
                kind: RuntimeTransitionKind::BackendHealthChanged,
                model_id: None,
                backend: Some(String::from("metal")),
                previous_status: Some(HealthStatus::Ready),
                status: Some(HealthStatus::Degraded),
                message: Some(String::from("reduced throughput")),
                observed_at_millis: 20,
            }]
        );
    }

    #[test]
    fn runtime_transition_log_retains_latest_events() {
        let mut log = RuntimeTransitionLog::with_limit(2);
        log.record(RuntimeTransitionEvent::model(
            RuntimeTransitionKind::ModelLoadedCold,
            "alpha",
            1,
        ));
        log.record(RuntimeTransitionEvent::model(
            RuntimeTransitionKind::ModelBecameWarm,
            "alpha",
            2,
        ));
        log.record(RuntimeTransitionEvent::model(
            RuntimeTransitionKind::ModelUnloaded,
            "alpha",
            3,
        ));

        assert_eq!(
            log.snapshot(),
            vec![
                RuntimeTransitionEvent::model(RuntimeTransitionKind::ModelBecameWarm, "alpha", 2,),
                RuntimeTransitionEvent::model(RuntimeTransitionKind::ModelUnloaded, "alpha", 3),
            ]
        );
    }

    #[test]
    fn local_runtime_observability_serializes_stably() -> Result<(), Box<dyn std::error::Error>> {
        let observability = LocalRuntimeObservability {
            isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
            cache_invalidation_policy: default_cache_invalidation_policy(),
            execution_profile: ExecutionCapabilityProfile::single_request_latency_optimized(),
            queue_depth: 0,
            queue_capacity: None,
            active_sessions: 2,
            active_requests: 1,
            memory_footprint: MemoryResidencySnapshot {
                loaded_models: 1,
                resident_host_bytes: 1024,
                resident_device_bytes: 2048,
            },
            backend_health: vec![tracker_observation("cpu", HealthStatus::Ready, "ready", 10)],
            recent_transitions: vec![
                RuntimeTransitionEvent::model(
                    RuntimeTransitionKind::ModelLoadedCold,
                    "fixture-word-decoder-v0",
                    8,
                ),
                RuntimeTransitionEvent {
                    kind: RuntimeTransitionKind::BackendHealthChanged,
                    model_id: None,
                    backend: Some(String::from("cpu")),
                    previous_status: Some(HealthStatus::Degraded),
                    status: Some(HealthStatus::Ready),
                    message: Some(String::from("ready")),
                    observed_at_millis: 10,
                },
            ],
        };

        assert_eq!(
            serde_json::to_value(&observability)?,
            json!({
                "isolation_policy": {
                    "backend_interface_mode": "in_process",
                    "failure_boundary": "shared_host_process",
                    "request_failure_recovery": "refuse_request",
                    "backend_error_recovery": "reset_runtime_state",
                    "crash_recovery": "restart_host_process",
                    "reset_scopes": [
                        "loaded_models",
                        "sessions",
                        "prefix_cache",
                        "kv_state",
                        "backend_runtime_resources"
                    ]
                },
                "cache_invalidation_policy": {
                    "runtime_binary_version": env!("CARGO_PKG_VERSION"),
                    "execution_plan": {
                        "scope": "process_local",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "rebuild",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "model_metadata_change",
                            "tokenizer_drift",
                            "chat_template_drift",
                            "generation_defaults_drift",
                            "quantization_change",
                            "plan_format_upgrade"
                        ]
                    },
                    "kernel_cache": {
                        "scope": "process_local",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "invalidate",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "kernel_format_upgrade"
                        ]
                    },
                    "paged_tensor_storage": {
                        "scope": "artifact_backed",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "restore",
                        "invalidates_on": [
                            "binary_upgrade",
                            "model_metadata_change",
                            "quantization_change",
                            "paged_tensor_format_upgrade"
                        ]
                    },
                    "prefix_cache": {
                        "scope": "shared_across_requests",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "rebuild",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "model_metadata_change",
                            "tokenizer_drift",
                            "chat_template_drift",
                            "generation_defaults_drift",
                            "quantization_change",
                            "prefix_cache_format_upgrade"
                        ]
                    },
                    "kv_state": {
                        "scope": "session_bound",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "invalidate",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "model_metadata_change",
                            "tokenizer_drift",
                            "chat_template_drift",
                            "generation_defaults_drift",
                            "quantization_change",
                            "kv_state_format_upgrade"
                        ]
                    }
                },
                "execution_profile": {
                    "batch_posture": "single_request_only",
                    "queue_policy": {
                        "discipline": "direct_caller_backpressure",
                        "max_active_requests": 1,
                        "max_queued_requests": 0,
                        "per_model_serialization": true
                    },
                    "throughput_class": "latency_optimized"
                },
                "queue_depth": 0,
                "active_sessions": 2,
                "active_requests": 1,
                "memory_footprint": {
                    "loaded_models": 1,
                    "resident_host_bytes": 1024,
                    "resident_device_bytes": 2048
                },
                "backend_health": [{
                    "backend": "cpu",
                    "status": "Ready",
                    "message": "ready",
                    "observed_at_millis": 10,
                    "changed_at_millis": 10
                }],
                "recent_transitions": [
                    {
                        "kind": "model_loaded_cold",
                        "model_id": "fixture-word-decoder-v0",
                        "observed_at_millis": 8
                    },
                    {
                        "kind": "backend_health_changed",
                        "backend": "cpu",
                        "previous_status": "Degraded",
                        "status": "Ready",
                        "message": "ready",
                        "observed_at_millis": 10
                    }
                ]
            })
        );
        Ok(())
    }

    #[test]
    fn execution_capability_profiles_are_machine_checkable() {
        assert_eq!(
            ExecutionCapabilityProfile::single_request_latency_optimized(),
            ExecutionCapabilityProfile {
                batch_posture: BatchExecutionPosture::SingleRequestOnly,
                queue_policy: QueuePolicy {
                    discipline: QueueDiscipline::DirectCallerBackpressure,
                    max_active_requests: 1,
                    max_queued_requests: 0,
                    per_model_serialization: true,
                },
                throughput_class: ThroughputClass::LatencyOptimized,
                prefill_decode_capability: None,
            }
        );
        assert_eq!(
            ExecutionCapabilityProfile::caller_static_batch_balanced(),
            ExecutionCapabilityProfile {
                batch_posture: BatchExecutionPosture::CallerStaticBatch,
                queue_policy: QueuePolicy {
                    discipline: QueueDiscipline::DirectCallerBackpressure,
                    max_active_requests: 1,
                    max_queued_requests: 0,
                    per_model_serialization: true,
                },
                throughput_class: ThroughputClass::Balanced,
                prefill_decode_capability: None,
            }
        );
        let scheduler_policy = GenerationSchedulerPolicy::continuous_batch_default();
        assert_eq!(
            ExecutionCapabilityProfile::continuous_batch_throughput_optimized(&scheduler_policy),
            ExecutionCapabilityProfile {
                batch_posture: BatchExecutionPosture::ContinuousBatch,
                queue_policy: QueuePolicy {
                    discipline: QueueDiscipline::Fifo,
                    max_active_requests: scheduler_policy.max_active_requests,
                    max_queued_requests: scheduler_policy.max_queued_requests,
                    per_model_serialization: true,
                },
                throughput_class: ThroughputClass::ThroughputOptimized,
                prefill_decode_capability: None,
            }
        );
    }

    #[test]
    fn generation_scheduler_metrics_track_fallback_counts() {
        let mut metrics = GenerationSchedulerMetrics::for_policy(
            GenerationSchedulerPolicy::continuous_batch_default(),
        );
        metrics.record_fallback(GenerationSchedulerFallbackReason::QueueCapacityExceeded);
        metrics.record_fallback(GenerationSchedulerFallbackReason::SessionSerialization);
        metrics.record_fallback(GenerationSchedulerFallbackReason::QueueCapacityExceeded);

        assert_eq!(
            metrics.fallback_counts,
            vec![
                GenerationSchedulerFallbackCount {
                    reason: GenerationSchedulerFallbackReason::QueueCapacityExceeded,
                    count: 2,
                },
                GenerationSchedulerFallbackCount {
                    reason: GenerationSchedulerFallbackReason::SessionSerialization,
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn runtime_refusal_taxonomy_maps_capability_and_serialization_boundaries() {
        let step_refusal = RuntimeError::UnsupportedStep(String::from("rope")).refusal();
        assert!(step_refusal.is_some());
        let Some(step_refusal) = step_refusal else {
            return;
        };
        assert_eq!(step_refusal.code, PsionicRefusalCode::UnsupportedOp);
        assert_eq!(step_refusal.scope, PsionicRefusalScope::Runtime);
        assert_eq!(step_refusal.subject.as_deref(), Some("rope"));

        let diagnostic = super::LocalRuntimeDiagnostic::new(
            super::LocalRuntimeErrorCode::UnsupportedCapability,
            503,
            "selected backend does not expose kv residency",
        )
        .with_backend("metal");
        let diagnostic_refusal = diagnostic.refusal();
        assert!(diagnostic_refusal.is_some());
        let Some(diagnostic_refusal) = diagnostic_refusal else {
            return;
        };
        assert_eq!(
            diagnostic_refusal.code,
            PsionicRefusalCode::UnsupportedBackendCapability
        );
        assert_eq!(diagnostic_refusal.subject.as_deref(), Some("metal"));

        let serialization = GenerationSchedulerFallbackReason::SessionSerialization.refusal();
        assert!(serialization.is_some());
        let Some(serialization) = serialization else {
            return;
        };
        assert_eq!(
            serialization.code,
            PsionicRefusalCode::SerializationIncompatibility
        );
        assert_eq!(serialization.scope, PsionicRefusalScope::Runtime);
    }

    #[test]
    fn quantization_dispatch_prefers_grouped_block_for_grouped_ggml_workload() {
        let request = QuantizationDispatchRequest::new(
            QuantizationMode::GgmlQ4_0,
            QuantizationDispatchWorkload::GroupedExpert,
            8,
            4096,
        )
        .with_native_quantized_kernels(true)
        .with_grouped_dispatch(true);

        let decision = super::QuantizationDispatchDecision::advise(&request);
        let benchmark = benchmark_quantization_dispatch(&request);

        assert_eq!(decision.strategy, QuantizationKernelStrategy::GroupedBlock);
        assert!(benchmark.optimized_cost_units < benchmark.baseline_cost_units);
        assert!(benchmark.improvement_basis_points > 0);
    }

    #[test]
    fn runtime_dispatch_plan_batches_data_plane_work_and_reduces_cost() {
        let items = vec![
            RuntimeWorkItem::new(RuntimeWorkClass::DatastreamChunk, 1, 256 * 1024),
            RuntimeWorkItem::new(RuntimeWorkClass::DatastreamChunk, 1, 256 * 1024),
            RuntimeWorkItem::new(RuntimeWorkClass::DatastreamChunk, 1, 256 * 1024),
            RuntimeWorkItem::new(RuntimeWorkClass::DatastreamChunk, 1, 256 * 1024),
        ];
        let policy = RuntimeDispatchPolicy::data_plane_default(4);
        let plan = RuntimeDispatchPlan::plan(policy.clone(), &items);
        let naive = RuntimeDispatchPlan::naive(&items, policy.max_workers);
        let benchmark = benchmark_dispatch_plan("data_plane", policy, &items);

        assert!(plan.total_wake_events < naive.total_wake_events);
        assert!(plan.simulated_cost_units() < naive.simulated_cost_units());
        assert!(benchmark.optimized_cost_units < benchmark.baseline_cost_units);
    }

    #[test]
    fn quantized_decode_policy_keeps_latency_sensitive_work_unbatched() {
        let items = vec![
            RuntimeWorkItem::new(RuntimeWorkClass::DecodeToken, 1, 4096).latency_sensitive(),
            RuntimeWorkItem::new(RuntimeWorkClass::DecodeToken, 1, 4096).latency_sensitive(),
        ];
        let plan =
            RuntimeDispatchPlan::plan(RuntimeDispatchPolicy::quantized_decode_default(2), &items);

        assert_eq!(plan.batches.len(), 2);
        assert_eq!(plan.total_wake_events, 2);
    }

    #[test]
    fn sandbox_execution_capability_profiles_are_machine_checkable()
    -> Result<(), Box<dyn std::error::Error>> {
        let profile = SandboxExecutionCapabilityProfile::bounded_accelerated("cuda", 2);
        assert!(profile.accelerator_access.requires_accelerator());
        assert_eq!(
            serde_json::to_value(&profile)?,
            json!({
                "dispatch_profile": {
                    "batch_posture": "single_request_only",
                    "queue_policy": {
                        "discipline": "direct_caller_backpressure",
                        "max_active_requests": 1,
                        "max_queued_requests": 0,
                        "per_model_serialization": true
                    },
                    "throughput_class": "latency_optimized"
                },
                "isolation_boundary": "container",
                "filesystem": {
                    "root": "read_only",
                    "writable_mounts": ["/tmp"],
                    "max_write_bytes": 67108864
                },
                "network": {
                    "mode": "disabled",
                    "allow_loopback": false
                },
                "process": {
                    "max_processes": 32,
                    "max_threads_per_process": 8,
                    "allow_privilege_escalation": false
                },
                "resource_limits": {
                    "max_wall_time_ms": 300000,
                    "max_cpu_time_ms": 300000,
                    "max_memory_bytes": 2147483648u64,
                    "max_stdout_bytes": 1048576,
                    "max_stderr_bytes": 1048576
                },
                "accelerator_access": {
                    "mode": "allowed",
                    "runtime_backend": "cuda",
                    "max_visible_devices": 2,
                    "allowed_performance_classes": [
                        "integrated_accelerator",
                        "discrete_accelerator",
                        "partitioned_accelerator"
                    ],
                    "require_topology_keys": true
                }
            })
        );
        assert_eq!(
            SandboxExecutionCapabilityProfile::bounded_cpu(),
            SandboxExecutionCapabilityProfile {
                dispatch_profile: ExecutionCapabilityProfile::single_request_latency_optimized(),
                isolation_boundary: SandboxIsolationBoundary::Container,
                filesystem: SandboxFilesystemPolicy {
                    root: SandboxFilesystemRoot::ReadOnly,
                    writable_mounts: vec![String::from("/tmp")],
                    max_write_bytes: 64 * 1024 * 1024,
                },
                network: SandboxNetworkPolicy {
                    mode: SandboxNetworkMode::Disabled,
                    allow_loopback: false,
                    allowed_hosts: Vec::new(),
                },
                process: SandboxProcessPolicy {
                    max_processes: 32,
                    max_threads_per_process: 8,
                    allow_privilege_escalation: false,
                },
                resource_limits: SandboxResourceLimits {
                    max_wall_time_ms: 300_000,
                    max_cpu_time_ms: 300_000,
                    max_memory_bytes: 2 * 1024 * 1024 * 1024,
                    max_stdout_bytes: 1024 * 1024,
                    max_stderr_bytes: 1024 * 1024,
                },
                accelerator_access: SandboxAcceleratorAccess::Disabled,
            }
        );
        Ok(())
    }

    #[test]
    fn sandbox_execution_profile_digest_changes_with_bounds() {
        let baseline = SandboxExecutionCapabilityProfile::bounded_cpu();
        let tightened = SandboxExecutionCapabilityProfile {
            resource_limits: SandboxResourceLimits {
                max_wall_time_ms: 60_000,
                ..baseline.resource_limits.clone()
            },
            ..baseline.clone()
        };
        assert_ne!(baseline.stable_digest(), tightened.stable_digest());
    }

    #[test]
    fn sandbox_execution_evidence_serializes_stably() -> Result<(), Box<dyn std::error::Error>> {
        let request = SandboxExecutionRequestIdentity {
            request_id: String::from("sandbox-req-1"),
            sandbox_profile_digest: String::from("profile123"),
            command_digest: String::from("command123"),
            environment_digest: String::from("env123"),
            input_artifact_digests: vec![String::from("input-a"), String::from("input-b")],
        };
        let evidence = SandboxExecutionEvidence {
            request_digest: String::from("request123"),
            sandbox_profile_digest: request.sandbox_profile_digest.clone(),
            command_digest: request.command_digest.clone(),
            environment_digest: request.environment_digest.clone(),
            input_artifact_digests: request.input_artifact_digests.clone(),
            output_artifact_digests: vec![String::from("output-a")],
            exit: SandboxExecutionExit {
                kind: SandboxExecutionExitKind::NonZeroExit,
                exit_code: Some(17),
                detail: String::from("tool exited non-zero"),
            },
            resources: SandboxExecutionResourceSummary {
                wall_time_ms: 1500,
                cpu_time_ms: 900,
                peak_memory_bytes: 512 * 1024 * 1024,
                filesystem_write_bytes: 4096,
                stdout_bytes: 128,
                stderr_bytes: 64,
                network_egress_bytes: 0,
            },
            stdout_sha256: Some(String::from("stdout123")),
            stderr_sha256: Some(String::from("stderr123")),
            delivery_proof: Some(ExecutionDeliveryProof {
                execution_plan_digest: String::from("plan123"),
                kernel_count: 3,
                bytes_moved: 1024,
                plan_cache_hits: 1,
                plan_cache_misses: 0,
                kv_growth: None,
                prefill_decode_handoff: None,
                kv_residency: None,
            }),
        };

        assert_eq!(
            serde_json::to_value(&evidence)?,
            json!({
                "request_digest": "request123",
                "sandbox_profile_digest": "profile123",
                "command_digest": "command123",
                "environment_digest": "env123",
                "input_artifact_digests": ["input-a", "input-b"],
                "output_artifact_digests": ["output-a"],
                "exit": {
                    "kind": "non_zero_exit",
                    "exit_code": 17,
                    "detail": "tool exited non-zero"
                },
                "resources": {
                    "wall_time_ms": 1500,
                    "cpu_time_ms": 900,
                    "peak_memory_bytes": 536870912u64,
                    "filesystem_write_bytes": 4096,
                    "stdout_bytes": 128,
                    "stderr_bytes": 64,
                    "network_egress_bytes": 0
                },
                "stdout_sha256": "stdout123",
                "stderr_sha256": "stderr123",
                "delivery_proof": {
                    "execution_plan_digest": "plan123",
                    "kernel_count": 3,
                    "bytes_moved": 1024,
                    "plan_cache_hits": 1,
                    "plan_cache_misses": 0
                }
            })
        );
        Ok(())
    }

    #[test]
    fn cache_observation_serializes_stably() -> Result<(), Box<dyn std::error::Error>> {
        let observation = CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Rebuild,
            "stale prefix entry rebuilt under the current runtime",
        )
        .with_trigger(CacheInvalidationTrigger::PrefixCacheFormatUpgrade);

        assert_eq!(
            serde_json::to_value(&observation)?,
            json!({
                "kind": "prefix_cache",
                "action": "rebuild",
                "trigger": "prefix_cache_format_upgrade",
                "detail": "stale prefix entry rebuilt under the current runtime"
            })
        );
        Ok(())
    }

    #[test]
    fn local_serving_isolation_policy_helpers_are_stable() -> Result<(), Box<dyn std::error::Error>>
    {
        assert_eq!(
            LocalServingIsolationPolicy::default(),
            LocalServingIsolationPolicy::in_process_runtime()
        );
        assert_eq!(
            serde_json::to_value(LocalServingIsolationPolicy::subprocess_runtime())?,
            json!({
                "backend_interface_mode": "subprocess",
                "failure_boundary": "dedicated_runtime_subprocess",
                "request_failure_recovery": "refuse_request",
                "backend_error_recovery": "restart_runtime_subprocess",
                "crash_recovery": "restart_runtime_subprocess",
                "reset_scopes": [
                    "loaded_models",
                    "sessions",
                    "prefix_cache",
                    "kv_state",
                    "backend_runtime_resources"
                ]
            })
        );
        Ok(())
    }

    fn tracker_observation(
        backend: &str,
        status: HealthStatus,
        message: &str,
        observed_at_millis: u64,
    ) -> super::BackendHealthObservation {
        super::BackendHealthObservation {
            backend: String::from(backend),
            status,
            message: String::from(message),
            observed_at_millis,
            changed_at_millis: observed_at_millis,
        }
    }
}
