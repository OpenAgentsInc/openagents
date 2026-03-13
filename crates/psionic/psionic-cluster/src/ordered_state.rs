use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::SocketAddr,
    path::Path,
};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ClusterDiscoveryCandidate, ClusterId, ClusterIntroductionPolicy,
    ClusterIntroductionVerificationError, ClusterJoinRefusal, ClusterNodeIdentity,
    ConfiguredClusterPeer, NodeId, PeerSnapshot, SignedClusterIntroductionEnvelope,
};

/// Monotonic cluster-election term for the first ordered-control seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterTerm(u64);

impl ClusterTerm {
    /// Initial election term for a never-before-established coordinator.
    #[must_use]
    pub const fn initial() -> Self {
        Self(1)
    }

    /// Next election term after one already-observed leader epoch.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Returns the raw term value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Monotonic logical tick used for leadership lease freshness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterLeaseTick(u64);

impl ClusterLeaseTick {
    /// Initial logical tick for a never-before-observed coordinator.
    #[must_use]
    pub const fn initial() -> Self {
        Self(0)
    }

    /// Creates an explicit logical tick.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Next logical tick after one already-observed heartbeat.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Returns the tick plus one explicit timeout window.
    #[must_use]
    pub const fn saturating_add(self, delta: u64) -> Self {
        Self(self.0.saturating_add(delta))
    }

    /// Returns the raw logical tick value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Monotonic ordered-event index for authoritative cluster facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterEventIndex(u64);

impl ClusterEventIndex {
    /// First authoritative event index in the ordered cluster log.
    #[must_use]
    pub const fn initial() -> Self {
        Self(1)
    }

    /// Next event index after one already-applied authoritative event.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Returns the raw event index value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Explicit schema/version boundary for authoritative cluster state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterSchemaVersion {
    /// Major version for incompatible state changes.
    pub major: u16,
    /// Minor version for compatible additive changes.
    pub minor: u16,
}

impl ClusterSchemaVersion {
    /// Current initial schema for the first cluster-state seam.
    #[must_use]
    pub const fn initial() -> Self {
        Self { major: 1, minor: 0 }
    }

    /// Creates an explicit cluster schema version.
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

/// Membership status visible in authoritative cluster state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterMembershipStatus {
    /// Node is joining but not yet relied on for steady-state work.
    Joining,
    /// Node is an active member of the cluster.
    Ready,
    /// Node is draining and should not receive new work.
    Draining,
    /// Node is known but currently unavailable.
    Offline,
}

/// Cluster-visible membership record for one node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterMembershipRecord {
    /// Stable node identity and role truth.
    pub identity: ClusterNodeIdentity,
    /// Advertised control-plane address, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advertised_addr: Option<SocketAddr>,
    /// Current membership state for the node.
    pub status: ClusterMembershipStatus,
}

impl ClusterMembershipRecord {
    /// Creates a cluster membership record from identity and state.
    #[must_use]
    pub fn new(
        identity: ClusterNodeIdentity,
        advertised_addr: Option<SocketAddr>,
        status: ClusterMembershipStatus,
    ) -> Self {
        Self {
            identity,
            advertised_addr,
            status,
        }
    }
}

/// Authoritative status for one discovered wider-network candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterDiscoveredCandidateStatus {
    /// Candidate has been introduced but not yet admitted.
    Introduced,
    /// Candidate has been promoted into explicit membership truth.
    Accepted,
    /// Candidate was refused under current admission policy.
    Refused,
    /// Candidate introduction expired before admission.
    Expired,
}

/// Cluster-visible discovery candidate record kept separate from admitted membership.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterDiscoveredCandidateRecord {
    /// Candidate node introduced by the signed discovery artifact.
    pub candidate: ClusterDiscoveryCandidate,
    /// Stable source identifier that introduced the candidate.
    pub introduced_by_source_id: String,
    /// Stable digest of the introduction policy used to admit the artifact.
    pub introduction_policy_digest: String,
    /// Stable digest of the signed introduction payload.
    pub introduction_payload_digest: String,
    /// Inclusive issuance timestamp for the introduction.
    pub introduced_at_ms: u64,
    /// Inclusive expiry timestamp for the introduction.
    pub expires_at_ms: u64,
    /// Current discovery-candidate status.
    pub status: ClusterDiscoveredCandidateStatus,
    /// Machine-checkable detail for the current status, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterDiscoveredCandidateRecord {
    /// Builds a discovery candidate record from one verified introduction artifact.
    pub fn from_signed_introduction(
        envelope: &SignedClusterIntroductionEnvelope,
        policy: &ClusterIntroductionPolicy,
    ) -> Result<Self, ClusterIntroductionVerificationError> {
        envelope.verify(policy)?;
        Ok(Self {
            candidate: envelope.payload.candidate.clone(),
            introduced_by_source_id: envelope.signature.source_id.clone(),
            introduction_policy_digest: policy.stable_digest(),
            introduction_payload_digest: envelope.payload.stable_digest(),
            introduced_at_ms: envelope.payload.issued_at_ms,
            expires_at_ms: envelope.payload.expires_at_ms,
            status: ClusterDiscoveredCandidateStatus::Introduced,
            detail: None,
        })
    }

    /// Attaches an explicit candidate status.
    #[must_use]
    pub const fn with_status(mut self, status: ClusterDiscoveredCandidateStatus) -> Self {
        self.status = status;
        self
    }

    /// Attaches a machine-checkable detail string.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Link health visible in authoritative cluster state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterLinkStatus {
    /// Link has been observed but not yet validated.
    Pending,
    /// Link is healthy enough for normal control-plane traffic.
    Healthy,
    /// Link is present but degraded.
    Degraded,
    /// Link is unavailable.
    Disconnected,
}

/// Transport class visible in authoritative cluster state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTransportClass {
    /// Loopback UDP transport.
    LoopbackUdp,
    /// Same-network UDP transport.
    LanUdp,
    /// Generic TCP transport.
    Tcp,
    /// Explicit low-latency/RDMA-class transport.
    Rdma,
    /// Unknown or not-yet-classified transport.
    Unknown,
}

/// Qualitative stability posture for cluster nodes and links.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterStabilityPosture {
    /// Stable enough for ordinary scheduling.
    Stable,
    /// Present but intermittently degraded.
    Flaky,
    /// Too unstable for trusted scheduling.
    Unstable,
}

/// Topology-visible connection class for one node pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterLinkClass {
    /// Local loopback path.
    Loopback,
    /// Ethernet-class path.
    Ethernet,
    /// Wi-Fi-class path.
    Wifi,
    /// Thunderbolt-class path.
    Thunderbolt,
    /// RDMA/JACCL-class path.
    Rdma,
    /// Unknown or not-yet-classified path.
    Unknown,
}

/// Canonical cluster link key for one unordered node pair.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterLinkKey {
    /// Lower-sorted node ID in the pair.
    pub left_node_id: NodeId,
    /// Higher-sorted node ID in the pair.
    pub right_node_id: NodeId,
}

impl ClusterLinkKey {
    /// Creates a canonical link key for one unordered node pair.
    #[must_use]
    pub fn new(first_node_id: NodeId, second_node_id: NodeId) -> Self {
        if first_node_id <= second_node_id {
            Self {
                left_node_id: first_node_id,
                right_node_id: second_node_id,
            }
        } else {
            Self {
                left_node_id: second_node_id,
                right_node_id: first_node_id,
            }
        }
    }
}

/// Cluster-visible connection fact that can be folded into authoritative state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterLink {
    /// Canonical unordered node pair for the link.
    pub key: ClusterLinkKey,
    /// Current transport class.
    pub transport: ClusterTransportClass,
    /// Current health of the link.
    pub status: ClusterLinkStatus,
    /// Topology-visible connection class.
    pub link_class: ClusterLinkClass,
    /// Observed one-way or median latency in microseconds, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_us: Option<u64>,
    /// Observed or configured bandwidth in megabits per second, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bandwidth_mbps: Option<u64>,
    /// Stability posture for this link.
    pub stability: ClusterStabilityPosture,
}

impl ClusterLink {
    /// Creates one cluster link fact from explicit endpoints and state.
    #[must_use]
    pub fn new(
        first_node_id: NodeId,
        second_node_id: NodeId,
        transport: ClusterTransportClass,
        status: ClusterLinkStatus,
    ) -> Self {
        Self {
            key: ClusterLinkKey::new(first_node_id, second_node_id),
            transport,
            status,
            link_class: default_link_class(transport),
            latency_us: None,
            bandwidth_mbps: None,
            stability: default_stability_for_link_status(status),
        }
    }

    /// Attaches an explicit topology-visible link class.
    #[must_use]
    pub const fn with_link_class(mut self, link_class: ClusterLinkClass) -> Self {
        self.link_class = link_class;
        self
    }

    /// Attaches observed link latency in microseconds.
    #[must_use]
    pub const fn with_latency_us(mut self, latency_us: u64) -> Self {
        self.latency_us = Some(latency_us);
        self
    }

    /// Attaches observed link bandwidth in megabits per second.
    #[must_use]
    pub const fn with_bandwidth_mbps(mut self, bandwidth_mbps: u64) -> Self {
        self.bandwidth_mbps = Some(bandwidth_mbps);
        self
    }

    /// Attaches an explicit stability posture.
    #[must_use]
    pub const fn with_stability_posture(mut self, stability: ClusterStabilityPosture) -> Self {
        self.stability = stability;
        self
    }
}

/// Backend readiness state visible in topology-aware cluster facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterBackendReadinessStatus {
    /// Backend is ready for scheduling.
    Ready,
    /// Backend is present but degraded.
    Degraded,
    /// Backend is explicitly unavailable or refused.
    Refused,
    /// Backend posture is unknown.
    Unknown,
}

/// Cluster-visible node telemetry and backend-readiness facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterNodeTelemetry {
    /// Node these telemetry facts describe.
    pub node_id: NodeId,
    /// Total memory visible to the node, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_memory_bytes: Option<u64>,
    /// Free memory currently visible to the node, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free_memory_bytes: Option<u64>,
    /// Logical CPU-core count, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_logical_cores: Option<u16>,
    /// Count of visible accelerator devices, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accelerator_count: Option<u16>,
    /// Backend readiness by backend label.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub backend_readiness: BTreeMap<String, ClusterBackendReadinessStatus>,
    /// Stability posture for the node.
    pub stability: ClusterStabilityPosture,
}

impl ClusterNodeTelemetry {
    /// Creates an empty telemetry record for one node.
    #[must_use]
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            total_memory_bytes: None,
            free_memory_bytes: None,
            cpu_logical_cores: None,
            accelerator_count: None,
            backend_readiness: BTreeMap::new(),
            stability: ClusterStabilityPosture::Stable,
        }
    }

    /// Attaches memory facts to the telemetry record.
    #[must_use]
    pub const fn with_memory(
        mut self,
        total_memory_bytes: Option<u64>,
        free_memory_bytes: Option<u64>,
    ) -> Self {
        self.total_memory_bytes = total_memory_bytes;
        self.free_memory_bytes = free_memory_bytes;
        self
    }

    /// Attaches logical CPU-core count.
    #[must_use]
    pub const fn with_cpu_logical_cores(mut self, cpu_logical_cores: u16) -> Self {
        self.cpu_logical_cores = Some(cpu_logical_cores);
        self
    }

    /// Attaches visible accelerator count.
    #[must_use]
    pub const fn with_accelerator_count(mut self, accelerator_count: u16) -> Self {
        self.accelerator_count = Some(accelerator_count);
        self
    }

    /// Records readiness for one backend label.
    #[must_use]
    pub fn with_backend_readiness(
        mut self,
        backend: impl Into<String>,
        status: ClusterBackendReadinessStatus,
    ) -> Self {
        self.backend_readiness.insert(backend.into(), status);
        self
    }

    /// Attaches an explicit stability posture.
    #[must_use]
    pub const fn with_stability_posture(mut self, stability: ClusterStabilityPosture) -> Self {
        self.stability = stability;
        self
    }
}

/// Cluster-visible artifact reference tied to existing Psionic artifact identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterArtifactReference {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
    /// Stable provenance digest, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_digest: Option<String>,
    /// Stable governance/license digest, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub governance_digest: Option<String>,
    /// Stable supply-policy digest, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supply_policy_digest: Option<String>,
}

impl ClusterArtifactReference {
    /// Creates a cluster-visible artifact reference.
    #[must_use]
    pub fn new(artifact_id: impl Into<String>, artifact_digest: impl Into<String>) -> Self {
        Self {
            artifact_id: artifact_id.into(),
            artifact_digest: artifact_digest.into(),
            provenance_digest: None,
            governance_digest: None,
            supply_policy_digest: None,
        }
    }

    /// Attaches provenance digest truth.
    #[must_use]
    pub fn with_provenance_digest(mut self, provenance_digest: impl Into<String>) -> Self {
        self.provenance_digest = Some(provenance_digest.into());
        self
    }

    /// Attaches governance/license digest truth.
    #[must_use]
    pub fn with_governance_digest(mut self, governance_digest: impl Into<String>) -> Self {
        self.governance_digest = Some(governance_digest.into());
        self
    }

    /// Attaches compute-market supply-policy digest truth.
    #[must_use]
    pub fn with_supply_policy_digest(mut self, supply_policy_digest: impl Into<String>) -> Self {
        self.supply_policy_digest = Some(supply_policy_digest.into());
        self
    }
}

/// Artifact transfer method visible in cluster staging truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterArtifactTransferMethod {
    /// Peer-to-peer artifact copy from another node.
    PeerCopy,
    /// OCI or registry pull.
    OciPull,
    /// Unknown or not-yet-classified transfer method.
    Unknown,
}

/// Artifact residency/staging status visible in authoritative cluster state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterArtifactResidencyStatus {
    /// Artifact is already resident on the node.
    Resident,
    /// Artifact requires a peer copy before execution.
    CopyRequired,
    /// Artifact requires a pull/fetch before execution.
    PullRequired,
    /// Artifact is refused for this node under current policy.
    Refused,
}

/// Canonical key for one artifact/node residency record.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterArtifactResidencyKey {
    /// Node the residency record describes.
    pub node_id: NodeId,
    /// Stable artifact digest.
    pub artifact_digest: String,
}

impl ClusterArtifactResidencyKey {
    /// Creates a residency key from node and artifact digest.
    #[must_use]
    pub fn new(node_id: NodeId, artifact_digest: impl Into<String>) -> Self {
        Self {
            node_id,
            artifact_digest: artifact_digest.into(),
        }
    }
}

/// Cluster-visible artifact residency and staging record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterArtifactResidencyRecord {
    /// Canonical residency key.
    pub key: ClusterArtifactResidencyKey,
    /// Artifact identity and governance facts.
    pub artifact: ClusterArtifactReference,
    /// Current residency/staging state.
    pub status: ClusterArtifactResidencyStatus,
    /// Transfer method required for staging, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer_method: Option<ClusterArtifactTransferMethod>,
    /// Machine-checkable detail explaining the current staging state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterArtifactResidencyRecord {
    /// Creates an artifact residency record from node, artifact, and status.
    #[must_use]
    pub fn new(
        node_id: NodeId,
        artifact: ClusterArtifactReference,
        status: ClusterArtifactResidencyStatus,
    ) -> Self {
        Self {
            key: ClusterArtifactResidencyKey::new(node_id, artifact.artifact_digest.clone()),
            artifact,
            status,
            transfer_method: None,
            detail: None,
        }
    }

    /// Attaches an explicit transfer method.
    #[must_use]
    pub const fn with_transfer_method(
        mut self,
        transfer_method: ClusterArtifactTransferMethod,
    ) -> Self {
        self.transfer_method = Some(transfer_method);
        self
    }

    /// Attaches a machine-checkable detail string.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Lease/timeout policy for coordinator authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterLeadershipLeasePolicy {
    /// Number of logical ticks a coordinator stays fresh after one heartbeat.
    pub heartbeat_timeout_ticks: u64,
}

impl Default for ClusterLeadershipLeasePolicy {
    fn default() -> Self {
        Self {
            heartbeat_timeout_ticks: 3,
        }
    }
}

impl ClusterLeadershipLeasePolicy {
    /// Creates an explicit coordinator lease policy.
    #[must_use]
    pub fn new(heartbeat_timeout_ticks: u64) -> Self {
        Self {
            heartbeat_timeout_ticks: heartbeat_timeout_ticks.max(1),
        }
    }

    /// Returns the tick at which the current coordinator lease expires.
    #[must_use]
    pub const fn expires_at_tick(
        self,
        heartbeat_observed_tick: ClusterLeaseTick,
    ) -> ClusterLeaseTick {
        heartbeat_observed_tick.saturating_add(self.heartbeat_timeout_ticks)
    }
}

/// Explicit freshness window for one coordinator heartbeat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterLeadershipLease {
    /// Logical tick of the latest accepted coordinator heartbeat.
    pub heartbeat_observed_tick: ClusterLeaseTick,
    /// Logical tick after which this coordinator is stale.
    pub expires_at_tick: ClusterLeaseTick,
}

impl Default for ClusterLeadershipLease {
    fn default() -> Self {
        Self::from_policy(
            ClusterLeaseTick::initial(),
            ClusterLeadershipLeasePolicy::default(),
        )
    }
}

impl ClusterLeadershipLease {
    /// Creates one lease from an observed heartbeat and explicit policy.
    #[must_use]
    pub const fn from_policy(
        heartbeat_observed_tick: ClusterLeaseTick,
        lease_policy: ClusterLeadershipLeasePolicy,
    ) -> Self {
        Self {
            heartbeat_observed_tick,
            expires_at_tick: lease_policy.expires_at_tick(heartbeat_observed_tick),
        }
    }

    /// Returns whether the coordinator is still fresh at one observed tick.
    #[must_use]
    pub const fn status_at(self, observed_tick: ClusterLeaseTick) -> ClusterLeadershipLeaseStatus {
        if observed_tick.as_u64() <= self.expires_at_tick.as_u64() {
            ClusterLeadershipLeaseStatus::Active {
                expires_at_tick: self.expires_at_tick,
                remaining_ticks: self
                    .expires_at_tick
                    .as_u64()
                    .saturating_sub(observed_tick.as_u64()),
            }
        } else {
            ClusterLeadershipLeaseStatus::Stale {
                expired_at_tick: self.expires_at_tick,
                overdue_ticks: observed_tick
                    .as_u64()
                    .saturating_sub(self.expires_at_tick.as_u64()),
            }
        }
    }
}

/// Machine-checkable freshness state for the current coordinator record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClusterLeadershipLeaseStatus {
    /// Coordinator lease is still active.
    Active {
        expires_at_tick: ClusterLeaseTick,
        remaining_ticks: u64,
    },
    /// Coordinator lease has gone stale.
    Stale {
        expired_at_tick: ClusterLeaseTick,
        overdue_ticks: u64,
    },
}

/// Diagnostic emitted when coordinator truth has gone stale.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterStaleLeadershipDiagnostic {
    /// Node currently recorded as coordinator.
    pub leader_id: NodeId,
    /// Election term associated with the stale coordinator.
    pub term: ClusterTerm,
    /// Highest committed authoritative event visible to the stale coordinator.
    pub committed_event_index: ClusterEventIndex,
    /// Latest heartbeat tick recorded for the coordinator.
    pub heartbeat_observed_tick: ClusterLeaseTick,
    /// Tick at which the coordinator lease expired.
    pub expired_at_tick: ClusterLeaseTick,
    /// Tick that observed the staleness condition.
    pub observed_tick: ClusterLeaseTick,
}

/// Stable coordinator-authority truth derived from the current leadership record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCommitAuthority {
    /// Coordinator currently holding commit authority.
    pub leader_id: NodeId,
    /// Election term that fences this coordinator epoch.
    pub term: ClusterTerm,
    /// Highest committed global event index visible to the coordinator.
    pub committed_event_index: ClusterEventIndex,
    /// Stable fencing token that changes across coordinator turnover.
    pub fence_token: String,
    /// Stable digest of the effective coordinator-authority record.
    pub authority_digest: String,
}

/// Leader/coordinator fact recorded in authoritative cluster state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterLeadershipRecord {
    /// Election term for the leader.
    pub term: ClusterTerm,
    /// Node ID currently acting as the ordering authority.
    pub leader_id: NodeId,
    /// Highest committed global event index visible to the leader.
    pub committed_event_index: ClusterEventIndex,
    /// Explicit freshness window for the current coordinator record.
    #[serde(default)]
    pub lease: ClusterLeadershipLease,
}

impl ClusterLeadershipRecord {
    /// Creates a leader/coordinator record from explicit term and commit truth.
    #[must_use]
    pub fn new(
        term: ClusterTerm,
        leader_id: NodeId,
        committed_event_index: ClusterEventIndex,
    ) -> Self {
        Self {
            term,
            leader_id,
            committed_event_index,
            lease: ClusterLeadershipLease::default(),
        }
    }

    /// Applies an explicit lease policy at one observed heartbeat tick.
    #[must_use]
    pub fn with_lease_policy(
        mut self,
        heartbeat_observed_tick: ClusterLeaseTick,
        lease_policy: ClusterLeadershipLeasePolicy,
    ) -> Self {
        self.lease = ClusterLeadershipLease::from_policy(heartbeat_observed_tick, lease_policy);
        self
    }

    /// Renews coordinator freshness at one later observed heartbeat tick.
    #[must_use]
    pub fn renewed_at(
        mut self,
        heartbeat_observed_tick: ClusterLeaseTick,
        lease_policy: ClusterLeadershipLeasePolicy,
    ) -> Self {
        self.lease = ClusterLeadershipLease::from_policy(heartbeat_observed_tick, lease_policy);
        self
    }

    /// Returns whether the coordinator is still effective at one observed tick.
    #[must_use]
    pub const fn lease_status_at(
        &self,
        observed_tick: ClusterLeaseTick,
    ) -> ClusterLeadershipLeaseStatus {
        self.lease.status_at(observed_tick)
    }

    /// Returns a stale-leadership diagnostic when the coordinator lease expired.
    #[must_use]
    pub fn stale_diagnostic_at(
        &self,
        observed_tick: ClusterLeaseTick,
    ) -> Option<ClusterStaleLeadershipDiagnostic> {
        match self.lease_status_at(observed_tick) {
            ClusterLeadershipLeaseStatus::Active { .. } => None,
            ClusterLeadershipLeaseStatus::Stale {
                expired_at_tick, ..
            } => Some(ClusterStaleLeadershipDiagnostic {
                leader_id: self.leader_id.clone(),
                term: self.term,
                committed_event_index: self.committed_event_index,
                heartbeat_observed_tick: self.lease.heartbeat_observed_tick,
                expired_at_tick,
                observed_tick,
            }),
        }
    }

    /// Returns the stable coordinator fence token for this leadership record.
    #[must_use]
    pub fn fence_token(&self, cluster_id: &ClusterId) -> String {
        let mut hasher = Sha256::new();
        hasher.update(cluster_id.as_str().as_bytes());
        hasher.update(b"|commit_fence|");
        hasher.update(self.term.as_u64().to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.leader_id.as_str().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Returns a stable digest of the full coordinator-authority record.
    #[must_use]
    pub fn authority_digest(&self, cluster_id: &ClusterId) -> String {
        let mut hasher = Sha256::new();
        hasher.update(cluster_id.as_str().as_bytes());
        hasher.update(b"|commit_authority|");
        hasher.update(self.term.as_u64().to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.leader_id.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(self.committed_event_index.as_u64().to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(
            self.lease
                .heartbeat_observed_tick
                .as_u64()
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(self.lease.expires_at_tick.as_u64().to_string().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Returns the full coordinator-authority record derived from this leadership fact.
    #[must_use]
    pub fn commit_authority(&self, cluster_id: &ClusterId) -> ClusterCommitAuthority {
        ClusterCommitAuthority {
            leader_id: self.leader_id.clone(),
            term: self.term,
            committed_event_index: self.committed_event_index,
            fence_token: self.fence_token(cluster_id),
            authority_digest: self.authority_digest(cluster_id),
        }
    }
}

/// Recovery and compaction policy for the first authoritative cluster seam.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterRecoveryPolicy {
    /// Maximum ordered events returned in one catchup response.
    pub max_events_per_response: usize,
    /// Number of tail events retained after compaction.
    pub retain_tail_events: usize,
    /// Gap threshold beyond which a full snapshot install is preferred.
    pub full_resync_gap_threshold: usize,
}

impl Default for ClusterRecoveryPolicy {
    fn default() -> Self {
        Self {
            max_events_per_response: 128,
            retain_tail_events: 32,
            full_resync_gap_threshold: 512,
        }
    }
}

impl ClusterRecoveryPolicy {
    fn response_limit(&self, requested: usize) -> usize {
        if requested == 0 {
            self.max_events_per_response
        } else {
            requested.min(self.max_events_per_response)
        }
    }
}

/// Recovery disposition for one catchup or rejoin request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterRecoveryDisposition {
    /// Simple ordered-event catchup is sufficient.
    CatchUp,
    /// Install the provided snapshot, then optionally apply a retained tail.
    InstallSnapshot,
    /// Require a full resync from a current authoritative snapshot.
    FullResync,
}

/// Reason a snapshot install or full resync was selected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClusterRecoveryReason {
    /// Requested event history fell behind the current compaction floor.
    CompactionBoundary {
        /// Earliest event still directly available from the retained tail.
        compacted_through: Option<ClusterEventIndex>,
    },
    /// Requested event gap exceeded the configured resync threshold.
    GapThresholdExceeded {
        /// Requester's last applied event, when one exists.
        requested_from: Option<ClusterEventIndex>,
        /// Current authoritative log head, when one exists.
        head: Option<ClusterEventIndex>,
        /// Threshold that triggered full resync.
        threshold: usize,
    },
    /// Requester uses an incompatible or stale schema version.
    SchemaVersionMismatch {
        /// Current authoritative schema version.
        expected: ClusterSchemaVersion,
        /// Requester schema version.
        actual: ClusterSchemaVersion,
    },
}

/// Catchup request for missing ordered cluster history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCatchupRequest {
    /// Cluster the requester believes it belongs to.
    pub cluster_id: ClusterId,
    /// Node requesting recovery data.
    pub requester_id: NodeId,
    /// Highest event already applied by the requester, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_applied_event_index: Option<ClusterEventIndex>,
    /// Requester schema version for recovery compatibility.
    pub schema_version: ClusterSchemaVersion,
    /// Maximum events the requester is prepared to apply in one batch.
    pub max_events: usize,
}

impl ClusterCatchupRequest {
    /// Creates a catchup request for one node.
    #[must_use]
    pub fn new(
        cluster_id: ClusterId,
        requester_id: NodeId,
        last_applied_event_index: Option<ClusterEventIndex>,
        schema_version: ClusterSchemaVersion,
        max_events: usize,
    ) -> Self {
        Self {
            cluster_id,
            requester_id,
            last_applied_event_index,
            schema_version,
            max_events,
        }
    }
}

/// Recovery payload for a catchup response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClusterCatchupPayload {
    /// Requester can catch up by applying ordered events only.
    Events {
        /// Ordered authoritative events to apply.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        events: Vec<IndexedClusterEvent>,
    },
    /// Requester must install a snapshot, then optionally apply a retained tail.
    Snapshot {
        /// Authoritative snapshot to install first.
        snapshot: Box<ClusterSnapshot>,
        /// Ordered tail events to apply after installing the snapshot.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tail_events: Vec<IndexedClusterEvent>,
        /// Reason the snapshot path was selected.
        reason: ClusterRecoveryReason,
    },
}

/// Catchup response for a recovery or rejoin request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCatchupResponse {
    /// Cluster identity that owns the authoritative response.
    pub cluster_id: ClusterId,
    /// Requesting node receiving the response.
    pub requester_id: NodeId,
    /// Current authoritative head event index, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_event_index: Option<ClusterEventIndex>,
    /// Recovery disposition chosen for this request.
    pub disposition: ClusterRecoveryDisposition,
    /// Recovery payload.
    pub payload: ClusterCatchupPayload,
}

impl ClusterCatchupResponse {
    /// Returns a stable digest for one recovery payload.
    pub fn stable_digest(&self) -> Result<String, ClusterRecoveryEnvelopeError> {
        let encoded = serde_json::to_vec(self)
            .map_err(|error| ClusterRecoveryEnvelopeError::EnvelopeEncoding(error.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_catchup_response|");
        hasher.update(encoded);
        Ok(hex::encode(hasher.finalize()))
    }

    /// Signs one recovery payload for authenticated transport.
    pub fn sign(
        self,
        signer: ClusterNodeIdentity,
        signing_key: &SigningKey,
        authenticated_counter: u64,
    ) -> Result<AuthenticatedClusterCatchupResponse, ClusterRecoveryEnvelopeError> {
        if signer.cluster_id != self.cluster_id {
            return Err(ClusterRecoveryEnvelopeError::SignerClusterMismatch {
                expected: self.cluster_id,
                actual: signer.cluster_id,
            });
        }
        let payload_digest = self.stable_digest()?;
        let signing_payload =
            recovery_signing_payload(&signer, authenticated_counter, &payload_digest)?;
        let signature = signing_key.sign(&signing_payload);
        Ok(AuthenticatedClusterCatchupResponse {
            signer,
            authenticated_counter,
            payload_digest,
            response: self,
            signature_hex: hex::encode(signature.to_bytes()),
        })
    }
}

/// Signed recovery envelope for catchup or snapshot install.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthenticatedClusterCatchupResponse {
    /// Node identity that signed the recovery payload.
    pub signer: ClusterNodeIdentity,
    /// Monotonic authenticated counter for replay protection.
    pub authenticated_counter: u64,
    /// Stable digest of the signed recovery payload.
    pub payload_digest: String,
    /// Recovery payload being authenticated.
    pub response: ClusterCatchupResponse,
    /// Hex-encoded ed25519 signature over the envelope metadata.
    pub signature_hex: String,
}

impl AuthenticatedClusterCatchupResponse {
    /// Verifies cluster, requester, signer, digest, signature, and replay state.
    pub fn verify(
        &self,
        expected_cluster_id: &ClusterId,
        expected_requester_id: &NodeId,
        expected_signer: &ConfiguredClusterPeer,
        replay_window: &mut ClusterRecoveryReplayWindow,
    ) -> Result<(), ClusterRecoveryEnvelopeError> {
        if self.response.cluster_id != *expected_cluster_id {
            return Err(ClusterRecoveryEnvelopeError::ClusterIdMismatch {
                expected: expected_cluster_id.clone(),
                actual: self.response.cluster_id.clone(),
            });
        }
        if self.signer.cluster_id != *expected_cluster_id {
            return Err(ClusterRecoveryEnvelopeError::SignerClusterMismatch {
                expected: expected_cluster_id.clone(),
                actual: self.signer.cluster_id.clone(),
            });
        }
        if self.response.requester_id != *expected_requester_id {
            return Err(ClusterRecoveryEnvelopeError::RequesterIdMismatch {
                expected: expected_requester_id.clone(),
                actual: self.response.requester_id.clone(),
            });
        }
        if self.signer.node_id != expected_signer.node_id {
            return Err(ClusterRecoveryEnvelopeError::SignerNodeIdMismatch {
                expected: expected_signer.node_id.clone(),
                actual: self.signer.node_id.clone(),
            });
        }
        if self.signer.auth_public_key != expected_signer.auth_public_key {
            return Err(ClusterRecoveryEnvelopeError::SignerKeyMismatch {
                expected: expected_signer.auth_public_key.clone(),
                actual: self.signer.auth_public_key.clone(),
            });
        }
        let actual_payload_digest = self.response.stable_digest()?;
        if actual_payload_digest != self.payload_digest {
            return Err(ClusterRecoveryEnvelopeError::PayloadDigestMismatch {
                expected: self.payload_digest.clone(),
                actual: actual_payload_digest,
            });
        }
        let verifying_key = decode_recovery_verifying_key(&self.signer.auth_public_key)?;
        let signature = decode_recovery_signature(&self.signature_hex)?;
        let signing_payload = recovery_signing_payload(
            &self.signer,
            self.authenticated_counter,
            &self.payload_digest,
        )?;
        verifying_key
            .verify(&signing_payload, &signature)
            .map_err(|_| ClusterRecoveryEnvelopeError::SignatureVerificationFailed)?;
        replay_window.record(self.authenticated_counter)?;
        Ok(())
    }
}

/// Replay window for authenticated recovery envelopes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterRecoveryReplayWindow {
    /// Highest authenticated counter already accepted.
    pub highest_seen: u64,
    /// Sliding set of accepted counters still in-window.
    pub accepted_counters: BTreeSet<u64>,
    /// Maximum number of counters retained for replay checks.
    pub window_size: u64,
}

impl ClusterRecoveryReplayWindow {
    /// Creates a replay window with one explicit retention size.
    #[must_use]
    pub fn new(window_size: u64) -> Self {
        Self {
            highest_seen: 0,
            accepted_counters: BTreeSet::new(),
            window_size,
        }
    }

    /// Records one authenticated counter or returns replay refusal.
    pub fn record(
        &mut self,
        authenticated_counter: u64,
    ) -> Result<(), ClusterRecoveryEnvelopeError> {
        if self.window_size == 0 {
            return Ok(());
        }
        let oldest_allowed = self
            .highest_seen
            .saturating_sub(self.window_size.saturating_sub(1));
        if authenticated_counter < oldest_allowed
            || self.accepted_counters.contains(&authenticated_counter)
        {
            return Err(ClusterRecoveryEnvelopeError::ReplayDetected {
                highest_seen: self.highest_seen,
                attempted: authenticated_counter,
            });
        }
        self.accepted_counters.insert(authenticated_counter);
        self.highest_seen = self.highest_seen.max(authenticated_counter);
        while self.accepted_counters.len() as u64 > self.window_size {
            let _ = self.accepted_counters.pop_first();
        }
        Ok(())
    }
}

/// Verification failures for authenticated recovery envelopes.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ClusterRecoveryEnvelopeError {
    /// Recovery payload belongs to another cluster.
    #[error("recovery payload belongs to cluster {actual:?} but {expected:?} was expected")]
    ClusterIdMismatch {
        /// Expected cluster identity.
        expected: ClusterId,
        /// Observed recovery cluster identity.
        actual: ClusterId,
    },
    /// Signer identity belongs to another cluster.
    #[error("recovery signer belongs to cluster {actual:?} but {expected:?} was expected")]
    SignerClusterMismatch {
        /// Expected cluster identity.
        expected: ClusterId,
        /// Observed signer cluster identity.
        actual: ClusterId,
    },
    /// Recovery payload was signed for another requester.
    #[error("recovery payload targets requester {actual:?} but {expected:?} was expected")]
    RequesterIdMismatch {
        /// Expected requester ID.
        expected: NodeId,
        /// Observed requester ID.
        actual: NodeId,
    },
    /// Recovery payload was signed by an unexpected node.
    #[error("recovery payload was signed by node {actual:?} but {expected:?} was expected")]
    SignerNodeIdMismatch {
        /// Expected signer node ID.
        expected: NodeId,
        /// Observed signer node ID.
        actual: NodeId,
    },
    /// Recovery payload signer key did not match the configured trust bundle.
    #[error("recovery payload signer key mismatch")]
    SignerKeyMismatch {
        /// Expected signer key.
        expected: String,
        /// Observed signer key.
        actual: String,
    },
    /// Recovery payload digest no longer matches the signed response.
    #[error("recovery payload digest mismatch")]
    PayloadDigestMismatch {
        /// Expected signed digest.
        expected: String,
        /// Observed digest.
        actual: String,
    },
    /// Recovery signature could not be validated.
    #[error("recovery signature verification failed")]
    SignatureVerificationFailed,
    /// Recovery envelope could not be encoded for signing or verification.
    #[error("recovery envelope encoding failed: {0}")]
    EnvelopeEncoding(String),
    /// Recovery envelope replayed a prior authenticated counter.
    #[error(
        "recovery envelope replay detected: attempted {attempted}, highest seen {highest_seen}"
    )]
    ReplayDetected {
        /// Highest authenticated counter already accepted.
        highest_seen: u64,
        /// Counter attempted by the replayed envelope.
        attempted: u64,
    },
}

fn recovery_signing_payload(
    signer: &ClusterNodeIdentity,
    authenticated_counter: u64,
    payload_digest: &str,
) -> Result<Vec<u8>, ClusterRecoveryEnvelopeError> {
    serde_json::to_vec(&(signer, authenticated_counter, payload_digest))
        .map_err(|error| ClusterRecoveryEnvelopeError::EnvelopeEncoding(error.to_string()))
}

fn decode_recovery_verifying_key(
    auth_public_key: &str,
) -> Result<VerifyingKey, ClusterRecoveryEnvelopeError> {
    let bytes = hex::decode(auth_public_key)
        .map_err(|_| ClusterRecoveryEnvelopeError::SignatureVerificationFailed)?;
    let verifying_key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ClusterRecoveryEnvelopeError::SignatureVerificationFailed)?;
    VerifyingKey::from_bytes(&verifying_key_bytes)
        .map_err(|_| ClusterRecoveryEnvelopeError::SignatureVerificationFailed)
}

fn decode_recovery_signature(
    signature_hex: &str,
) -> Result<Signature, ClusterRecoveryEnvelopeError> {
    let bytes = hex::decode(signature_hex)
        .map_err(|_| ClusterRecoveryEnvelopeError::SignatureVerificationFailed)?;
    let signature_bytes: [u8; 64] = bytes
        .try_into()
        .map_err(|_| ClusterRecoveryEnvelopeError::SignatureVerificationFailed)?;
    Ok(Signature::from_bytes(&signature_bytes))
}

/// Imperative cluster command submitted before the leader orders a result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClusterCommand {
    /// Request that the leader reconcile one membership record.
    ReconcileMembership { membership: ClusterMembershipRecord },
    /// Request that the leader remove one node from authoritative state.
    RemoveMember { node_id: NodeId, reason: String },
    /// Request that the leader reconcile one discovered candidate record.
    ReconcileDiscoveryCandidate {
        candidate: ClusterDiscoveredCandidateRecord,
    },
    /// Request that the leader remove one discovered candidate from authoritative state.
    RemoveDiscoveryCandidate { node_id: NodeId, reason: String },
    /// Request that the leader promote one discovered candidate into explicit membership truth.
    AdmitDiscoveryCandidate {
        node_id: NodeId,
        advertised_addr: Option<SocketAddr>,
        membership_status: ClusterMembershipStatus,
    },
    /// Request that the leader reconcile one connection fact.
    ReconcileConnection { link: ClusterLink },
    /// Request that the leader reconcile one node telemetry record.
    ReconcileNodeTelemetry { telemetry: ClusterNodeTelemetry },
    /// Request that the leader reconcile one artifact-residency record.
    ReconcileArtifactResidency {
        residency: ClusterArtifactResidencyRecord,
    },
    /// Request that the leader advance authoritative leadership truth.
    UpdateLeadership {
        leader_id: NodeId,
        term: ClusterTerm,
    },
    /// Request missing ordered history from the current authority.
    RequestCatchup { request: ClusterCatchupRequest },
    /// Request an authoritative state snapshot digest.
    RequestSnapshot,
}

/// Required authority scope for one imperative cluster command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterCommandAuthorityScope {
    /// Only the current coordinator may authorize the mutation.
    CoordinatorOnly,
    /// Any allowed cluster member may authorize the command.
    ClusterMember,
    /// Only the node referenced by the command may authorize it.
    SelfNode,
    /// Only one endpoint referenced by the link may authorize it.
    LinkPeer,
    /// Only the node proposed as leader may authorize the update.
    ProposedLeader,
}

/// Replay-safe authorization policy for imperative cluster commands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCommandAuthorizationPolicy {
    /// Whether joining members may submit cluster commands.
    pub allow_joining_members: bool,
    /// Whether draining members may submit cluster commands.
    pub allow_draining_members: bool,
    /// Whether offline members may submit cluster commands.
    pub allow_offline_members: bool,
    /// Whether the current coordinator may override narrower command scopes.
    pub allow_coordinator_override: bool,
}

impl ClusterCommandAuthorizationPolicy {
    /// Default policy for operator-managed clusters.
    #[must_use]
    pub const fn operator_managed_default() -> Self {
        Self {
            allow_joining_members: true,
            allow_draining_members: false,
            allow_offline_members: false,
            allow_coordinator_override: true,
        }
    }

    /// Returns a stable digest for this authorization policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_json_digest("cluster_command_authorization_policy", self)
    }

    fn permits_status(&self, status: ClusterMembershipStatus) -> bool {
        match status {
            ClusterMembershipStatus::Ready => true,
            ClusterMembershipStatus::Joining => self.allow_joining_members,
            ClusterMembershipStatus::Draining => self.allow_draining_members,
            ClusterMembershipStatus::Offline => self.allow_offline_members,
        }
    }
}

impl Default for ClusterCommandAuthorizationPolicy {
    fn default() -> Self {
        Self::operator_managed_default()
    }
}

/// Stable refusal code for one cluster-command authorization failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterCommandAuthorizationRefusalCode {
    /// Submitter identity is not known well enough to authorize the command.
    SubmitterUnknown,
    /// Submitter membership status is not currently allowed by policy.
    SubmitterMembershipNotAllowed,
    /// Current coordinator authority was required but not present.
    CoordinatorRequired,
    /// Command targeted a different node than the submitter.
    TargetNodeMismatch,
    /// Connection command submitter was not one endpoint of the link.
    LinkPeerMismatch,
    /// Leadership update submitter did not match the proposed leader.
    ProposedLeaderMismatch,
}

/// Machine-checkable authorization refusal for one cluster command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCommandAuthorizationRefusal {
    /// Stable refusal code.
    pub code: ClusterCommandAuthorizationRefusalCode,
    /// Node that attempted to submit the command.
    pub submitter_node_id: NodeId,
    /// Required scope for the refused command.
    pub required_scope: ClusterCommandAuthorityScope,
    /// Stable digest of the refused command.
    pub command_digest: String,
    /// Stable digest of the policy used for the decision.
    pub authorization_policy_digest: String,
    /// Plain-language refusal detail.
    pub detail: String,
}

impl ClusterCommandAuthorizationRefusal {
    fn new(
        code: ClusterCommandAuthorizationRefusalCode,
        submitter_node_id: NodeId,
        required_scope: ClusterCommandAuthorityScope,
        command_digest: String,
        authorization_policy_digest: String,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code,
            submitter_node_id,
            required_scope,
            command_digest,
            authorization_policy_digest,
            detail: detail.into(),
        }
    }
}

/// Authorization failure for one cluster command.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ClusterCommandAuthorizationError {
    /// Command submission was refused by policy.
    #[error("cluster command authorization refused: {refusal:?}")]
    Refused {
        /// Machine-checkable refusal detail.
        refusal: ClusterCommandAuthorizationRefusal,
    },
}

/// Machine-checkable authorization fact for one imperative cluster command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCommandAuthorization {
    /// Stable digest of the command payload.
    pub command_digest: String,
    /// Stable digest of the policy used for the authorization decision.
    pub authorization_policy_digest: String,
    /// Required authority scope for the command family.
    pub authority_scope: ClusterCommandAuthorityScope,
    /// Node that submitted the command.
    pub submitter_node_id: NodeId,
    /// Role recorded for the submitter at authorization time.
    pub submitter_role: crate::NodeRole,
    /// Membership status recorded for the submitter at authorization time.
    pub submitter_membership_status: ClusterMembershipStatus,
    /// Coordinator authority visible at authorization time, when one existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinator_authority: Option<ClusterCommitAuthority>,
}

impl ClusterCommandAuthorization {
    fn new(
        command_digest: String,
        authorization_policy_digest: String,
        authority_scope: ClusterCommandAuthorityScope,
        submitter_node_id: NodeId,
        submitter_role: crate::NodeRole,
        submitter_membership_status: ClusterMembershipStatus,
        coordinator_authority: Option<ClusterCommitAuthority>,
    ) -> Self {
        Self {
            command_digest,
            authorization_policy_digest,
            authority_scope,
            submitter_node_id,
            submitter_role,
            submitter_membership_status,
            coordinator_authority,
        }
    }

    /// Returns a stable digest for this authorization fact.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_json_digest("cluster_command_authorization", self)
    }
}

impl ClusterCommand {
    /// Returns the required authority scope for this command family.
    #[must_use]
    pub const fn authority_scope(&self) -> ClusterCommandAuthorityScope {
        match self {
            Self::ReconcileMembership { .. }
            | Self::ReconcileNodeTelemetry { .. }
            | Self::ReconcileArtifactResidency { .. }
            | Self::RequestCatchup { .. } => ClusterCommandAuthorityScope::SelfNode,
            Self::ReconcileDiscoveryCandidate { .. }
            | Self::RemoveDiscoveryCandidate { .. }
            | Self::AdmitDiscoveryCandidate { .. }
            | Self::RemoveMember { .. } => ClusterCommandAuthorityScope::CoordinatorOnly,
            Self::ReconcileConnection { .. } => ClusterCommandAuthorityScope::LinkPeer,
            Self::UpdateLeadership { .. } => ClusterCommandAuthorityScope::ProposedLeader,
            Self::RequestSnapshot => ClusterCommandAuthorityScope::ClusterMember,
        }
    }

    /// Returns a stable digest for this command payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_json_digest("cluster_command", self)
    }
}

/// Local non-authoritative fact produced by one node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LocalClusterEvent {
    /// Local identity became available for control-plane use.
    LocalIdentityLoaded { identity: ClusterNodeIdentity },
    /// One peer was observed by the local transport.
    PeerObserved { peer: PeerSnapshot },
    /// One connection fact was observed locally.
    ConnectionObserved { fact: ClusterConnectionFact },
    /// One local telemetry fact was observed.
    TelemetryObserved { telemetry: ClusterNodeTelemetry },
    /// One local artifact residency fact was observed.
    ArtifactResidencyObserved {
        residency: ClusterArtifactResidencyRecord,
    },
    /// One local refusal was recorded.
    JoinRefusalObserved { refusal: ClusterJoinRefusal },
}

/// Local connection fact kept separate from authoritative global ordering.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClusterConnectionFact {
    /// A seed peer is configured for initial discovery.
    SeedPeerConfigured { remote_addr: SocketAddr },
    /// A peer became reachable from the local node.
    PeerConnected {
        local_node_id: NodeId,
        remote_node_id: NodeId,
        remote_addr: SocketAddr,
    },
    /// A peer became unreachable from the local node.
    PeerDisconnected {
        local_node_id: NodeId,
        remote_node_id: NodeId,
        remote_addr: SocketAddr,
    },
}

/// Typed leader-election traffic kept separate from ordered global facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClusterElectionMessage {
    /// Candidate requests votes for one term.
    RequestVotes {
        term: ClusterTerm,
        candidate_id: NodeId,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_applied_event_index: Option<ClusterEventIndex>,
    },
    /// Voter grants a vote to one candidate.
    GrantVote {
        term: ClusterTerm,
        candidate_id: NodeId,
        voter_id: NodeId,
    },
    /// Leader announces current authority and commit point.
    LeaderHeartbeat {
        term: ClusterTerm,
        leader_id: NodeId,
        #[serde(skip_serializing_if = "Option::is_none")]
        committed_event_index: Option<ClusterEventIndex>,
    },
}

/// Machine-checkable vote-conflict diagnostic for one election term.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterVoteConflictDiagnostic {
    /// Election term that observed the conflict.
    pub term: ClusterTerm,
    /// Voter that attempted to grant two different candidates in one term.
    pub voter_id: NodeId,
    /// Candidate that already held this voter's term grant.
    pub current_candidate_id: NodeId,
    /// Candidate that was attempted after the first grant.
    pub attempted_candidate_id: NodeId,
}

/// Machine-checkable split-brain diagnostic for one election term.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterSplitBrainDiagnostic {
    /// Election term with conflicting leader claims.
    pub term: ClusterTerm,
    /// Current leader already recorded for the term.
    pub current_leader_id: NodeId,
    /// New conflicting leader claim attempted for the same term.
    pub attempted_leader_id: NodeId,
}

/// Election-ledger refusal for conflicting vote or leader claims.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClusterElectionError {
    /// One voter tried to grant different candidates within one term.
    #[error("conflicting vote grant in term {diagnostic:?}")]
    ConflictingVote {
        /// Machine-checkable vote-conflict detail.
        diagnostic: ClusterVoteConflictDiagnostic,
    },
    /// One term observed two different leader claims.
    #[error("split-brain leader claim in term {diagnostic:?}")]
    SplitBrainLeaderClaim {
        /// Machine-checkable same-term conflicting-leader detail.
        diagnostic: ClusterSplitBrainDiagnostic,
    },
}

/// Election truth for one term kept separate from authoritative ordered facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterElectionTermLedger {
    /// Term this ledger row belongs to.
    pub term: ClusterTerm,
    /// Candidates that requested votes in this term.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub requested_candidates: BTreeSet<NodeId>,
    /// Vote grants keyed by voter ID.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub granted_votes: BTreeMap<NodeId, NodeId>,
    /// Current leader claim for this term, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader_id: Option<NodeId>,
}

impl ClusterElectionTermLedger {
    /// Creates an empty ledger for one term.
    #[must_use]
    pub fn new(term: ClusterTerm) -> Self {
        Self {
            term,
            requested_candidates: BTreeSet::new(),
            granted_votes: BTreeMap::new(),
            leader_id: None,
        }
    }

    /// Records that one candidate requested votes for this term.
    pub fn record_vote_request(&mut self, candidate_id: NodeId) {
        self.requested_candidates.insert(candidate_id);
    }

    /// Records one vote grant and refuses conflicting grants by the same voter.
    pub fn record_vote_grant(
        &mut self,
        candidate_id: NodeId,
        voter_id: NodeId,
    ) -> Result<(), ClusterElectionError> {
        match self.granted_votes.get(&voter_id) {
            Some(current_candidate_id) if current_candidate_id != &candidate_id => {
                Err(ClusterElectionError::ConflictingVote {
                    diagnostic: ClusterVoteConflictDiagnostic {
                        term: self.term,
                        voter_id,
                        current_candidate_id: current_candidate_id.clone(),
                        attempted_candidate_id: candidate_id,
                    },
                })
            }
            _ => {
                self.granted_votes.insert(voter_id, candidate_id);
                Ok(())
            }
        }
    }

    /// Records one leader claim and refuses same-term conflicting claims.
    pub fn record_leader_heartbeat(
        &mut self,
        leader_id: NodeId,
    ) -> Result<(), ClusterElectionError> {
        match self.leader_id.as_ref() {
            Some(current_leader_id) if current_leader_id != &leader_id => {
                Err(ClusterElectionError::SplitBrainLeaderClaim {
                    diagnostic: ClusterSplitBrainDiagnostic {
                        term: self.term,
                        current_leader_id: current_leader_id.clone(),
                        attempted_leader_id: leader_id,
                    },
                })
            }
            _ => {
                self.leader_id = Some(leader_id);
                Ok(())
            }
        }
    }

    /// Returns how many voters granted one candidate in this term.
    #[must_use]
    pub fn votes_for(&self, candidate_id: &NodeId) -> usize {
        self.granted_votes
            .values()
            .filter(|current_candidate_id| *current_candidate_id == candidate_id)
            .count()
    }
}

/// Multi-term ledger for typed election traffic.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterElectionLedger {
    /// Election truth keyed by term.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub terms: BTreeMap<ClusterTerm, ClusterElectionTermLedger>,
}

impl ClusterElectionLedger {
    /// Creates an empty multi-term election ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns election truth for one explicit term.
    #[must_use]
    pub fn term(&self, term: ClusterTerm) -> Option<&ClusterElectionTermLedger> {
        self.terms.get(&term)
    }

    /// Observes one typed election message and updates the per-term ledger.
    pub fn observe(
        &mut self,
        message: &ClusterElectionMessage,
    ) -> Result<(), ClusterElectionError> {
        match message {
            ClusterElectionMessage::RequestVotes {
                term, candidate_id, ..
            } => {
                self.terms
                    .entry(*term)
                    .or_insert_with(|| ClusterElectionTermLedger::new(*term))
                    .record_vote_request(candidate_id.clone());
                Ok(())
            }
            ClusterElectionMessage::GrantVote {
                term,
                candidate_id,
                voter_id,
            } => self
                .terms
                .entry(*term)
                .or_insert_with(|| ClusterElectionTermLedger::new(*term))
                .record_vote_grant(candidate_id.clone(), voter_id.clone()),
            ClusterElectionMessage::LeaderHeartbeat {
                term, leader_id, ..
            } => self
                .terms
                .entry(*term)
                .or_insert_with(|| ClusterElectionTermLedger::new(*term))
                .record_leader_heartbeat(leader_id.clone()),
        }
    }
}

/// Authoritative global cluster event ordered by one leader.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClusterEvent {
    /// Insert or replace one cluster membership record.
    MembershipReconciled { membership: ClusterMembershipRecord },
    /// Remove one node from authoritative state.
    MembershipRemoved { node_id: NodeId, reason: String },
    /// Insert or replace one discovered candidate record.
    DiscoveryCandidateReconciled {
        candidate: ClusterDiscoveredCandidateRecord,
    },
    /// Remove one discovered candidate from authoritative state.
    DiscoveryCandidateRemoved { node_id: NodeId, reason: String },
    /// Promote one discovered candidate into explicit membership truth.
    DiscoveryCandidateAdmitted {
        node_id: NodeId,
        membership: ClusterMembershipRecord,
    },
    /// Insert or replace one connection/link fact.
    ConnectionReconciled { link: ClusterLink },
    /// Remove one connection/link fact.
    ConnectionRemoved { key: ClusterLinkKey },
    /// Insert or replace one telemetry fact.
    NodeTelemetryReconciled { telemetry: ClusterNodeTelemetry },
    /// Insert or replace one artifact-residency fact.
    ArtifactResidencyReconciled {
        residency: ClusterArtifactResidencyRecord,
    },
    /// Remove one artifact-residency fact.
    ArtifactResidencyRemoved { key: ClusterArtifactResidencyKey },
    /// Insert or replace leader/coordinator truth.
    LeadershipReconciled { leadership: ClusterLeadershipRecord },
}

/// One authoritative global cluster event with its contiguous log index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedClusterEvent {
    /// Cluster identity that owns this event stream.
    pub cluster_id: ClusterId,
    /// Contiguous authoritative event index.
    pub index: ClusterEventIndex,
    /// Ordered event payload.
    pub event: ClusterEvent,
    /// Authorization provenance for the command that produced this event, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_authorization: Option<ClusterCommandAuthorization>,
}

impl IndexedClusterEvent {
    /// Creates one indexed authoritative cluster event.
    #[must_use]
    pub fn new(cluster_id: ClusterId, index: ClusterEventIndex, event: ClusterEvent) -> Self {
        Self {
            cluster_id,
            index,
            event,
            command_authorization: None,
        }
    }

    /// Attaches command-authorization provenance to this ordered event.
    #[must_use]
    pub fn with_command_authorization(
        mut self,
        command_authorization: ClusterCommandAuthorization,
    ) -> Self {
        self.command_authorization = Some(command_authorization);
        self
    }
}

/// Authoritative cluster snapshot derived from the ordered event history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterSnapshot {
    /// Cluster identity the snapshot belongs to.
    pub cluster_id: ClusterId,
    /// Explicit schema version for this snapshot.
    pub schema_version: ClusterSchemaVersion,
    /// Highest authoritative event applied into this snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_applied_event_index: Option<ClusterEventIndex>,
    /// Current cluster membership by node ID.
    pub memberships: BTreeMap<NodeId, ClusterMembershipRecord>,
    /// Command provenance for the current membership map.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub membership_provenance: BTreeMap<NodeId, ClusterCommandAuthorization>,
    /// Current discovered wider-network candidates by node ID.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub discovery_candidates: BTreeMap<NodeId, ClusterDiscoveredCandidateRecord>,
    /// Command provenance for the current discovery-candidate map.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub discovery_candidate_provenance: BTreeMap<NodeId, ClusterCommandAuthorization>,
    /// Current cluster links by canonical node pair.
    pub links: BTreeMap<ClusterLinkKey, ClusterLink>,
    /// Command provenance for the current link map.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub link_provenance: BTreeMap<ClusterLinkKey, ClusterCommandAuthorization>,
    /// Current node telemetry by node ID.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub telemetry: BTreeMap<NodeId, ClusterNodeTelemetry>,
    /// Command provenance for current node telemetry.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub telemetry_provenance: BTreeMap<NodeId, ClusterCommandAuthorization>,
    /// Current artifact residency/staging facts by node and artifact.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub artifact_residency: BTreeMap<ClusterArtifactResidencyKey, ClusterArtifactResidencyRecord>,
    /// Command provenance for current artifact residency facts.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub artifact_residency_provenance:
        BTreeMap<ClusterArtifactResidencyKey, ClusterCommandAuthorization>,
    /// Current leader/coordinator truth, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leadership: Option<ClusterLeadershipRecord>,
    /// Command provenance for the current leader/coordinator fact, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leadership_provenance: Option<ClusterCommandAuthorization>,
}

impl ClusterSnapshot {
    /// Creates an empty cluster snapshot for one cluster identity.
    #[must_use]
    pub fn new(cluster_id: ClusterId) -> Self {
        Self::new_with_schema(cluster_id, ClusterSchemaVersion::initial())
    }

    /// Creates an empty cluster snapshot with an explicit schema version.
    #[must_use]
    pub fn new_with_schema(cluster_id: ClusterId, schema_version: ClusterSchemaVersion) -> Self {
        Self {
            cluster_id,
            schema_version,
            last_applied_event_index: None,
            memberships: BTreeMap::new(),
            membership_provenance: BTreeMap::new(),
            discovery_candidates: BTreeMap::new(),
            discovery_candidate_provenance: BTreeMap::new(),
            links: BTreeMap::new(),
            link_provenance: BTreeMap::new(),
            telemetry: BTreeMap::new(),
            telemetry_provenance: BTreeMap::new(),
            artifact_residency: BTreeMap::new(),
            artifact_residency_provenance: BTreeMap::new(),
            leadership: None,
            leadership_provenance: None,
        }
    }

    /// Returns a stable digest of the topology-relevant portion of cluster state.
    #[must_use]
    pub fn topology_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(b"|schema|");
        hasher.update(self.schema_version.major.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.schema_version.minor.to_string().as_bytes());
        for membership in self.memberships.values() {
            hasher.update(b"|membership|");
            hasher.update(membership.identity.node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(membership.identity.cluster_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(
                membership
                    .identity
                    .node_epoch
                    .as_u64()
                    .to_string()
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(node_role_label(membership.identity.role));
            hasher.update(b"|");
            hasher.update(membership_status_label(membership.status));
            hasher.update(b"|");
            hasher.update(
                membership
                    .advertised_addr
                    .map_or(String::new(), |addr| addr.to_string())
                    .as_bytes(),
            );
        }
        for link in self.links.values() {
            hasher.update(b"|link|");
            hasher.update(link.key.left_node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(link.key.right_node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(transport_class_label(link.transport));
            hasher.update(b"|");
            hasher.update(link_status_label(link.status));
            hasher.update(b"|");
            hasher.update(link_class_label(link.link_class));
            hasher.update(b"|");
            hasher.update(
                link.latency_us
                    .map_or(String::new(), |value| value.to_string())
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(
                link.bandwidth_mbps
                    .map_or(String::new(), |value| value.to_string())
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(stability_label(link.stability));
        }
        for telemetry in self.telemetry.values() {
            hasher.update(b"|telemetry|");
            hasher.update(telemetry.node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(
                telemetry
                    .total_memory_bytes
                    .map_or(String::new(), |value| value.to_string())
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(
                telemetry
                    .free_memory_bytes
                    .map_or(String::new(), |value| value.to_string())
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(
                telemetry
                    .cpu_logical_cores
                    .map_or(String::new(), |value| value.to_string())
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(
                telemetry
                    .accelerator_count
                    .map_or(String::new(), |value| value.to_string())
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(stability_label(telemetry.stability));
            for (backend, readiness) in &telemetry.backend_readiness {
                hasher.update(b"|backend|");
                hasher.update(backend.as_bytes());
                hasher.update(b"|");
                hasher.update(backend_readiness_label(*readiness));
            }
        }
        hex::encode(hasher.finalize())
    }

    /// Returns a stable digest of the full authoritative cluster snapshot.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(b"|snapshot|");
        hasher.update(self.topology_digest().as_bytes());
        hasher.update(b"|");
        hasher.update(
            self.last_applied_event_index
                .map_or(0, ClusterEventIndex::as_u64)
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(self.discovery_candidate_digest().as_bytes());
        hasher.update(self.artifact_residency_digest().as_bytes());
        hasher.update(b"|");
        for (node_id, authorization) in &self.membership_provenance {
            hasher.update(b"|membership_provenance|");
            hasher.update(node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(authorization.stable_digest().as_bytes());
        }
        for (node_id, authorization) in &self.discovery_candidate_provenance {
            hasher.update(b"|discovery_candidate_provenance|");
            hasher.update(node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(authorization.stable_digest().as_bytes());
        }
        for (link_key, authorization) in &self.link_provenance {
            hasher.update(b"|link_provenance|");
            hasher.update(link_key.left_node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(link_key.right_node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(authorization.stable_digest().as_bytes());
        }
        for (node_id, authorization) in &self.telemetry_provenance {
            hasher.update(b"|telemetry_provenance|");
            hasher.update(node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(authorization.stable_digest().as_bytes());
        }
        for (residency_key, authorization) in &self.artifact_residency_provenance {
            hasher.update(b"|artifact_residency_provenance|");
            hasher.update(residency_key.node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(residency_key.artifact_digest.as_bytes());
            hasher.update(b"|");
            hasher.update(authorization.stable_digest().as_bytes());
        }
        if let Some(authorization) = &self.leadership_provenance {
            hasher.update(b"|leadership_provenance|");
            hasher.update(authorization.stable_digest().as_bytes());
            hasher.update(b"|");
        }
        if let Some(leadership) = &self.leadership {
            hasher.update(b"|leadership|");
            hasher.update(leadership.term.as_u64().to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(leadership.leader_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(
                leadership
                    .committed_event_index
                    .as_u64()
                    .to_string()
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(
                leadership
                    .lease
                    .heartbeat_observed_tick
                    .as_u64()
                    .to_string()
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(
                leadership
                    .lease
                    .expires_at_tick
                    .as_u64()
                    .to_string()
                    .as_bytes(),
            );
        }
        hex::encode(hasher.finalize())
    }

    /// Returns a stable digest of artifact residency and staging facts only.
    #[must_use]
    pub fn artifact_residency_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(b"|artifact_residency|");
        for residency in self.artifact_residency.values() {
            hasher.update(b"|artifact|");
            hasher.update(residency.key.node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(residency.artifact.artifact_id.as_bytes());
            hasher.update(b"|");
            hasher.update(residency.artifact.artifact_digest.as_bytes());
            hasher.update(b"|");
            hasher.update(
                residency
                    .artifact
                    .provenance_digest
                    .as_deref()
                    .unwrap_or_default()
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(
                residency
                    .artifact
                    .governance_digest
                    .as_deref()
                    .unwrap_or_default()
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(
                residency
                    .artifact
                    .supply_policy_digest
                    .as_deref()
                    .unwrap_or_default()
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(artifact_residency_label(residency.status));
            hasher.update(b"|");
            hasher.update(
                residency
                    .transfer_method
                    .map_or(b"".as_slice(), artifact_transfer_label),
            );
            hasher.update(b"|");
            hasher.update(residency.detail.as_deref().unwrap_or_default().as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Returns a stable digest of discovery-candidate facts only.
    #[must_use]
    pub fn discovery_candidate_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(b"|discovery_candidates|");
        for candidate in self.discovery_candidates.values() {
            hasher.update(b"|candidate|");
            hasher.update(
                stable_json_digest("cluster_discovered_candidate_record", candidate).as_bytes(),
            );
        }
        hex::encode(hasher.finalize())
    }
}

/// Deterministic cluster state rebuilt from authoritative global events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterState {
    snapshot: ClusterSnapshot,
}

impl ClusterState {
    /// Creates empty authoritative cluster state for one cluster identity.
    #[must_use]
    pub fn new(cluster_id: ClusterId) -> Self {
        Self::new_with_schema(cluster_id, ClusterSchemaVersion::initial())
    }

    /// Creates empty authoritative cluster state with an explicit schema version.
    #[must_use]
    pub fn new_with_schema(cluster_id: ClusterId, schema_version: ClusterSchemaVersion) -> Self {
        Self {
            snapshot: ClusterSnapshot::new_with_schema(cluster_id, schema_version),
        }
    }

    /// Rehydrates authoritative cluster state from one prior snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: ClusterSnapshot) -> Self {
        Self { snapshot }
    }

    /// Rehydrates authoritative state from a snapshot plus retained tail events.
    pub fn recover(
        snapshot: ClusterSnapshot,
        tail_events: &[IndexedClusterEvent],
    ) -> Result<Self, ClusterHistoryError> {
        let mut state = Self::from_snapshot(snapshot);
        for event in tail_events {
            state.apply(event.clone())?;
        }
        Ok(state)
    }

    /// Returns the cluster identity owned by this state.
    #[must_use]
    pub fn cluster_id(&self) -> &ClusterId {
        &self.snapshot.cluster_id
    }

    /// Returns the schema version for this state.
    #[must_use]
    pub const fn schema_version(&self) -> ClusterSchemaVersion {
        self.snapshot.schema_version
    }

    /// Returns the highest event index applied into this state.
    #[must_use]
    pub const fn last_applied_event_index(&self) -> Option<ClusterEventIndex> {
        self.snapshot.last_applied_event_index
    }

    /// Returns the current cluster membership map.
    #[must_use]
    pub fn memberships(&self) -> &BTreeMap<NodeId, ClusterMembershipRecord> {
        &self.snapshot.memberships
    }

    /// Returns the current discovered-candidate map.
    #[must_use]
    pub fn discovery_candidates(&self) -> &BTreeMap<NodeId, ClusterDiscoveredCandidateRecord> {
        &self.snapshot.discovery_candidates
    }

    /// Returns the current cluster link map.
    #[must_use]
    pub fn links(&self) -> &BTreeMap<ClusterLinkKey, ClusterLink> {
        &self.snapshot.links
    }

    /// Returns the current node telemetry map.
    #[must_use]
    pub fn telemetry(&self) -> &BTreeMap<NodeId, ClusterNodeTelemetry> {
        &self.snapshot.telemetry
    }

    /// Returns current artifact residency and staging facts.
    #[must_use]
    pub fn artifact_residency(
        &self,
    ) -> &BTreeMap<ClusterArtifactResidencyKey, ClusterArtifactResidencyRecord> {
        &self.snapshot.artifact_residency
    }

    /// Returns the command provenance for one membership record, when known.
    #[must_use]
    pub fn membership_provenance(&self, node_id: &NodeId) -> Option<&ClusterCommandAuthorization> {
        self.snapshot.membership_provenance.get(node_id)
    }

    /// Returns the command provenance for one discovered candidate, when known.
    #[must_use]
    pub fn discovery_candidate_provenance(
        &self,
        node_id: &NodeId,
    ) -> Option<&ClusterCommandAuthorization> {
        self.snapshot.discovery_candidate_provenance.get(node_id)
    }

    /// Returns the command provenance for one link record, when known.
    #[must_use]
    pub fn link_provenance(&self, key: &ClusterLinkKey) -> Option<&ClusterCommandAuthorization> {
        self.snapshot.link_provenance.get(key)
    }

    /// Returns the command provenance for one telemetry record, when known.
    #[must_use]
    pub fn telemetry_provenance(&self, node_id: &NodeId) -> Option<&ClusterCommandAuthorization> {
        self.snapshot.telemetry_provenance.get(node_id)
    }

    /// Returns the command provenance for one artifact-residency record, when known.
    #[must_use]
    pub fn artifact_residency_provenance(
        &self,
        key: &ClusterArtifactResidencyKey,
    ) -> Option<&ClusterCommandAuthorization> {
        self.snapshot.artifact_residency_provenance.get(key)
    }

    /// Returns the current leader/coordinator record, when one exists.
    #[must_use]
    pub fn leadership(&self) -> Option<&ClusterLeadershipRecord> {
        self.snapshot.leadership.as_ref()
    }

    /// Returns the command provenance for the current leadership record, when known.
    #[must_use]
    pub fn leadership_provenance(&self) -> Option<&ClusterCommandAuthorization> {
        self.snapshot.leadership_provenance.as_ref()
    }

    /// Returns the current leadership lease status, when one leader exists.
    #[must_use]
    pub fn leadership_lease_status_at(
        &self,
        observed_tick: ClusterLeaseTick,
    ) -> Option<ClusterLeadershipLeaseStatus> {
        self.snapshot
            .leadership
            .as_ref()
            .map(|leadership| leadership.lease_status_at(observed_tick))
    }

    /// Returns the effective coordinator at one observed tick.
    #[must_use]
    pub fn effective_leadership_at(
        &self,
        observed_tick: ClusterLeaseTick,
    ) -> Option<&ClusterLeadershipRecord> {
        self.snapshot.leadership.as_ref().filter(|leadership| {
            matches!(
                leadership.lease_status_at(observed_tick),
                ClusterLeadershipLeaseStatus::Active { .. }
            )
        })
    }

    /// Returns a stale-leadership diagnostic when the recorded coordinator expired.
    #[must_use]
    pub fn stale_leadership_diagnostic_at(
        &self,
        observed_tick: ClusterLeaseTick,
    ) -> Option<ClusterStaleLeadershipDiagnostic> {
        self.snapshot
            .leadership
            .as_ref()
            .and_then(|leadership| leadership.stale_diagnostic_at(observed_tick))
    }

    /// Returns current coordinator-authority truth derived from leadership.
    #[must_use]
    pub fn commit_authority(&self) -> Option<ClusterCommitAuthority> {
        self.snapshot
            .leadership
            .as_ref()
            .map(|leadership| leadership.commit_authority(self.cluster_id()))
    }

    /// Returns a cloned authoritative snapshot of the current state.
    #[must_use]
    pub fn snapshot(&self) -> ClusterSnapshot {
        self.snapshot.clone()
    }

    /// Returns a stable digest of the topology-relevant portion of state.
    #[must_use]
    pub fn topology_digest(&self) -> String {
        self.snapshot.topology_digest()
    }

    /// Returns a stable digest of the current authoritative state.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        self.snapshot.stable_digest()
    }

    /// Returns a stable digest of artifact residency and staging facts only.
    #[must_use]
    pub fn artifact_residency_digest(&self) -> String {
        self.snapshot.artifact_residency_digest()
    }

    /// Authorizes one imperative cluster command against the current authoritative state.
    pub fn authorize_command(
        &self,
        submitter_node_id: &NodeId,
        command: &ClusterCommand,
        policy: &ClusterCommandAuthorizationPolicy,
    ) -> Result<ClusterCommandAuthorization, ClusterCommandAuthorizationError> {
        let required_scope = command.authority_scope();
        let command_digest = command.stable_digest();
        let authorization_policy_digest = policy.stable_digest();
        let coordinator_authority = self.commit_authority();

        if policy.allow_coordinator_override
            && let Some(current_coordinator) = coordinator_authority.as_ref()
            && &current_coordinator.leader_id == submitter_node_id
        {
            let submitter = self.membership_submitter(submitter_node_id).ok_or_else(|| {
                refused_command_authorization(
                    ClusterCommandAuthorizationRefusalCode::SubmitterUnknown,
                    submitter_node_id.clone(),
                    required_scope,
                    command_digest.clone(),
                    authorization_policy_digest.clone(),
                    format!(
                        "submitter `{}` holds coordinator authority but has no current membership record",
                        submitter_node_id.as_str()
                    ),
                )
            })?;
            if !policy.permits_status(submitter.membership_status) {
                return Err(refused_command_authorization(
                    ClusterCommandAuthorizationRefusalCode::SubmitterMembershipNotAllowed,
                    submitter_node_id.clone(),
                    required_scope,
                    command_digest,
                    authorization_policy_digest,
                    format!(
                        "submitter `{}` has membership status `{:?}` which policy does not allow",
                        submitter_node_id.as_str(),
                        submitter.membership_status
                    ),
                ));
            }
            return Ok(ClusterCommandAuthorization::new(
                command.stable_digest(),
                policy.stable_digest(),
                required_scope,
                submitter.node_id,
                submitter.role,
                submitter.membership_status,
                coordinator_authority,
            ));
        }

        let submitter = self
            .command_submitter(submitter_node_id, command)
            .ok_or_else(|| {
                refused_command_authorization(
                    ClusterCommandAuthorizationRefusalCode::SubmitterUnknown,
                    submitter_node_id.clone(),
                    required_scope,
                    command_digest.clone(),
                    authorization_policy_digest.clone(),
                    format!(
                        "submitter `{}` is not known well enough to authorize `{required_scope:?}`",
                        submitter_node_id.as_str()
                    ),
                )
            })?;
        if !policy.permits_status(submitter.membership_status) {
            return Err(refused_command_authorization(
                ClusterCommandAuthorizationRefusalCode::SubmitterMembershipNotAllowed,
                submitter_node_id.clone(),
                required_scope,
                command_digest,
                authorization_policy_digest,
                format!(
                    "submitter `{}` has membership status `{:?}` which policy does not allow",
                    submitter_node_id.as_str(),
                    submitter.membership_status
                ),
            ));
        }

        match command {
            ClusterCommand::ReconcileMembership { membership } => {
                if submitter.node_id != membership.identity.node_id {
                    return Err(refused_command_authorization(
                        ClusterCommandAuthorizationRefusalCode::TargetNodeMismatch,
                        submitter_node_id.clone(),
                        required_scope,
                        command.stable_digest(),
                        policy.stable_digest(),
                        format!(
                            "submitter `{}` may only reconcile its own membership record, not `{}`",
                            submitter_node_id.as_str(),
                            membership.identity.node_id.as_str()
                        ),
                    ));
                }
            }
            ClusterCommand::RemoveMember { .. }
            | ClusterCommand::ReconcileDiscoveryCandidate { .. }
            | ClusterCommand::RemoveDiscoveryCandidate { .. }
            | ClusterCommand::AdmitDiscoveryCandidate { .. } => {
                return Err(refused_command_authorization(
                    ClusterCommandAuthorizationRefusalCode::CoordinatorRequired,
                    submitter_node_id.clone(),
                    required_scope,
                    command.stable_digest(),
                    policy.stable_digest(),
                    format!(
                        "submitter `{}` is not the current coordinator and cannot remove members",
                        submitter_node_id.as_str()
                    ),
                ));
            }
            ClusterCommand::ReconcileConnection { link } => {
                if submitter.node_id != link.key.left_node_id
                    && submitter.node_id != link.key.right_node_id
                {
                    return Err(refused_command_authorization(
                        ClusterCommandAuthorizationRefusalCode::LinkPeerMismatch,
                        submitter_node_id.clone(),
                        required_scope,
                        command.stable_digest(),
                        policy.stable_digest(),
                        format!(
                            "submitter `{}` is not one endpoint of link `{}` <-> `{}`",
                            submitter_node_id.as_str(),
                            link.key.left_node_id.as_str(),
                            link.key.right_node_id.as_str()
                        ),
                    ));
                }
            }
            ClusterCommand::ReconcileNodeTelemetry { telemetry } => {
                if submitter.node_id != telemetry.node_id {
                    return Err(refused_command_authorization(
                        ClusterCommandAuthorizationRefusalCode::TargetNodeMismatch,
                        submitter_node_id.clone(),
                        required_scope,
                        command.stable_digest(),
                        policy.stable_digest(),
                        format!(
                            "submitter `{}` may only reconcile its own telemetry, not `{}`",
                            submitter_node_id.as_str(),
                            telemetry.node_id.as_str()
                        ),
                    ));
                }
            }
            ClusterCommand::ReconcileArtifactResidency { residency } => {
                if submitter.node_id != residency.key.node_id {
                    return Err(refused_command_authorization(
                        ClusterCommandAuthorizationRefusalCode::TargetNodeMismatch,
                        submitter_node_id.clone(),
                        required_scope,
                        command.stable_digest(),
                        policy.stable_digest(),
                        format!(
                            "submitter `{}` may only reconcile its own artifact residency, not `{}`",
                            submitter_node_id.as_str(),
                            residency.key.node_id.as_str()
                        ),
                    ));
                }
            }
            ClusterCommand::UpdateLeadership { leader_id, .. } => {
                if submitter.node_id != *leader_id {
                    return Err(refused_command_authorization(
                        ClusterCommandAuthorizationRefusalCode::ProposedLeaderMismatch,
                        submitter_node_id.clone(),
                        required_scope,
                        command.stable_digest(),
                        policy.stable_digest(),
                        format!(
                            "submitter `{}` may only propose itself as leader, not `{}`",
                            submitter_node_id.as_str(),
                            leader_id.as_str()
                        ),
                    ));
                }
            }
            ClusterCommand::RequestCatchup { request } => {
                if submitter.node_id != request.requester_id {
                    return Err(refused_command_authorization(
                        ClusterCommandAuthorizationRefusalCode::TargetNodeMismatch,
                        submitter_node_id.clone(),
                        required_scope,
                        command.stable_digest(),
                        policy.stable_digest(),
                        format!(
                            "submitter `{}` may only request catchup for itself, not `{}`",
                            submitter_node_id.as_str(),
                            request.requester_id.as_str()
                        ),
                    ));
                }
            }
            ClusterCommand::RequestSnapshot => {}
        }

        Ok(ClusterCommandAuthorization::new(
            command.stable_digest(),
            policy.stable_digest(),
            required_scope,
            submitter.node_id,
            submitter.role,
            submitter.membership_status,
            coordinator_authority,
        ))
    }

    /// Applies one indexed authoritative event using contiguous-index discipline.
    pub fn apply(&mut self, indexed_event: IndexedClusterEvent) -> Result<(), ClusterHistoryError> {
        validate_event_owner(self.cluster_id(), &indexed_event.cluster_id)?;
        validate_contiguous_index(self.last_applied_event_index(), indexed_event.index)?;
        let command_authorization = indexed_event.command_authorization.clone();

        match indexed_event.event {
            ClusterEvent::MembershipReconciled { membership } => {
                let node_id = membership.identity.node_id.clone();
                self.snapshot
                    .memberships
                    .insert(node_id.clone(), membership);
                if let Some(command_authorization) = command_authorization {
                    self.snapshot
                        .membership_provenance
                        .insert(node_id, command_authorization);
                } else {
                    self.snapshot.membership_provenance.remove(&node_id);
                }
            }
            ClusterEvent::MembershipRemoved { node_id, .. } => {
                self.snapshot.memberships.remove(&node_id);
                self.snapshot.membership_provenance.remove(&node_id);
                self.snapshot
                    .links
                    .retain(|key, _| key.left_node_id != node_id && key.right_node_id != node_id);
                self.snapshot
                    .link_provenance
                    .retain(|key, _| key.left_node_id != node_id && key.right_node_id != node_id);
                self.snapshot.telemetry.remove(&node_id);
                self.snapshot.telemetry_provenance.remove(&node_id);
                self.snapshot
                    .artifact_residency
                    .retain(|key, _| key.node_id != node_id);
                self.snapshot
                    .artifact_residency_provenance
                    .retain(|key, _| key.node_id != node_id);
                if self
                    .snapshot
                    .leadership
                    .as_ref()
                    .is_some_and(|leadership| leadership.leader_id == node_id)
                {
                    self.snapshot.leadership = None;
                    self.snapshot.leadership_provenance = None;
                }
            }
            ClusterEvent::DiscoveryCandidateReconciled { candidate } => {
                let node_id = candidate.candidate.node_id.clone();
                self.snapshot
                    .discovery_candidates
                    .insert(node_id.clone(), candidate);
                if let Some(command_authorization) = command_authorization {
                    self.snapshot
                        .discovery_candidate_provenance
                        .insert(node_id, command_authorization);
                } else {
                    self.snapshot
                        .discovery_candidate_provenance
                        .remove(&node_id);
                }
            }
            ClusterEvent::DiscoveryCandidateRemoved { node_id, .. } => {
                self.snapshot.discovery_candidates.remove(&node_id);
                self.snapshot
                    .discovery_candidate_provenance
                    .remove(&node_id);
            }
            ClusterEvent::DiscoveryCandidateAdmitted {
                node_id,
                membership,
            } => {
                let Some(candidate_record) =
                    self.snapshot.discovery_candidates.get(&node_id).cloned()
                else {
                    return Err(ClusterHistoryError::DiscoveryCandidateMissing { node_id });
                };

                if candidate_record.status != ClusterDiscoveredCandidateStatus::Introduced {
                    return Err(ClusterHistoryError::DiscoveryCandidateAdmissionRefused {
                        node_id,
                        status: candidate_record.status,
                    });
                }

                self.snapshot.discovery_candidates.insert(
                    node_id.clone(),
                    candidate_record
                        .with_status(ClusterDiscoveredCandidateStatus::Accepted)
                        .with_detail("admitted_into_membership"),
                );
                if let Some(command_authorization) = command_authorization.clone() {
                    self.snapshot
                        .discovery_candidate_provenance
                        .insert(node_id.clone(), command_authorization);
                } else {
                    self.snapshot
                        .discovery_candidate_provenance
                        .remove(&node_id);
                }

                self.snapshot
                    .memberships
                    .insert(node_id.clone(), membership);
                if let Some(command_authorization) = command_authorization {
                    self.snapshot
                        .membership_provenance
                        .insert(node_id, command_authorization);
                } else {
                    self.snapshot.membership_provenance.remove(&node_id);
                }
            }
            ClusterEvent::ConnectionReconciled { link } => {
                let link_key = link.key.clone();
                self.snapshot.links.insert(link_key.clone(), link);
                if let Some(command_authorization) = command_authorization {
                    self.snapshot
                        .link_provenance
                        .insert(link_key, command_authorization);
                } else {
                    self.snapshot.link_provenance.remove(&link_key);
                }
            }
            ClusterEvent::ConnectionRemoved { key } => {
                self.snapshot.links.remove(&key);
                self.snapshot.link_provenance.remove(&key);
            }
            ClusterEvent::NodeTelemetryReconciled { telemetry } => {
                let node_id = telemetry.node_id.clone();
                self.snapshot.telemetry.insert(node_id.clone(), telemetry);
                if let Some(command_authorization) = command_authorization {
                    self.snapshot
                        .telemetry_provenance
                        .insert(node_id, command_authorization);
                } else {
                    self.snapshot.telemetry_provenance.remove(&node_id);
                }
            }
            ClusterEvent::ArtifactResidencyReconciled { residency } => {
                let residency_key = residency.key.clone();
                self.snapshot
                    .artifact_residency
                    .insert(residency_key.clone(), residency);
                if let Some(command_authorization) = command_authorization {
                    self.snapshot
                        .artifact_residency_provenance
                        .insert(residency_key, command_authorization);
                } else {
                    self.snapshot
                        .artifact_residency_provenance
                        .remove(&residency_key);
                }
            }
            ClusterEvent::ArtifactResidencyRemoved { key } => {
                self.snapshot.artifact_residency.remove(&key);
                self.snapshot.artifact_residency_provenance.remove(&key);
            }
            ClusterEvent::LeadershipReconciled { leadership } => {
                if let Some(current_leadership) = &self.snapshot.leadership
                    && current_leadership.term == leadership.term
                    && current_leadership.leader_id != leadership.leader_id
                {
                    return Err(ClusterHistoryError::SplitBrainLeadership {
                        diagnostic: ClusterSplitBrainDiagnostic {
                            term: leadership.term,
                            current_leader_id: current_leadership.leader_id.clone(),
                            attempted_leader_id: leadership.leader_id,
                        },
                    });
                }
                self.snapshot.leadership = Some(leadership);
                self.snapshot.leadership_provenance = command_authorization;
            }
        }

        self.snapshot.last_applied_event_index = Some(indexed_event.index);
        Ok(())
    }

    fn membership_submitter(
        &self,
        submitter_node_id: &NodeId,
    ) -> Option<ResolvedClusterCommandSubmitter> {
        self.snapshot
            .memberships
            .get(submitter_node_id)
            .map(ResolvedClusterCommandSubmitter::from_membership)
    }

    fn command_submitter(
        &self,
        submitter_node_id: &NodeId,
        command: &ClusterCommand,
    ) -> Option<ResolvedClusterCommandSubmitter> {
        self.membership_submitter(submitter_node_id)
            .or_else(|| match command {
                ClusterCommand::ReconcileMembership { membership }
                    if membership.identity.node_id == *submitter_node_id =>
                {
                    Some(ResolvedClusterCommandSubmitter {
                        node_id: membership.identity.node_id.clone(),
                        role: membership.identity.role,
                        membership_status: membership.status,
                    })
                }
                _ => None,
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedClusterCommandSubmitter {
    node_id: NodeId,
    role: crate::NodeRole,
    membership_status: ClusterMembershipStatus,
}

impl ResolvedClusterCommandSubmitter {
    fn from_membership(membership: &ClusterMembershipRecord) -> Self {
        Self {
            node_id: membership.identity.node_id.clone(),
            role: membership.identity.role,
            membership_status: membership.status,
        }
    }
}

/// Ordered authoritative event log for cluster-state replay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterEventLog {
    /// Cluster identity that owns the log.
    pub cluster_id: ClusterId,
    /// Explicit schema version for this ordered log.
    pub schema_version: ClusterSchemaVersion,
    /// Compacted authoritative base snapshot, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    compacted_snapshot: Option<ClusterSnapshot>,
    /// Retained ordered tail after compaction.
    events: Vec<IndexedClusterEvent>,
}

impl ClusterEventLog {
    /// Creates an empty authoritative event log for one cluster identity.
    #[must_use]
    pub fn new(cluster_id: ClusterId) -> Self {
        Self::with_schema_version(cluster_id, ClusterSchemaVersion::initial())
    }

    /// Creates an empty authoritative event log with an explicit schema version.
    #[must_use]
    pub fn with_schema_version(
        cluster_id: ClusterId,
        schema_version: ClusterSchemaVersion,
    ) -> Self {
        Self {
            cluster_id,
            schema_version,
            compacted_snapshot: None,
            events: Vec::new(),
        }
    }

    /// Loads one authoritative event log from stable JSON storage.
    pub fn load_json(path: impl AsRef<Path>) -> Result<Self, ClusterHistoryError> {
        let bytes = fs::read(path).map_err(cluster_persistence_io_error)?;
        let log =
            serde_json::from_slice::<Self>(&bytes).map_err(cluster_persistence_format_error)?;
        log.validate_persisted_structure()?;
        Ok(log)
    }

    /// Stores one authoritative event log as stable pretty JSON.
    pub fn store_json(&self, path: impl AsRef<Path>) -> Result<(), ClusterHistoryError> {
        self.validate_persisted_structure()?;
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(cluster_persistence_io_error)?;
        }
        let encoded = serde_json::to_vec_pretty(self).map_err(cluster_persistence_format_error)?;
        fs::write(path, encoded).map_err(cluster_persistence_io_error)
    }

    /// Returns the ordered authoritative event history retained in the log.
    #[must_use]
    pub fn events(&self) -> &[IndexedClusterEvent] {
        &self.events
    }

    /// Returns the compacted authoritative base snapshot, when one exists.
    #[must_use]
    pub fn compacted_snapshot(&self) -> Option<&ClusterSnapshot> {
        self.compacted_snapshot.as_ref()
    }

    /// Returns the highest authoritative event index hidden behind compaction, when one exists.
    #[must_use]
    pub fn compacted_through(&self) -> Option<ClusterEventIndex> {
        self.compacted_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.last_applied_event_index)
    }

    /// Returns the highest authoritative event index in the log, when one exists.
    #[must_use]
    pub fn last_index(&self) -> Option<ClusterEventIndex> {
        self.events
            .last()
            .map(|event| event.index)
            .or_else(|| self.compacted_through())
    }

    /// Appends one new authoritative event at the next contiguous index.
    #[must_use]
    pub fn append_event(&mut self, event: ClusterEvent) -> IndexedClusterEvent {
        let index = next_expected_index(self.last_index());
        let indexed_event = IndexedClusterEvent::new(self.cluster_id.clone(), index, event);
        self.events.push(indexed_event.clone());
        indexed_event
    }

    /// Appends one new authoritative event with retained command provenance.
    #[must_use]
    pub fn append_event_with_authorization(
        &mut self,
        event: ClusterEvent,
        command_authorization: ClusterCommandAuthorization,
    ) -> IndexedClusterEvent {
        let index = next_expected_index(self.last_index());
        let indexed_event = IndexedClusterEvent::new(self.cluster_id.clone(), index, event)
            .with_command_authorization(command_authorization);
        self.events.push(indexed_event.clone());
        indexed_event
    }

    /// Pushes one externally indexed authoritative event after contiguous validation.
    pub fn push_indexed_event(
        &mut self,
        indexed_event: IndexedClusterEvent,
    ) -> Result<(), ClusterHistoryError> {
        validate_event_owner(&self.cluster_id, &indexed_event.cluster_id)?;
        validate_contiguous_index(self.last_index(), indexed_event.index)?;
        self.events.push(indexed_event);
        Ok(())
    }

    /// Returns the current authoritative snapshot represented by this log.
    pub fn snapshot(&self) -> Result<ClusterSnapshot, ClusterHistoryError> {
        self.replay().map(|state| state.snapshot())
    }

    /// Returns retained events after one already-applied index.
    #[must_use]
    pub fn events_after(
        &self,
        last_applied_event_index: Option<ClusterEventIndex>,
        limit: usize,
    ) -> Vec<IndexedClusterEvent> {
        self.events
            .iter()
            .filter(|event| match last_applied_event_index {
                Some(last_applied) => event.index > last_applied,
                None => true,
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Compacts the retained log into a base snapshot while preserving a tail for catchup.
    pub fn compact(
        &mut self,
        policy: &ClusterRecoveryPolicy,
    ) -> Result<Option<ClusterSnapshot>, ClusterHistoryError> {
        let split = self.events.len().saturating_sub(policy.retain_tail_events);
        if split == 0 {
            return Ok(None);
        }

        let snapshot = self.replay_prefix(split)?.snapshot();
        self.compacted_snapshot = Some(snapshot.clone());
        self.events.drain(..split);
        Ok(Some(snapshot))
    }

    /// Builds a catchup response for one recovering or rejoining node.
    pub fn catchup_response(
        &self,
        request: &ClusterCatchupRequest,
        policy: &ClusterRecoveryPolicy,
    ) -> Result<ClusterCatchupResponse, ClusterHistoryError> {
        validate_event_owner(&self.cluster_id, &request.cluster_id)?;

        let head_event_index = self.last_index();
        let limit = policy.response_limit(request.max_events);
        let current_snapshot = self.snapshot()?;

        if request.schema_version != self.schema_version {
            return Ok(ClusterCatchupResponse {
                cluster_id: self.cluster_id.clone(),
                requester_id: request.requester_id.clone(),
                head_event_index,
                disposition: ClusterRecoveryDisposition::FullResync,
                payload: ClusterCatchupPayload::Snapshot {
                    snapshot: Box::new(current_snapshot),
                    tail_events: Vec::new(),
                    reason: ClusterRecoveryReason::SchemaVersionMismatch {
                        expected: self.schema_version,
                        actual: request.schema_version,
                    },
                },
            });
        }

        if gap_exceeds_threshold(
            request.last_applied_event_index,
            head_event_index,
            policy.full_resync_gap_threshold,
        ) {
            return Ok(ClusterCatchupResponse {
                cluster_id: self.cluster_id.clone(),
                requester_id: request.requester_id.clone(),
                head_event_index,
                disposition: ClusterRecoveryDisposition::FullResync,
                payload: ClusterCatchupPayload::Snapshot {
                    snapshot: Box::new(current_snapshot),
                    tail_events: Vec::new(),
                    reason: ClusterRecoveryReason::GapThresholdExceeded {
                        requested_from: request.last_applied_event_index,
                        head: head_event_index,
                        threshold: policy.full_resync_gap_threshold,
                    },
                },
            });
        }

        if let Some(compacted_through) = self.compacted_through()
            && request
                .last_applied_event_index
                .is_none_or(|index| index < compacted_through)
        {
            let snapshot = self
                .compacted_snapshot
                .clone()
                .unwrap_or_else(|| current_snapshot.clone());
            let tail_events = self.events_after(snapshot.last_applied_event_index, limit);
            return Ok(ClusterCatchupResponse {
                cluster_id: self.cluster_id.clone(),
                requester_id: request.requester_id.clone(),
                head_event_index,
                disposition: ClusterRecoveryDisposition::InstallSnapshot,
                payload: ClusterCatchupPayload::Snapshot {
                    snapshot: Box::new(snapshot),
                    tail_events,
                    reason: ClusterRecoveryReason::CompactionBoundary {
                        compacted_through: Some(compacted_through),
                    },
                },
            });
        }

        Ok(ClusterCatchupResponse {
            cluster_id: self.cluster_id.clone(),
            requester_id: request.requester_id.clone(),
            head_event_index,
            disposition: ClusterRecoveryDisposition::CatchUp,
            payload: ClusterCatchupPayload::Events {
                events: self.events_after(request.last_applied_event_index, limit),
            },
        })
    }

    /// Rebuilds authoritative cluster state from the ordered event history.
    pub fn replay(&self) -> Result<ClusterState, ClusterHistoryError> {
        self.replay_prefix(self.events.len())
    }

    fn replay_prefix(&self, prefix_len: usize) -> Result<ClusterState, ClusterHistoryError> {
        let mut state = match &self.compacted_snapshot {
            Some(snapshot) => ClusterState::from_snapshot(snapshot.clone()),
            None => ClusterState::new_with_schema(self.cluster_id.clone(), self.schema_version),
        };
        for event in self.events.iter().take(prefix_len) {
            state.apply(event.clone())?;
        }
        Ok(state)
    }

    fn validate_persisted_structure(&self) -> Result<(), ClusterHistoryError> {
        if let Some(snapshot) = &self.compacted_snapshot {
            validate_event_owner(&self.cluster_id, &snapshot.cluster_id)?;
            if snapshot.schema_version != self.schema_version {
                return Err(ClusterHistoryError::SnapshotSchemaVersionMismatch {
                    expected: self.schema_version,
                    actual: snapshot.schema_version,
                });
            }
        }
        let mut last_applied_event_index = self
            .compacted_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.last_applied_event_index);
        for event in &self.events {
            validate_event_owner(&self.cluster_id, &event.cluster_id)?;
            validate_contiguous_index(last_applied_event_index, event.index)?;
            last_applied_event_index = Some(event.index);
        }
        let _ = self.replay()?;
        Ok(())
    }
}

/// Ordered-log and state-apply failures for the first authoritative seam.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClusterHistoryError {
    /// Persisted log JSON could not be read or written.
    #[error("failed to read or write cluster event log ({kind:?}): {message}")]
    PersistenceIo {
        /// Coarse I/O failure category.
        kind: std::io::ErrorKind,
        /// Stable human-readable error detail.
        message: String,
    },
    /// Persisted log JSON could not be decoded or encoded.
    #[error(
        "failed to decode or encode cluster event log JSON ({category} at {line}:{column}): {message}"
    )]
    PersistenceFormat {
        /// Coarse serde-json error category.
        category: String,
        /// 1-based line surfaced by serde-json.
        line: usize,
        /// 1-based column surfaced by serde-json.
        column: usize,
        /// Stable human-readable error detail.
        message: String,
    },
    /// Event or request belongs to a different cluster than the owning log/state.
    #[error("cluster event belongs to a different cluster: expected {expected:?}, got {actual:?}")]
    ClusterIdMismatch {
        /// Expected cluster identity.
        expected: ClusterId,
        /// Observed cluster identity.
        actual: ClusterId,
    },
    /// Compacted snapshot schema version no longer matches the owning log schema.
    #[error("cluster snapshot schema version mismatch: expected {expected:?}, got {actual:?}")]
    SnapshotSchemaVersionMismatch {
        /// Expected schema version carried by the owning log.
        expected: ClusterSchemaVersion,
        /// Actual schema version carried by the compacted snapshot.
        actual: ClusterSchemaVersion,
    },
    /// Event index does not continue contiguously from the current apply point.
    #[error("cluster event index is out of order: expected {expected:?}, got {actual:?}")]
    OutOfOrderEvent {
        /// Next expected authoritative event index.
        expected: ClusterEventIndex,
        /// Actual authoritative event index presented.
        actual: ClusterEventIndex,
    },
    /// Same term attempted to reconcile two different leaders.
    #[error("conflicting leadership claim in authoritative state: {diagnostic:?}")]
    SplitBrainLeadership {
        /// Machine-checkable split-brain detail.
        diagnostic: ClusterSplitBrainDiagnostic,
    },
    /// Discovery-candidate admission referenced a node not present in the candidate ledger.
    #[error("discovery candidate `{node_id:?}` is not present in authoritative state")]
    DiscoveryCandidateMissing {
        /// Candidate node ID that was required.
        node_id: NodeId,
    },
    /// Discovery-candidate admission referenced a candidate that is no longer eligible.
    #[error("discovery candidate `{node_id:?}` cannot be admitted while status is `{status:?}`")]
    DiscoveryCandidateAdmissionRefused {
        /// Candidate node ID that was refused.
        node_id: NodeId,
        /// Current status recorded for the candidate.
        status: ClusterDiscoveredCandidateStatus,
    },
}

fn validate_event_owner(
    expected_cluster_id: &ClusterId,
    actual_cluster_id: &ClusterId,
) -> Result<(), ClusterHistoryError> {
    if expected_cluster_id != actual_cluster_id {
        return Err(ClusterHistoryError::ClusterIdMismatch {
            expected: expected_cluster_id.clone(),
            actual: actual_cluster_id.clone(),
        });
    }
    Ok(())
}

fn validate_contiguous_index(
    last_applied_event_index: Option<ClusterEventIndex>,
    actual_event_index: ClusterEventIndex,
) -> Result<(), ClusterHistoryError> {
    let expected_event_index = next_expected_index(last_applied_event_index);
    if actual_event_index != expected_event_index {
        return Err(ClusterHistoryError::OutOfOrderEvent {
            expected: expected_event_index,
            actual: actual_event_index,
        });
    }
    Ok(())
}

fn cluster_persistence_io_error(error: std::io::Error) -> ClusterHistoryError {
    ClusterHistoryError::PersistenceIo {
        kind: error.kind(),
        message: error.to_string(),
    }
}

fn cluster_persistence_format_error(error: serde_json::Error) -> ClusterHistoryError {
    ClusterHistoryError::PersistenceFormat {
        category: format!("{:?}", error.classify()),
        line: error.line(),
        column: error.column(),
        message: error.to_string(),
    }
}

fn next_expected_index(last_applied_event_index: Option<ClusterEventIndex>) -> ClusterEventIndex {
    last_applied_event_index.map_or(ClusterEventIndex::initial(), ClusterEventIndex::next)
}

fn stable_json_digest(label: &str, value: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(value)
        .unwrap_or_else(|_| unreachable!("cluster stable-digest serialization should succeed"));
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    hasher.update(b"|");
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn refused_command_authorization(
    code: ClusterCommandAuthorizationRefusalCode,
    submitter_node_id: NodeId,
    required_scope: ClusterCommandAuthorityScope,
    command_digest: String,
    authorization_policy_digest: String,
    detail: impl Into<String>,
) -> ClusterCommandAuthorizationError {
    ClusterCommandAuthorizationError::Refused {
        refusal: ClusterCommandAuthorizationRefusal::new(
            code,
            submitter_node_id,
            required_scope,
            command_digest,
            authorization_policy_digest,
            detail,
        ),
    }
}

fn gap_exceeds_threshold(
    requested_from: Option<ClusterEventIndex>,
    head: Option<ClusterEventIndex>,
    threshold: usize,
) -> bool {
    let Some(head) = head else {
        return false;
    };
    let requested = requested_from.map_or(0, ClusterEventIndex::as_u64);
    head.as_u64().saturating_sub(requested) as usize > threshold
}

fn default_link_class(transport: ClusterTransportClass) -> ClusterLinkClass {
    match transport {
        ClusterTransportClass::LoopbackUdp => ClusterLinkClass::Loopback,
        ClusterTransportClass::LanUdp => ClusterLinkClass::Ethernet,
        ClusterTransportClass::Tcp => ClusterLinkClass::Ethernet,
        ClusterTransportClass::Rdma => ClusterLinkClass::Rdma,
        ClusterTransportClass::Unknown => ClusterLinkClass::Unknown,
    }
}

fn default_stability_for_link_status(status: ClusterLinkStatus) -> ClusterStabilityPosture {
    match status {
        ClusterLinkStatus::Healthy => ClusterStabilityPosture::Stable,
        ClusterLinkStatus::Pending | ClusterLinkStatus::Degraded => ClusterStabilityPosture::Flaky,
        ClusterLinkStatus::Disconnected => ClusterStabilityPosture::Unstable,
    }
}

fn node_role_label(role: crate::NodeRole) -> &'static [u8] {
    match role {
        crate::NodeRole::CoordinatorOnly => b"coordinator_only",
        crate::NodeRole::ExecutorOnly => b"executor_only",
        crate::NodeRole::Mixed => b"mixed",
    }
}

fn membership_status_label(status: ClusterMembershipStatus) -> &'static [u8] {
    match status {
        ClusterMembershipStatus::Joining => b"joining",
        ClusterMembershipStatus::Ready => b"ready",
        ClusterMembershipStatus::Draining => b"draining",
        ClusterMembershipStatus::Offline => b"offline",
    }
}

fn transport_class_label(transport: ClusterTransportClass) -> &'static [u8] {
    match transport {
        ClusterTransportClass::LoopbackUdp => b"loopback_udp",
        ClusterTransportClass::LanUdp => b"lan_udp",
        ClusterTransportClass::Tcp => b"tcp",
        ClusterTransportClass::Rdma => b"rdma",
        ClusterTransportClass::Unknown => b"unknown",
    }
}

fn link_status_label(status: ClusterLinkStatus) -> &'static [u8] {
    match status {
        ClusterLinkStatus::Pending => b"pending",
        ClusterLinkStatus::Healthy => b"healthy",
        ClusterLinkStatus::Degraded => b"degraded",
        ClusterLinkStatus::Disconnected => b"disconnected",
    }
}

fn stability_label(stability: ClusterStabilityPosture) -> &'static [u8] {
    match stability {
        ClusterStabilityPosture::Stable => b"stable",
        ClusterStabilityPosture::Flaky => b"flaky",
        ClusterStabilityPosture::Unstable => b"unstable",
    }
}

fn link_class_label(link_class: ClusterLinkClass) -> &'static [u8] {
    match link_class {
        ClusterLinkClass::Loopback => b"loopback",
        ClusterLinkClass::Ethernet => b"ethernet",
        ClusterLinkClass::Wifi => b"wifi",
        ClusterLinkClass::Thunderbolt => b"thunderbolt",
        ClusterLinkClass::Rdma => b"rdma",
        ClusterLinkClass::Unknown => b"unknown",
    }
}

fn backend_readiness_label(readiness: ClusterBackendReadinessStatus) -> &'static [u8] {
    match readiness {
        ClusterBackendReadinessStatus::Ready => b"ready",
        ClusterBackendReadinessStatus::Degraded => b"degraded",
        ClusterBackendReadinessStatus::Refused => b"refused",
        ClusterBackendReadinessStatus::Unknown => b"unknown",
    }
}

fn artifact_residency_label(status: ClusterArtifactResidencyStatus) -> &'static [u8] {
    match status {
        ClusterArtifactResidencyStatus::Resident => b"resident",
        ClusterArtifactResidencyStatus::CopyRequired => b"copy_required",
        ClusterArtifactResidencyStatus::PullRequired => b"pull_required",
        ClusterArtifactResidencyStatus::Refused => b"refused",
    }
}

fn artifact_transfer_label(transfer_method: ClusterArtifactTransferMethod) -> &'static [u8] {
    match transfer_method {
        ClusterArtifactTransferMethod::PeerCopy => b"peer_copy",
        ClusterArtifactTransferMethod::OciPull => b"oci_pull",
        ClusterArtifactTransferMethod::Unknown => b"unknown",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use std::{fs, net::SocketAddr};

    use ed25519_dalek::SigningKey;
    use tempfile::tempdir;

    use crate::{AdmissionToken, ClusterDiscoveryCandidate, ClusterNamespace, NodeEpoch, NodeRole};

    use super::{
        ClusterArtifactReference, ClusterArtifactResidencyRecord, ClusterArtifactResidencyStatus,
        ClusterArtifactTransferMethod, ClusterBackendReadinessStatus, ClusterCatchupPayload,
        ClusterCatchupRequest, ClusterCommand, ClusterCommandAuthorityScope,
        ClusterCommandAuthorizationError, ClusterCommandAuthorizationPolicy,
        ClusterCommandAuthorizationRefusalCode, ClusterDiscoveredCandidateRecord,
        ClusterDiscoveredCandidateStatus, ClusterElectionError, ClusterElectionLedger,
        ClusterElectionMessage, ClusterEvent, ClusterEventIndex, ClusterEventLog,
        ClusterHistoryError, ClusterLeadershipLeasePolicy, ClusterLeadershipLeaseStatus,
        ClusterLeadershipRecord, ClusterLeaseTick, ClusterLink, ClusterLinkClass,
        ClusterLinkStatus, ClusterMembershipRecord, ClusterMembershipStatus, ClusterNodeTelemetry,
        ClusterRecoveryDisposition, ClusterRecoveryEnvelopeError, ClusterRecoveryPolicy,
        ClusterRecoveryReason, ClusterRecoveryReplayWindow, ClusterSchemaVersion, ClusterSnapshot,
        ClusterSplitBrainDiagnostic, ClusterStabilityPosture, ClusterState, ClusterTerm,
        ClusterTransportClass, ConfiguredClusterPeer, IndexedClusterEvent,
    };

    fn sample_cluster_id() -> crate::ClusterId {
        crate::ClusterId::new(
            &ClusterNamespace::new("lan-alpha"),
            &AdmissionToken::new("shared-secret"),
        )
    }

    fn sample_membership_record(
        cluster_id: &crate::ClusterId,
        port: u16,
        role: NodeRole,
        status: ClusterMembershipStatus,
    ) -> ClusterMembershipRecord {
        ClusterMembershipRecord::new(
            crate::ClusterNodeIdentity {
                cluster_id: cluster_id.clone(),
                node_id: crate::NodeId::random(),
                node_epoch: NodeEpoch::initial(),
                role,
                auth_public_key: String::new(),
                attestation: None,
            },
            Some(SocketAddr::from(([127, 0, 0, 1], port))),
            status,
        )
    }

    fn sample_recovery_policy() -> ClusterRecoveryPolicy {
        ClusterRecoveryPolicy {
            max_events_per_response: 16,
            retain_tail_events: 2,
            full_resync_gap_threshold: 64,
        }
    }

    fn sample_leadership_lease_policy() -> ClusterLeadershipLeasePolicy {
        ClusterLeadershipLeasePolicy::new(4)
    }

    fn sample_discovery_candidate_record(
        cluster_id: &crate::ClusterId,
        node_id: crate::NodeId,
        port: u16,
        role: NodeRole,
    ) -> ClusterDiscoveredCandidateRecord {
        ClusterDiscoveredCandidateRecord {
            candidate: ClusterDiscoveryCandidate::new(
                cluster_id.clone(),
                ClusterNamespace::new("lan-alpha"),
                node_id,
                role,
                format!("candidate-public-key-{port}"),
                vec![SocketAddr::from(([10, 0, 0, 1], port))],
            ),
            introduced_by_source_id: "operator-source".to_string(),
            introduction_policy_digest: format!("policy-digest-{port}"),
            introduction_payload_digest: format!("payload-digest-{port}"),
            introduced_at_ms: 10_000,
            expires_at_ms: 20_000,
            status: ClusterDiscoveredCandidateStatus::Introduced,
            detail: None,
        }
    }

    fn sample_state_with_coordinator_authority(
        cluster_id: &crate::ClusterId,
        coordinator: &ClusterMembershipRecord,
        policy: &ClusterCommandAuthorizationPolicy,
    ) -> ClusterState {
        let mut state = ClusterState::new(cluster_id.clone());
        let membership_authorization = state
            .authorize_command(
                &coordinator.identity.node_id,
                &ClusterCommand::ReconcileMembership {
                    membership: coordinator.clone(),
                },
                policy,
            )
            .expect("coordinator membership authorization should succeed");
        state
            .apply(
                IndexedClusterEvent::new(
                    cluster_id.clone(),
                    ClusterEventIndex::initial(),
                    ClusterEvent::MembershipReconciled {
                        membership: coordinator.clone(),
                    },
                )
                .with_command_authorization(membership_authorization),
            )
            .expect("coordinator membership should apply");

        let leadership_authorization = state
            .authorize_command(
                &coordinator.identity.node_id,
                &ClusterCommand::UpdateLeadership {
                    leader_id: coordinator.identity.node_id.clone(),
                    term: ClusterTerm::initial(),
                },
                policy,
            )
            .expect("coordinator leadership authorization should succeed");
        state
            .apply(
                IndexedClusterEvent::new(
                    cluster_id.clone(),
                    ClusterEventIndex::initial().next(),
                    ClusterEvent::LeadershipReconciled {
                        leadership: ClusterLeadershipRecord::new(
                            ClusterTerm::initial(),
                            coordinator.identity.node_id.clone(),
                            ClusterEventIndex::initial(),
                        ),
                    },
                )
                .with_command_authorization(leadership_authorization),
            )
            .expect("coordinator leadership should apply");

        state
    }

    fn sample_signing_key(byte: u8) -> SigningKey {
        SigningKey::from_bytes(&[byte; 32])
    }

    fn recovery_signer(
        cluster_id: &crate::ClusterId,
        node_id: &str,
        role: NodeRole,
        signing_key: &SigningKey,
    ) -> crate::ClusterNodeIdentity {
        crate::ClusterNodeIdentity {
            cluster_id: cluster_id.clone(),
            node_id: crate::NodeId::new(node_id),
            node_epoch: NodeEpoch::initial(),
            role,
            auth_public_key: hex::encode(signing_key.verifying_key().to_bytes()),
            attestation: None,
        }
    }

    #[test]
    fn authoritative_event_log_replays_into_stable_cluster_state() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4101,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let executor = sample_membership_record(
            &cluster_id,
            4102,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );
        let link = ClusterLink::new(
            coordinator.identity.node_id.clone(),
            executor.identity.node_id.clone(),
            ClusterTransportClass::LoopbackUdp,
            ClusterLinkStatus::Healthy,
        );

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let first_event = log.append_event(ClusterEvent::MembershipReconciled {
            membership: coordinator.clone(),
        });
        assert_eq!(first_event.index, ClusterEventIndex::initial());

        let _ = log.append_event(ClusterEvent::MembershipReconciled {
            membership: executor.clone(),
        });
        let third_event =
            log.append_event(ClusterEvent::ConnectionReconciled { link: link.clone() });
        let fourth_event = log.append_event(ClusterEvent::LeadershipReconciled {
            leadership: ClusterLeadershipRecord::new(
                ClusterTerm::initial(),
                coordinator.identity.node_id.clone(),
                third_event.index,
            ),
        });

        let replayed = log.replay();
        assert!(replayed.is_ok(), "ordered event log should replay");
        let replayed = replayed
            .ok()
            .unwrap_or_else(|| unreachable!("assert above ensures success"));

        assert_eq!(
            replayed.last_applied_event_index(),
            Some(fourth_event.index)
        );
        assert_eq!(replayed.memberships().len(), 2);
        assert_eq!(
            replayed
                .memberships()
                .get(&coordinator.identity.node_id)
                .map(|record| record.status),
            Some(ClusterMembershipStatus::Ready)
        );
        assert_eq!(replayed.links().get(&link.key), Some(&link));
        assert_eq!(
            replayed
                .leadership()
                .map(|leadership| &leadership.leader_id),
            Some(&coordinator.identity.node_id)
        );

        let digest = replayed.stable_digest();
        let replayed_again = log.replay();
        assert!(replayed_again.is_ok(), "replay should stay deterministic");
        let replayed_again = replayed_again
            .ok()
            .unwrap_or_else(|| unreachable!("assert above ensures success"));
        assert_eq!(digest, replayed_again.stable_digest());

        let snapshot_round_trip = ClusterState::from_snapshot(replayed.snapshot());
        assert_eq!(digest, snapshot_round_trip.stable_digest());
    }

    #[test]
    fn leadership_lease_reports_active_then_stale() {
        let lease_policy = sample_leadership_lease_policy();
        let leadership = ClusterLeadershipRecord::new(
            ClusterTerm::initial(),
            crate::NodeId::new("leader-alpha"),
            ClusterEventIndex::initial(),
        )
        .with_lease_policy(ClusterLeaseTick::new(10), lease_policy);

        assert_eq!(
            leadership.lease_status_at(ClusterLeaseTick::new(12)),
            ClusterLeadershipLeaseStatus::Active {
                expires_at_tick: ClusterLeaseTick::new(14),
                remaining_ticks: 2,
            }
        );
        assert_eq!(
            leadership.lease_status_at(ClusterLeaseTick::new(15)),
            ClusterLeadershipLeaseStatus::Stale {
                expired_at_tick: ClusterLeaseTick::new(14),
                overdue_ticks: 1,
            }
        );

        let diagnostic = leadership
            .stale_diagnostic_at(ClusterLeaseTick::new(15))
            .expect("stale leadership should surface a diagnostic");
        assert_eq!(diagnostic.leader_id, crate::NodeId::new("leader-alpha"));
        assert_eq!(diagnostic.term, ClusterTerm::initial());
        assert_eq!(
            diagnostic.heartbeat_observed_tick,
            ClusterLeaseTick::new(10)
        );
        assert_eq!(diagnostic.expired_at_tick, ClusterLeaseTick::new(14));
        assert_eq!(diagnostic.observed_tick, ClusterLeaseTick::new(15));
    }

    #[test]
    fn cluster_state_effective_leadership_expires_when_lease_goes_stale() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4111,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let lease_policy = sample_leadership_lease_policy();

        let mut state = ClusterState::new(cluster_id.clone());
        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial(),
                ClusterEvent::MembershipReconciled {
                    membership: coordinator.clone(),
                },
            ))
            .expect("membership should apply");
        state
            .apply(IndexedClusterEvent::new(
                cluster_id,
                ClusterEventIndex::initial().next(),
                ClusterEvent::LeadershipReconciled {
                    leadership: ClusterLeadershipRecord::new(
                        ClusterTerm::initial(),
                        coordinator.identity.node_id.clone(),
                        ClusterEventIndex::initial(),
                    )
                    .with_lease_policy(ClusterLeaseTick::new(20), lease_policy),
                },
            ))
            .expect("leadership should apply");

        assert_eq!(
            state
                .effective_leadership_at(ClusterLeaseTick::new(23))
                .map(|leadership| leadership.leader_id.clone()),
            Some(coordinator.identity.node_id.clone())
        );
        assert_eq!(
            state.leadership_lease_status_at(ClusterLeaseTick::new(24)),
            Some(ClusterLeadershipLeaseStatus::Active {
                expires_at_tick: ClusterLeaseTick::new(24),
                remaining_ticks: 0,
            })
        );
        assert!(
            state
                .effective_leadership_at(ClusterLeaseTick::new(25))
                .is_none(),
            "stale leadership should not remain effective"
        );

        let diagnostic = state
            .stale_leadership_diagnostic_at(ClusterLeaseTick::new(25))
            .expect("stale coordinator should produce a diagnostic");
        assert_eq!(diagnostic.leader_id, coordinator.identity.node_id);
        assert_eq!(diagnostic.expired_at_tick, ClusterLeaseTick::new(24));
        assert_eq!(diagnostic.observed_tick, ClusterLeaseTick::new(25));
    }

    #[test]
    fn leadership_lease_changes_snapshot_digest() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4112,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let base_leadership = ClusterLeadershipRecord::new(
            ClusterTerm::initial(),
            coordinator.identity.node_id.clone(),
            ClusterEventIndex::initial(),
        )
        .with_lease_policy(ClusterLeaseTick::new(30), sample_leadership_lease_policy());
        let renewed_leadership = base_leadership
            .clone()
            .renewed_at(ClusterLeaseTick::new(31), sample_leadership_lease_policy());

        let mut base_snapshot = ClusterSnapshot::new(cluster_id.clone());
        base_snapshot
            .memberships
            .insert(coordinator.identity.node_id.clone(), coordinator.clone());
        base_snapshot.leadership = Some(base_leadership);

        let mut renewed_snapshot = ClusterSnapshot::new(cluster_id);
        renewed_snapshot
            .memberships
            .insert(coordinator.identity.node_id.clone(), coordinator);
        renewed_snapshot.leadership = Some(renewed_leadership);

        assert_ne!(
            base_snapshot.stable_digest(),
            renewed_snapshot.stable_digest(),
            "leadership lease changes should affect snapshot digests"
        );
    }

    #[test]
    fn commit_authority_fence_token_changes_after_failover() {
        let cluster_id = sample_cluster_id();
        let first_leader = ClusterLeadershipRecord::new(
            ClusterTerm::initial(),
            crate::NodeId::new("leader-alpha"),
            ClusterEventIndex::initial(),
        )
        .with_lease_policy(ClusterLeaseTick::new(40), sample_leadership_lease_policy());
        let second_leader = ClusterLeadershipRecord::new(
            ClusterTerm::initial().next(),
            crate::NodeId::new("leader-beta"),
            ClusterEventIndex::initial().next(),
        )
        .with_lease_policy(ClusterLeaseTick::new(45), sample_leadership_lease_policy());

        let first_authority = first_leader.commit_authority(&cluster_id);
        let second_authority = second_leader.commit_authority(&cluster_id);

        assert_ne!(first_authority.fence_token, second_authority.fence_token);
        assert_ne!(
            first_authority.authority_digest,
            second_authority.authority_digest
        );

        let mut state = ClusterState::new(cluster_id.clone());
        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial(),
                ClusterEvent::LeadershipReconciled {
                    leadership: first_leader,
                },
            ))
            .expect("first leader should apply");
        state
            .apply(IndexedClusterEvent::new(
                cluster_id,
                ClusterEventIndex::initial().next(),
                ClusterEvent::LeadershipReconciled {
                    leadership: second_leader,
                },
            ))
            .expect("next-term failover should apply");

        let current_authority = state
            .commit_authority()
            .expect("current authority should exist after failover");
        assert_eq!(
            current_authority.leader_id,
            crate::NodeId::new("leader-beta")
        );
        assert_eq!(current_authority.term, ClusterTerm::initial().next());
        assert_eq!(current_authority.fence_token, second_authority.fence_token);
    }

    #[test]
    fn election_ledger_refuses_conflicting_votes_in_same_term() {
        let term = ClusterTerm::initial();
        let voter = crate::NodeId::new("voter-alpha");
        let candidate_alpha = crate::NodeId::new("candidate-alpha");
        let candidate_beta = crate::NodeId::new("candidate-beta");
        let mut ledger = ClusterElectionLedger::new();

        ledger
            .observe(&ClusterElectionMessage::RequestVotes {
                term,
                candidate_id: candidate_alpha.clone(),
                last_applied_event_index: None,
            })
            .expect("vote request should record");
        ledger
            .observe(&ClusterElectionMessage::GrantVote {
                term,
                candidate_id: candidate_alpha.clone(),
                voter_id: voter.clone(),
            })
            .expect("first vote grant should record");

        let conflict = ledger.observe(&ClusterElectionMessage::GrantVote {
            term,
            candidate_id: candidate_beta.clone(),
            voter_id: voter.clone(),
        });
        assert!(
            matches!(
                conflict,
                Err(ClusterElectionError::ConflictingVote { diagnostic })
                    if diagnostic.term == term
                        && diagnostic.voter_id == voter
                        && diagnostic.current_candidate_id == candidate_alpha
                        && diagnostic.attempted_candidate_id == candidate_beta
            ),
            "same voter should not grant two candidates in one term"
        );
        assert_eq!(
            ledger
                .term(term)
                .map(|term_state| term_state.votes_for(&candidate_alpha)),
            Some(1)
        );
    }

    #[test]
    fn election_ledger_refuses_split_brain_leader_claims_in_same_term() {
        let term = ClusterTerm::initial();
        let leader_alpha = crate::NodeId::new("leader-alpha");
        let leader_beta = crate::NodeId::new("leader-beta");
        let mut ledger = ClusterElectionLedger::new();

        ledger
            .observe(&ClusterElectionMessage::LeaderHeartbeat {
                term,
                leader_id: leader_alpha.clone(),
                committed_event_index: Some(ClusterEventIndex::initial()),
            })
            .expect("first leader claim should record");

        let conflict = ledger.observe(&ClusterElectionMessage::LeaderHeartbeat {
            term,
            leader_id: leader_beta.clone(),
            committed_event_index: Some(ClusterEventIndex::initial().next()),
        });
        assert!(
            matches!(
                conflict,
                Err(ClusterElectionError::SplitBrainLeaderClaim { diagnostic })
                    if diagnostic == ClusterSplitBrainDiagnostic {
                        term,
                        current_leader_id: leader_alpha,
                        attempted_leader_id: leader_beta,
                    }
            ),
            "same term should refuse conflicting leader claims"
        );
    }

    #[test]
    fn authoritative_state_refuses_conflicting_same_term_leadership() {
        let cluster_id = sample_cluster_id();
        let leader_alpha = crate::NodeId::new("leader-alpha");
        let leader_beta = crate::NodeId::new("leader-beta");
        let mut state = ClusterState::new(cluster_id.clone());

        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial(),
                ClusterEvent::LeadershipReconciled {
                    leadership: ClusterLeadershipRecord::new(
                        ClusterTerm::initial(),
                        leader_alpha.clone(),
                        ClusterEventIndex::initial(),
                    )
                    .with_lease_policy(ClusterLeaseTick::new(1), sample_leadership_lease_policy()),
                },
            ))
            .expect("first leadership event should apply");

        let conflicting = state.apply(IndexedClusterEvent::new(
            cluster_id,
            ClusterEventIndex::initial().next(),
            ClusterEvent::LeadershipReconciled {
                leadership: ClusterLeadershipRecord::new(
                    ClusterTerm::initial(),
                    leader_beta.clone(),
                    ClusterEventIndex::initial().next(),
                )
                .with_lease_policy(ClusterLeaseTick::new(2), sample_leadership_lease_policy()),
            },
        ));
        assert!(
            matches!(
                conflicting,
                Err(ClusterHistoryError::SplitBrainLeadership { diagnostic })
                    if diagnostic == ClusterSplitBrainDiagnostic {
                        term: ClusterTerm::initial(),
                        current_leader_id: leader_alpha,
                        attempted_leader_id: leader_beta,
                    }
            ),
            "authoritative state should refuse conflicting same-term leadership"
        );
    }

    #[test]
    fn authoritative_state_refuses_out_of_order_application() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4201,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let executor = sample_membership_record(
            &cluster_id,
            4202,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Joining,
        );

        let mut state = ClusterState::new(cluster_id.clone());
        let first_apply = state.apply(IndexedClusterEvent::new(
            cluster_id.clone(),
            ClusterEventIndex::initial(),
            ClusterEvent::MembershipReconciled {
                membership: coordinator,
            },
        ));
        assert!(first_apply.is_ok(), "first contiguous event should apply");

        let out_of_order = state.apply(IndexedClusterEvent::new(
            cluster_id,
            ClusterEventIndex::initial().next().next(),
            ClusterEvent::MembershipReconciled {
                membership: executor,
            },
        ));
        assert!(
            matches!(
                out_of_order,
                Err(ClusterHistoryError::OutOfOrderEvent { expected, actual })
                    if expected == ClusterEventIndex::initial().next()
                        && actual == ClusterEventIndex::initial().next().next()
            ),
            "out-of-order apply should be refused"
        );
    }

    #[test]
    fn joining_node_can_authorize_its_own_membership_reconcile() {
        let cluster_id = sample_cluster_id();
        let joining = sample_membership_record(
            &cluster_id,
            4251,
            NodeRole::Mixed,
            ClusterMembershipStatus::Joining,
        );
        let state = ClusterState::new(cluster_id);
        let command = ClusterCommand::ReconcileMembership {
            membership: joining.clone(),
        };

        let authorization = state
            .authorize_command(
                &joining.identity.node_id,
                &command,
                &ClusterCommandAuthorizationPolicy::default(),
            )
            .expect("joining node should authorize its own membership");

        assert_eq!(
            authorization.authority_scope,
            ClusterCommandAuthorityScope::SelfNode
        );
        assert_eq!(authorization.submitter_node_id, joining.identity.node_id);
        assert_eq!(authorization.submitter_role, NodeRole::Mixed);
        assert_eq!(
            authorization.submitter_membership_status,
            ClusterMembershipStatus::Joining
        );
        assert!(authorization.coordinator_authority.is_none());
        assert!(!authorization.command_digest.is_empty());
        assert!(!authorization.authorization_policy_digest.is_empty());
        assert!(!authorization.stable_digest().is_empty());
    }

    #[test]
    fn coordinator_can_authorize_remove_member_command() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4252,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let executor = sample_membership_record(
            &cluster_id,
            4253,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );
        let mut state = ClusterState::new(cluster_id.clone());
        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial(),
                ClusterEvent::MembershipReconciled {
                    membership: coordinator.clone(),
                },
            ))
            .expect("coordinator membership should apply");
        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial().next(),
                ClusterEvent::MembershipReconciled {
                    membership: executor.clone(),
                },
            ))
            .expect("executor membership should apply");
        state
            .apply(IndexedClusterEvent::new(
                cluster_id,
                ClusterEventIndex::initial().next().next(),
                ClusterEvent::LeadershipReconciled {
                    leadership: ClusterLeadershipRecord::new(
                        ClusterTerm::initial(),
                        coordinator.identity.node_id.clone(),
                        ClusterEventIndex::initial().next(),
                    )
                    .with_lease_policy(ClusterLeaseTick::new(6), sample_leadership_lease_policy()),
                },
            ))
            .expect("leadership should apply");

        let authorization = state
            .authorize_command(
                &coordinator.identity.node_id,
                &ClusterCommand::RemoveMember {
                    node_id: executor.identity.node_id.clone(),
                    reason: String::from("draining"),
                },
                &ClusterCommandAuthorizationPolicy::default(),
            )
            .expect("coordinator should authorize removal");

        assert_eq!(
            authorization.authority_scope,
            ClusterCommandAuthorityScope::CoordinatorOnly
        );
        let authority = authorization
            .coordinator_authority
            .expect("coordinator authority should be attached");
        assert_eq!(authority.leader_id, coordinator.identity.node_id);
        assert_eq!(authority.term, ClusterTerm::initial());
    }

    #[test]
    fn non_coordinator_cannot_authorize_remove_member_command() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4254,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let executor = sample_membership_record(
            &cluster_id,
            4255,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );
        let mut state = ClusterState::new(cluster_id.clone());
        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial(),
                ClusterEvent::MembershipReconciled {
                    membership: coordinator.clone(),
                },
            ))
            .expect("coordinator membership should apply");
        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial().next(),
                ClusterEvent::MembershipReconciled {
                    membership: executor.clone(),
                },
            ))
            .expect("executor membership should apply");
        state
            .apply(IndexedClusterEvent::new(
                cluster_id,
                ClusterEventIndex::initial().next().next(),
                ClusterEvent::LeadershipReconciled {
                    leadership: ClusterLeadershipRecord::new(
                        ClusterTerm::initial(),
                        coordinator.identity.node_id.clone(),
                        ClusterEventIndex::initial().next(),
                    )
                    .with_lease_policy(ClusterLeaseTick::new(7), sample_leadership_lease_policy()),
                },
            ))
            .expect("leadership should apply");

        let refusal = state.authorize_command(
            &executor.identity.node_id,
            &ClusterCommand::RemoveMember {
                node_id: coordinator.identity.node_id.clone(),
                reason: String::from("stale"),
            },
            &ClusterCommandAuthorizationPolicy::default(),
        );
        assert!(
            matches!(
                refusal,
                Err(ClusterCommandAuthorizationError::Refused { refusal })
                    if refusal.code == ClusterCommandAuthorizationRefusalCode::CoordinatorRequired
                        && refusal.required_scope == ClusterCommandAuthorityScope::CoordinatorOnly
                        && refusal.submitter_node_id == executor.identity.node_id
            ),
            "non-coordinator member should be refused"
        );
    }

    #[test]
    fn non_peer_cannot_authorize_connection_reconcile_command() {
        let cluster_id = sample_cluster_id();
        let left = sample_membership_record(
            &cluster_id,
            4256,
            NodeRole::Mixed,
            ClusterMembershipStatus::Ready,
        );
        let right = sample_membership_record(
            &cluster_id,
            4257,
            NodeRole::Mixed,
            ClusterMembershipStatus::Ready,
        );
        let observer = sample_membership_record(
            &cluster_id,
            4258,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );
        let mut state = ClusterState::new(cluster_id.clone());
        for (index, membership) in [left.clone(), right.clone(), observer.clone()]
            .into_iter()
            .enumerate()
        {
            let event_index =
                (0..index).fold(ClusterEventIndex::initial(), |current, _| current.next());
            state
                .apply(IndexedClusterEvent::new(
                    cluster_id.clone(),
                    event_index,
                    ClusterEvent::MembershipReconciled { membership },
                ))
                .expect("membership should apply");
        }

        let refusal = state.authorize_command(
            &observer.identity.node_id,
            &ClusterCommand::ReconcileConnection {
                link: ClusterLink::new(
                    left.identity.node_id.clone(),
                    right.identity.node_id.clone(),
                    ClusterTransportClass::Tcp,
                    ClusterLinkStatus::Healthy,
                ),
            },
            &ClusterCommandAuthorizationPolicy::default(),
        );
        assert!(
            matches!(
                refusal,
                Err(ClusterCommandAuthorizationError::Refused { refusal })
                    if refusal.code == ClusterCommandAuthorizationRefusalCode::LinkPeerMismatch
                        && refusal.required_scope == ClusterCommandAuthorityScope::LinkPeer
                        && refusal.submitter_node_id == observer.identity.node_id
            ),
            "non-peer submitter should be refused for link commands"
        );
    }

    #[test]
    fn draining_member_cannot_request_snapshot_under_default_policy() {
        let cluster_id = sample_cluster_id();
        let draining = sample_membership_record(
            &cluster_id,
            4259,
            NodeRole::Mixed,
            ClusterMembershipStatus::Draining,
        );
        let mut state = ClusterState::new(cluster_id.clone());
        state
            .apply(IndexedClusterEvent::new(
                cluster_id,
                ClusterEventIndex::initial(),
                ClusterEvent::MembershipReconciled {
                    membership: draining.clone(),
                },
            ))
            .expect("draining membership should apply");

        let refusal = state.authorize_command(
            &draining.identity.node_id,
            &ClusterCommand::RequestSnapshot,
            &ClusterCommandAuthorizationPolicy::default(),
        );
        assert!(
            matches!(
                refusal,
                Err(ClusterCommandAuthorizationError::Refused { refusal })
                    if refusal.code
                        == ClusterCommandAuthorizationRefusalCode::SubmitterMembershipNotAllowed
                        && refusal.required_scope
                            == ClusterCommandAuthorityScope::ClusterMember
                        && refusal.submitter_node_id == draining.identity.node_id
            ),
            "default operator-managed policy should refuse draining members"
        );
    }

    #[test]
    fn authoritative_replay_preserves_command_provenance() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4260,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let executor = sample_membership_record(
            &cluster_id,
            4261,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );
        let policy = ClusterCommandAuthorizationPolicy::default();
        let coordinator_membership_command = ClusterCommand::ReconcileMembership {
            membership: coordinator.clone(),
        };
        let coordinator_membership_auth = ClusterState::new(cluster_id.clone())
            .authorize_command(
                &coordinator.identity.node_id,
                &coordinator_membership_command,
                &policy,
            )
            .expect("coordinator should authorize its own membership");
        let executor_membership_command = ClusterCommand::ReconcileMembership {
            membership: executor.clone(),
        };
        let executor_membership_auth = ClusterState::new(cluster_id.clone())
            .authorize_command(
                &executor.identity.node_id,
                &executor_membership_command,
                &policy,
            )
            .expect("executor should authorize its own membership");

        let mut authorization_state = ClusterState::new(cluster_id.clone());
        authorization_state
            .apply(
                IndexedClusterEvent::new(
                    cluster_id.clone(),
                    ClusterEventIndex::initial(),
                    ClusterEvent::MembershipReconciled {
                        membership: coordinator.clone(),
                    },
                )
                .with_command_authorization(coordinator_membership_auth.clone()),
            )
            .expect("coordinator membership should apply");
        authorization_state
            .apply(
                IndexedClusterEvent::new(
                    cluster_id.clone(),
                    ClusterEventIndex::initial().next(),
                    ClusterEvent::MembershipReconciled {
                        membership: executor.clone(),
                    },
                )
                .with_command_authorization(executor_membership_auth.clone()),
            )
            .expect("executor membership should apply");

        let link = ClusterLink::new(
            coordinator.identity.node_id.clone(),
            executor.identity.node_id.clone(),
            ClusterTransportClass::LanUdp,
            ClusterLinkStatus::Healthy,
        );
        let link_auth = authorization_state
            .authorize_command(
                &coordinator.identity.node_id,
                &ClusterCommand::ReconcileConnection { link: link.clone() },
                &policy,
            )
            .expect("coordinator should authorize one endpoint link reconcile");

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let _ = log.append_event_with_authorization(
            ClusterEvent::MembershipReconciled {
                membership: coordinator.clone(),
            },
            coordinator_membership_auth.clone(),
        );
        let _ = log.append_event_with_authorization(
            ClusterEvent::MembershipReconciled {
                membership: executor.clone(),
            },
            executor_membership_auth.clone(),
        );
        let _ = log.append_event_with_authorization(
            ClusterEvent::ConnectionReconciled { link: link.clone() },
            link_auth.clone(),
        );

        let replayed = log.replay().expect("authorized log should replay");
        assert_eq!(
            replayed.membership_provenance(&coordinator.identity.node_id),
            Some(&coordinator_membership_auth)
        );
        assert_eq!(
            replayed.membership_provenance(&executor.identity.node_id),
            Some(&executor_membership_auth)
        );
        assert_eq!(replayed.link_provenance(&link.key), Some(&link_auth));
        assert_eq!(
            replayed.snapshot().stable_digest(),
            ClusterState::from_snapshot(replayed.snapshot()).stable_digest()
        );
    }

    #[test]
    fn compaction_and_snapshot_recovery_preserve_command_provenance() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4262,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let executor = sample_membership_record(
            &cluster_id,
            4263,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );
        let policy = ClusterCommandAuthorizationPolicy::default();
        let coordinator_membership_auth = ClusterState::new(cluster_id.clone())
            .authorize_command(
                &coordinator.identity.node_id,
                &ClusterCommand::ReconcileMembership {
                    membership: coordinator.clone(),
                },
                &policy,
            )
            .expect("coordinator membership authorization should succeed");
        let executor_membership_auth = ClusterState::new(cluster_id.clone())
            .authorize_command(
                &executor.identity.node_id,
                &ClusterCommand::ReconcileMembership {
                    membership: executor.clone(),
                },
                &policy,
            )
            .expect("executor membership authorization should succeed");

        let mut state = ClusterState::new(cluster_id.clone());
        state
            .apply(
                IndexedClusterEvent::new(
                    cluster_id.clone(),
                    ClusterEventIndex::initial(),
                    ClusterEvent::MembershipReconciled {
                        membership: coordinator.clone(),
                    },
                )
                .with_command_authorization(coordinator_membership_auth.clone()),
            )
            .expect("coordinator membership should apply");
        state
            .apply(
                IndexedClusterEvent::new(
                    cluster_id.clone(),
                    ClusterEventIndex::initial().next(),
                    ClusterEvent::MembershipReconciled {
                        membership: executor.clone(),
                    },
                )
                .with_command_authorization(executor_membership_auth.clone()),
            )
            .expect("executor membership should apply");

        let leadership_auth = state
            .authorize_command(
                &coordinator.identity.node_id,
                &ClusterCommand::UpdateLeadership {
                    leader_id: coordinator.identity.node_id.clone(),
                    term: ClusterTerm::initial(),
                },
                &policy,
            )
            .expect("coordinator should authorize leadership update");

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let first = log.append_event_with_authorization(
            ClusterEvent::MembershipReconciled {
                membership: coordinator.clone(),
            },
            coordinator_membership_auth.clone(),
        );
        let _ = log.append_event_with_authorization(
            ClusterEvent::MembershipReconciled {
                membership: executor.clone(),
            },
            executor_membership_auth.clone(),
        );
        let _ = log.append_event(ClusterEvent::NodeTelemetryReconciled {
            telemetry: ClusterNodeTelemetry::new(executor.identity.node_id.clone())
                .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready),
        });
        let leadership_event = log.append_event_with_authorization(
            ClusterEvent::LeadershipReconciled {
                leadership: ClusterLeadershipRecord::new(
                    ClusterTerm::initial(),
                    coordinator.identity.node_id.clone(),
                    ClusterEventIndex::initial().next().next(),
                )
                .with_lease_policy(ClusterLeaseTick::new(9), sample_leadership_lease_policy()),
            },
            leadership_auth.clone(),
        );
        let full_snapshot = log.snapshot().expect("full snapshot");
        let full_digest = full_snapshot.stable_digest();
        let compacted_snapshot = log
            .compact(&sample_recovery_policy())
            .expect("compaction should work")
            .expect("compaction should produce a snapshot");
        assert_eq!(
            compacted_snapshot
                .membership_provenance
                .get(&coordinator.identity.node_id),
            Some(&coordinator_membership_auth)
        );

        let response = log
            .catchup_response(
                &ClusterCatchupRequest::new(
                    cluster_id,
                    executor.identity.node_id.clone(),
                    Some(first.index),
                    ClusterSchemaVersion::initial(),
                    8,
                ),
                &sample_recovery_policy(),
            )
            .expect("snapshot catchup response should build");
        match response.payload {
            ClusterCatchupPayload::Snapshot {
                snapshot,
                tail_events,
                ..
            } => {
                let recovered = ClusterState::recover(*snapshot, &tail_events)
                    .expect("snapshot plus tail should recover current state");
                assert_eq!(
                    recovered.membership_provenance(&coordinator.identity.node_id),
                    Some(&coordinator_membership_auth)
                );
                assert_eq!(
                    recovered.membership_provenance(&executor.identity.node_id),
                    Some(&executor_membership_auth)
                );
                assert_eq!(recovered.leadership_provenance(), Some(&leadership_auth));
                assert_eq!(
                    recovered.last_applied_event_index(),
                    Some(leadership_event.index)
                );
                assert_eq!(recovered.stable_digest(), full_digest);
            }
            payload => panic!("expected snapshot payload, got {payload:?}"),
        }
    }

    #[test]
    fn authoritative_event_log_refuses_non_contiguous_external_events() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4301,
            NodeRole::Mixed,
            ClusterMembershipStatus::Ready,
        );

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let push = log.push_indexed_event(IndexedClusterEvent::new(
            cluster_id,
            ClusterEventIndex::initial().next(),
            ClusterEvent::MembershipReconciled {
                membership: coordinator,
            },
        ));
        assert!(
            matches!(
                push,
                Err(ClusterHistoryError::OutOfOrderEvent { expected, actual })
                    if expected == ClusterEventIndex::initial()
                        && actual == ClusterEventIndex::initial().next()
            ),
            "external indexed events should be contiguous from the first slot"
        );
    }

    #[test]
    fn compaction_and_catchup_can_restore_rejoined_node_state() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4401,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let executor = sample_membership_record(
            &cluster_id,
            4402,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );
        let telemetry = ClusterNodeTelemetry::new(executor.identity.node_id.clone())
            .with_memory(Some(64 * 1024), Some(32 * 1024))
            .with_cpu_logical_cores(16)
            .with_accelerator_count(2)
            .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready)
            .with_stability_posture(ClusterStabilityPosture::Stable);
        let artifact = ClusterArtifactReference::new("gpt-oss", "artifact-digest")
            .with_provenance_digest("prov-digest")
            .with_governance_digest("gov-digest")
            .with_supply_policy_digest("supply-digest");
        let residency = ClusterArtifactResidencyRecord::new(
            executor.identity.node_id.clone(),
            artifact,
            ClusterArtifactResidencyStatus::Resident,
        );
        let link = ClusterLink::new(
            coordinator.identity.node_id.clone(),
            executor.identity.node_id.clone(),
            ClusterTransportClass::Rdma,
            ClusterLinkStatus::Healthy,
        )
        .with_link_class(ClusterLinkClass::Rdma)
        .with_latency_us(50)
        .with_bandwidth_mbps(40_000)
        .with_stability_posture(ClusterStabilityPosture::Stable);

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let _ = log.append_event(ClusterEvent::MembershipReconciled {
            membership: coordinator.clone(),
        });
        let _ = log.append_event(ClusterEvent::MembershipReconciled {
            membership: executor.clone(),
        });
        let _ = log.append_event(ClusterEvent::ConnectionReconciled { link });
        let telemetry_event = log.append_event(ClusterEvent::NodeTelemetryReconciled { telemetry });
        let compacted_through =
            log.append_event(ClusterEvent::ArtifactResidencyReconciled { residency });
        let head = log.append_event(ClusterEvent::LeadershipReconciled {
            leadership: ClusterLeadershipRecord::new(
                ClusterTerm::initial(),
                coordinator.identity.node_id.clone(),
                compacted_through.index,
            ),
        });

        let full_snapshot = log.snapshot().expect("full snapshot");
        let full_digest = full_snapshot.stable_digest();
        let policy = sample_recovery_policy();
        let compacted_snapshot = log
            .compact(&policy)
            .expect("compaction should work")
            .expect("enough events to compact");
        assert_eq!(
            compacted_snapshot.last_applied_event_index,
            Some(telemetry_event.index)
        );
        assert_eq!(
            log.replay()
                .expect("replay after compaction")
                .stable_digest(),
            full_digest
        );

        let response = log
            .catchup_response(
                &ClusterCatchupRequest::new(
                    cluster_id,
                    executor.identity.node_id.clone(),
                    compacted_snapshot.last_applied_event_index,
                    ClusterSchemaVersion::initial(),
                    16,
                ),
                &policy,
            )
            .expect("catchup response should build");
        assert_eq!(response.head_event_index, Some(head.index));
        assert_eq!(response.disposition, ClusterRecoveryDisposition::CatchUp);

        match response.payload {
            ClusterCatchupPayload::Events { events } => {
                assert_eq!(events.len(), 2);
                let recovered = ClusterState::recover(compacted_snapshot, &events)
                    .expect("rejoin recovery should apply retained tail");
                assert_eq!(recovered.stable_digest(), full_digest);
            }
            payload => panic!("expected event catchup payload, got {payload:?}"),
        }
    }

    #[test]
    fn catchup_request_behind_compaction_floor_installs_snapshot() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4501,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let executor = sample_membership_record(
            &cluster_id,
            4502,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let first = log.append_event(ClusterEvent::MembershipReconciled {
            membership: coordinator.clone(),
        });
        let _ = log.append_event(ClusterEvent::MembershipReconciled {
            membership: executor.clone(),
        });
        let _ = log.append_event(ClusterEvent::NodeTelemetryReconciled {
            telemetry: ClusterNodeTelemetry::new(executor.identity.node_id.clone())
                .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready),
        });
        let _ = log.append_event(ClusterEvent::ArtifactResidencyReconciled {
            residency: ClusterArtifactResidencyRecord::new(
                executor.identity.node_id.clone(),
                ClusterArtifactReference::new("gpt-oss", "artifact-digest"),
                ClusterArtifactResidencyStatus::CopyRequired,
            )
            .with_transfer_method(ClusterArtifactTransferMethod::PeerCopy)
            .with_detail("copy required before scheduling"),
        });

        let full_digest = log.snapshot().expect("snapshot").stable_digest();
        let policy = sample_recovery_policy();
        let compacted_snapshot = log
            .compact(&policy)
            .expect("compaction")
            .expect("snapshot should be generated");

        let response = log
            .catchup_response(
                &ClusterCatchupRequest::new(
                    cluster_id,
                    coordinator.identity.node_id.clone(),
                    Some(first.index),
                    ClusterSchemaVersion::initial(),
                    16,
                ),
                &policy,
            )
            .expect("snapshot catchup response");

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
                assert_eq!(*snapshot, compacted_snapshot);
                assert!(matches!(
                    reason,
                    ClusterRecoveryReason::CompactionBoundary {
                        compacted_through: Some(_)
                    }
                ));
                let recovered = ClusterState::recover(*snapshot, &tail_events)
                    .expect("snapshot plus tail should recover current state");
                assert_eq!(recovered.stable_digest(), full_digest);
            }
            payload => panic!("expected snapshot payload, got {payload:?}"),
        }
    }

    #[test]
    fn schema_version_mismatch_forces_full_resync() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4601,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let mut log = ClusterEventLog::new(cluster_id.clone());
        let head = log.append_event(ClusterEvent::MembershipReconciled {
            membership: coordinator,
        });

        let response = log
            .catchup_response(
                &ClusterCatchupRequest::new(
                    cluster_id,
                    crate::NodeId::random(),
                    Some(head.index),
                    ClusterSchemaVersion::new(2, 0),
                    16,
                ),
                &sample_recovery_policy(),
            )
            .expect("response should be generated");

        assert_eq!(response.disposition, ClusterRecoveryDisposition::FullResync);
        match response.payload {
            ClusterCatchupPayload::Snapshot { reason, .. } => {
                assert!(matches!(
                    reason,
                    ClusterRecoveryReason::SchemaVersionMismatch { .. }
                ));
            }
            payload => panic!("expected snapshot resync payload, got {payload:?}"),
        }
    }

    #[test]
    fn artifact_residency_is_separate_from_topology_digest() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4701,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let executor = sample_membership_record(
            &cluster_id,
            4702,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );
        let mut state = ClusterState::new(cluster_id.clone());
        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial(),
                ClusterEvent::MembershipReconciled {
                    membership: coordinator.clone(),
                },
            ))
            .expect("coordinator should apply");
        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial().next(),
                ClusterEvent::MembershipReconciled {
                    membership: executor.clone(),
                },
            ))
            .expect("executor should apply");
        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial().next().next(),
                ClusterEvent::ConnectionReconciled {
                    link: ClusterLink::new(
                        coordinator.identity.node_id.clone(),
                        executor.identity.node_id.clone(),
                        ClusterTransportClass::LanUdp,
                        ClusterLinkStatus::Healthy,
                    )
                    .with_link_class(ClusterLinkClass::Ethernet)
                    .with_latency_us(250)
                    .with_bandwidth_mbps(1_000),
                },
            ))
            .expect("link should apply");
        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial().next().next().next(),
                ClusterEvent::NodeTelemetryReconciled {
                    telemetry: ClusterNodeTelemetry::new(executor.identity.node_id.clone())
                        .with_memory(Some(128 * 1024), Some(96 * 1024))
                        .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready),
                },
            ))
            .expect("telemetry should apply");

        let topology_digest_before = state.topology_digest();
        let stable_digest_before = state.stable_digest();
        state
            .apply(IndexedClusterEvent::new(
                cluster_id,
                ClusterEventIndex::initial().next().next().next().next(),
                ClusterEvent::ArtifactResidencyReconciled {
                    residency: ClusterArtifactResidencyRecord::new(
                        executor.identity.node_id,
                        ClusterArtifactReference::new("gpt-oss", "artifact-digest")
                            .with_provenance_digest("prov-digest")
                            .with_governance_digest("gov-digest"),
                        ClusterArtifactResidencyStatus::PullRequired,
                    )
                    .with_transfer_method(ClusterArtifactTransferMethod::OciPull)
                    .with_detail("pull required before load"),
                },
            ))
            .expect("artifact residency should apply");

        assert_eq!(state.topology_digest(), topology_digest_before);
        assert_ne!(state.stable_digest(), stable_digest_before);
    }

    #[test]
    fn replay_keeps_discovery_candidates_separate_from_membership_truth() {
        let cluster_id = sample_cluster_id();
        let candidate = sample_discovery_candidate_record(
            &cluster_id,
            crate::NodeId::new("candidate-a"),
            4751,
            NodeRole::ExecutorOnly,
        );
        let mut log = ClusterEventLog::new(cluster_id.clone());
        let event = log.append_event(ClusterEvent::DiscoveryCandidateReconciled {
            candidate: candidate.clone(),
        });

        let replayed = log.replay().expect("candidate ledger should replay");
        assert_eq!(replayed.last_applied_event_index(), Some(event.index));
        assert_eq!(replayed.memberships().len(), 0);
        assert_eq!(
            replayed
                .discovery_candidates()
                .get(&candidate.candidate.node_id),
            Some(&candidate)
        );
    }

    #[test]
    fn admitting_discovery_candidate_promotes_membership_without_dropping_candidate_truth() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4752,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let policy = ClusterCommandAuthorizationPolicy::default();
        let mut state = sample_state_with_coordinator_authority(&cluster_id, &coordinator, &policy);
        let candidate_membership = sample_membership_record(
            &cluster_id,
            4753,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Joining,
        );
        let candidate = sample_discovery_candidate_record(
            &cluster_id,
            candidate_membership.identity.node_id.clone(),
            5753,
            candidate_membership.identity.role,
        );

        let candidate_authorization = state
            .authorize_command(
                &coordinator.identity.node_id,
                &ClusterCommand::ReconcileDiscoveryCandidate {
                    candidate: candidate.clone(),
                },
                &policy,
            )
            .expect("coordinator should authorize candidate reconciliation");
        state
            .apply(
                IndexedClusterEvent::new(
                    cluster_id.clone(),
                    ClusterEventIndex::initial().next().next(),
                    ClusterEvent::DiscoveryCandidateReconciled {
                        candidate: candidate.clone(),
                    },
                )
                .with_command_authorization(candidate_authorization),
            )
            .expect("candidate reconcile should apply");

        let admission_authorization = state
            .authorize_command(
                &coordinator.identity.node_id,
                &ClusterCommand::AdmitDiscoveryCandidate {
                    node_id: candidate.candidate.node_id.clone(),
                    advertised_addr: candidate_membership.advertised_addr,
                    membership_status: candidate_membership.status,
                },
                &policy,
            )
            .expect("coordinator should authorize candidate admission");
        state
            .apply(
                IndexedClusterEvent::new(
                    cluster_id.clone(),
                    ClusterEventIndex::initial().next().next().next(),
                    ClusterEvent::DiscoveryCandidateAdmitted {
                        node_id: candidate.candidate.node_id.clone(),
                        membership: candidate_membership.clone(),
                    },
                )
                .with_command_authorization(admission_authorization.clone()),
            )
            .expect("candidate admission should apply");

        assert_eq!(
            state.memberships().get(&candidate.candidate.node_id),
            Some(&candidate_membership)
        );
        assert_eq!(
            state
                .discovery_candidates()
                .get(&candidate.candidate.node_id)
                .map(|candidate| candidate.status),
            Some(ClusterDiscoveredCandidateStatus::Accepted)
        );
        assert_eq!(
            state
                .discovery_candidates()
                .get(&candidate.candidate.node_id)
                .and_then(|candidate| candidate.detail.as_deref()),
            Some("admitted_into_membership")
        );
        assert_eq!(
            state.discovery_candidate_provenance(&candidate.candidate.node_id),
            Some(&admission_authorization)
        );
        assert_eq!(
            state.membership_provenance(&candidate.candidate.node_id),
            Some(&admission_authorization)
        );
    }

    #[test]
    fn admitting_missing_discovery_candidate_is_refused() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4754,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let policy = ClusterCommandAuthorizationPolicy::default();
        let mut state = sample_state_with_coordinator_authority(&cluster_id, &coordinator, &policy);
        let candidate_membership = sample_membership_record(
            &cluster_id,
            4755,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Joining,
        );
        let candidate_node_id = candidate_membership.identity.node_id.clone();

        let result = state.apply(IndexedClusterEvent::new(
            cluster_id,
            ClusterEventIndex::initial().next().next(),
            ClusterEvent::DiscoveryCandidateAdmitted {
                node_id: candidate_node_id.clone(),
                membership: candidate_membership,
            },
        ));

        assert_eq!(
            result,
            Err(ClusterHistoryError::DiscoveryCandidateMissing {
                node_id: candidate_node_id,
            })
        );
    }

    #[test]
    fn expired_discovery_candidate_cannot_be_admitted() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4756,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let policy = ClusterCommandAuthorizationPolicy::default();
        let mut state = sample_state_with_coordinator_authority(&cluster_id, &coordinator, &policy);
        let candidate_membership = sample_membership_record(
            &cluster_id,
            4757,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Joining,
        );
        let candidate_node_id = candidate_membership.identity.node_id.clone();
        let candidate = sample_discovery_candidate_record(
            &cluster_id,
            candidate_node_id.clone(),
            5757,
            candidate_membership.identity.role,
        )
        .with_status(ClusterDiscoveredCandidateStatus::Expired)
        .with_detail("introduction_expired");
        state
            .apply(IndexedClusterEvent::new(
                cluster_id.clone(),
                ClusterEventIndex::initial().next().next(),
                ClusterEvent::DiscoveryCandidateReconciled { candidate },
            ))
            .expect("expired candidate should reconcile");

        let result = state.apply(IndexedClusterEvent::new(
            cluster_id,
            ClusterEventIndex::initial().next().next().next(),
            ClusterEvent::DiscoveryCandidateAdmitted {
                node_id: candidate_membership.identity.node_id.clone(),
                membership: candidate_membership,
            },
        ));

        assert_eq!(
            result,
            Err(ClusterHistoryError::DiscoveryCandidateAdmissionRefused {
                node_id: candidate_node_id,
                status: ClusterDiscoveredCandidateStatus::Expired,
            })
        );
    }

    #[test]
    fn snapshot_recovery_preserves_discovery_candidate_truth_and_provenance() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4758,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let policy = ClusterCommandAuthorizationPolicy::default();
        let mut authorization_state =
            sample_state_with_coordinator_authority(&cluster_id, &coordinator, &policy);
        let candidate_membership = sample_membership_record(
            &cluster_id,
            4759,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );
        let candidate = sample_discovery_candidate_record(
            &cluster_id,
            candidate_membership.identity.node_id.clone(),
            5759,
            candidate_membership.identity.role,
        );
        let candidate_authorization = authorization_state
            .authorize_command(
                &coordinator.identity.node_id,
                &ClusterCommand::ReconcileDiscoveryCandidate {
                    candidate: candidate.clone(),
                },
                &policy,
            )
            .expect("candidate authorization should succeed");
        authorization_state
            .apply(
                IndexedClusterEvent::new(
                    cluster_id.clone(),
                    ClusterEventIndex::initial().next().next(),
                    ClusterEvent::DiscoveryCandidateReconciled {
                        candidate: candidate.clone(),
                    },
                )
                .with_command_authorization(candidate_authorization.clone()),
            )
            .expect("candidate event should apply");
        let admission_authorization = authorization_state
            .authorize_command(
                &coordinator.identity.node_id,
                &ClusterCommand::AdmitDiscoveryCandidate {
                    node_id: candidate.candidate.node_id.clone(),
                    advertised_addr: candidate_membership.advertised_addr,
                    membership_status: candidate_membership.status,
                },
                &policy,
            )
            .expect("admission authorization should succeed");

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let coordinator_membership_authorization = ClusterState::new(cluster_id.clone())
            .authorize_command(
                &coordinator.identity.node_id,
                &ClusterCommand::ReconcileMembership {
                    membership: coordinator.clone(),
                },
                &policy,
            )
            .expect("coordinator membership authorization should succeed");
        let coordinator_leadership_authorization =
            sample_state_with_coordinator_authority(&cluster_id, &coordinator, &policy)
                .leadership_provenance()
                .cloned()
                .expect("coordinator leadership provenance should exist");
        let _ = log.append_event_with_authorization(
            ClusterEvent::MembershipReconciled {
                membership: coordinator.clone(),
            },
            coordinator_membership_authorization,
        );
        let _ = log.append_event_with_authorization(
            ClusterEvent::LeadershipReconciled {
                leadership: ClusterLeadershipRecord::new(
                    ClusterTerm::initial(),
                    coordinator.identity.node_id.clone(),
                    ClusterEventIndex::initial(),
                ),
            },
            coordinator_leadership_authorization,
        );
        let _ = log.append_event_with_authorization(
            ClusterEvent::DiscoveryCandidateReconciled {
                candidate: candidate.clone(),
            },
            candidate_authorization.clone(),
        );
        let _ = log.append_event_with_authorization(
            ClusterEvent::DiscoveryCandidateAdmitted {
                node_id: candidate.candidate.node_id.clone(),
                membership: candidate_membership.clone(),
            },
            admission_authorization.clone(),
        );
        let _ = log.append_event(ClusterEvent::NodeTelemetryReconciled {
            telemetry: ClusterNodeTelemetry::new(candidate.candidate.node_id.clone())
                .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready),
        });

        let full_digest = log.snapshot().expect("full snapshot").stable_digest();
        let compacted_snapshot = log
            .compact(&sample_recovery_policy())
            .expect("compaction should work")
            .expect("compaction should produce snapshot");
        assert_eq!(
            compacted_snapshot
                .discovery_candidate_provenance
                .get(&candidate.candidate.node_id),
            Some(&candidate_authorization)
        );

        let response = log
            .catchup_response(
                &ClusterCatchupRequest::new(
                    cluster_id.clone(),
                    candidate.candidate.node_id.clone(),
                    compacted_snapshot.last_applied_event_index,
                    ClusterSchemaVersion::initial(),
                    8,
                ),
                &sample_recovery_policy(),
            )
            .expect("catchup response should build");
        match response.payload {
            ClusterCatchupPayload::Events { events } => {
                let recovered = ClusterState::recover(compacted_snapshot, &events)
                    .expect("snapshot plus tail should recover discovery state");
                assert_eq!(recovered.stable_digest(), full_digest);
                assert_eq!(
                    recovered
                        .discovery_candidates()
                        .get(&candidate.candidate.node_id)
                        .map(|candidate| candidate.status),
                    Some(ClusterDiscoveredCandidateStatus::Accepted)
                );
                assert_eq!(
                    recovered.discovery_candidate_provenance(&candidate.candidate.node_id),
                    Some(&admission_authorization)
                );
                assert_eq!(
                    recovered.membership_provenance(&candidate.candidate.node_id),
                    Some(&admission_authorization)
                );
            }
            payload => panic!("expected event payload, got {payload:?}"),
        }
    }

    #[test]
    fn signed_catchup_response_verifies_and_recovers_current_state() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4701,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let executor = sample_membership_record(
            &cluster_id,
            4702,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let _ = log.append_event(ClusterEvent::MembershipReconciled {
            membership: coordinator.clone(),
        });
        let _ = log.append_event(ClusterEvent::MembershipReconciled {
            membership: executor.clone(),
        });

        let request = ClusterCatchupRequest::new(
            cluster_id.clone(),
            crate::NodeId::new("recovering-node"),
            None,
            ClusterSchemaVersion::initial(),
            8,
        );
        let response = log
            .catchup_response(&request, &sample_recovery_policy())
            .unwrap_or_else(|_| unreachable!("catchup response should build"));
        let signing_key = sample_signing_key(19);
        let signer = recovery_signer(
            &cluster_id,
            coordinator.identity.node_id.as_str(),
            NodeRole::CoordinatorOnly,
            &signing_key,
        );
        let expected_signer = ConfiguredClusterPeer::new(
            signer.node_id.clone(),
            SocketAddr::from(([127, 0, 0, 1], 4701)),
            signer.auth_public_key.clone(),
        );
        let envelope = response
            .clone()
            .sign(signer, &signing_key, 7)
            .unwrap_or_else(|_| unreachable!("recovery response should sign"));
        let mut replay_window = ClusterRecoveryReplayWindow::new(8);

        let verified = envelope.verify(
            &cluster_id,
            &request.requester_id,
            &expected_signer,
            &mut replay_window,
        );
        assert!(verified.is_ok(), "signed recovery should verify");

        match envelope.response.payload {
            ClusterCatchupPayload::Events { events } => {
                let recovered = ClusterState::recover(ClusterSnapshot::new(cluster_id), &events)
                    .unwrap_or_else(|_| unreachable!("verified catchup should recover"));
                assert_eq!(recovered.memberships().len(), 2);
            }
            payload => panic!("expected events payload, got {payload:?}"),
        }
    }

    #[test]
    fn tampered_signed_catchup_response_is_refused() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4801,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let _ = log.append_event(ClusterEvent::MembershipReconciled {
            membership: coordinator.clone(),
        });
        let request = ClusterCatchupRequest::new(
            cluster_id.clone(),
            crate::NodeId::new("recovering-node"),
            None,
            ClusterSchemaVersion::initial(),
            8,
        );
        let response = log
            .catchup_response(&request, &sample_recovery_policy())
            .unwrap_or_else(|_| unreachable!("catchup response should build"));
        let signing_key = sample_signing_key(23);
        let signer = recovery_signer(
            &cluster_id,
            coordinator.identity.node_id.as_str(),
            NodeRole::CoordinatorOnly,
            &signing_key,
        );
        let expected_signer = ConfiguredClusterPeer::new(
            signer.node_id.clone(),
            SocketAddr::from(([127, 0, 0, 1], 4801)),
            signer.auth_public_key.clone(),
        );
        let mut envelope = response
            .sign(signer, &signing_key, 9)
            .unwrap_or_else(|_| unreachable!("recovery response should sign"));
        envelope.response.head_event_index = None;
        let mut replay_window = ClusterRecoveryReplayWindow::new(8);

        let verified = envelope.verify(
            &cluster_id,
            &request.requester_id,
            &expected_signer,
            &mut replay_window,
        );
        assert!(
            matches!(
                verified,
                Err(ClusterRecoveryEnvelopeError::PayloadDigestMismatch { .. })
            ),
            "tampered recovery should be refused"
        );
    }

    #[test]
    fn replayed_signed_catchup_response_is_refused() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            4901,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let _ = log.append_event(ClusterEvent::MembershipReconciled {
            membership: coordinator.clone(),
        });
        let request = ClusterCatchupRequest::new(
            cluster_id.clone(),
            crate::NodeId::new("recovering-node"),
            None,
            ClusterSchemaVersion::initial(),
            8,
        );
        let response = log
            .catchup_response(&request, &sample_recovery_policy())
            .unwrap_or_else(|_| unreachable!("catchup response should build"));
        let signing_key = sample_signing_key(29);
        let signer = recovery_signer(
            &cluster_id,
            coordinator.identity.node_id.as_str(),
            NodeRole::CoordinatorOnly,
            &signing_key,
        );
        let expected_signer = ConfiguredClusterPeer::new(
            signer.node_id.clone(),
            SocketAddr::from(([127, 0, 0, 1], 4901)),
            signer.auth_public_key.clone(),
        );
        let envelope = response
            .sign(signer, &signing_key, 11)
            .unwrap_or_else(|_| unreachable!("recovery response should sign"));
        let mut replay_window = ClusterRecoveryReplayWindow::new(8);

        let first_verify = envelope.verify(
            &cluster_id,
            &request.requester_id,
            &expected_signer,
            &mut replay_window,
        );
        assert!(first_verify.is_ok(), "first verify should succeed");
        let replayed = envelope.verify(
            &cluster_id,
            &request.requester_id,
            &expected_signer,
            &mut replay_window,
        );
        assert!(
            matches!(
                replayed,
                Err(ClusterRecoveryEnvelopeError::ReplayDetected {
                    highest_seen: 11,
                    attempted: 11
                })
            ),
            "replayed recovery should be refused"
        );
    }

    #[test]
    fn authoritative_event_log_round_trips_through_json_and_preserves_catchup() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            5001,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );
        let executor = sample_membership_record(
            &cluster_id,
            5002,
            NodeRole::ExecutorOnly,
            ClusterMembershipStatus::Ready,
        );

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let _ = log.append_event(ClusterEvent::MembershipReconciled {
            membership: coordinator.clone(),
        });
        let _ = log.append_event(ClusterEvent::MembershipReconciled {
            membership: executor.clone(),
        });
        let telemetry_event = log.append_event(ClusterEvent::NodeTelemetryReconciled {
            telemetry: ClusterNodeTelemetry::new(executor.identity.node_id.clone())
                .with_backend_readiness("cuda", ClusterBackendReadinessStatus::Ready)
                .with_cpu_logical_cores(32),
        });
        let _ = log.append_event(ClusterEvent::LeadershipReconciled {
            leadership: ClusterLeadershipRecord::new(
                ClusterTerm::initial(),
                coordinator.identity.node_id.clone(),
                telemetry_event.index,
            ),
        });

        let policy = sample_recovery_policy();
        let authoritative_digest = log
            .replay()
            .expect("authoritative replay should work")
            .stable_digest();
        let compacted_snapshot = log
            .compact(&policy)
            .expect("compaction should succeed")
            .expect("compaction should emit a snapshot");
        let request = ClusterCatchupRequest::new(
            cluster_id.clone(),
            executor.identity.node_id.clone(),
            compacted_snapshot.last_applied_event_index,
            ClusterSchemaVersion::initial(),
            8,
        );
        let expected_response = log
            .catchup_response(&request, &policy)
            .expect("catchup response should build");
        assert_eq!(
            expected_response.disposition,
            ClusterRecoveryDisposition::CatchUp
        );

        let tempdir = tempdir().expect("tempdir should exist");
        let path = tempdir.path().join("cluster/state/ordered-state.json");
        log.store_json(&path)
            .expect("persisted ordered log should store");

        let encoded = fs::read_to_string(&path).expect("persisted file should be readable");
        assert!(
            encoded.contains("\"schema_version\""),
            "stored log should remain inspectable JSON"
        );

        let loaded = ClusterEventLog::load_json(&path).expect("persisted log should load");
        assert_eq!(loaded, log);
        assert_eq!(
            loaded
                .replay()
                .expect("loaded replay should work")
                .stable_digest(),
            authoritative_digest
        );

        let response_after_restart = loaded
            .catchup_response(&request, &policy)
            .expect("catchup after restart should build");
        assert_eq!(response_after_restart, expected_response);

        match response_after_restart.payload {
            ClusterCatchupPayload::Events { events } => {
                let recovered = ClusterState::recover(compacted_snapshot, &events)
                    .expect("snapshot plus tail should recover current state");
                assert_eq!(recovered.stable_digest(), authoritative_digest);
            }
            payload => panic!("expected events payload, got {payload:?}"),
        }
    }

    #[test]
    fn authoritative_event_log_load_rejects_invalid_json() {
        let tempdir = tempdir().expect("tempdir should exist");
        let path = tempdir.path().join("cluster/state/ordered-state.json");
        fs::create_dir_all(path.parent().expect("fixture path should have a parent"))
            .expect("fixture parent should exist");
        fs::write(&path, b"{not valid json").expect("invalid fixture should write");

        let load = ClusterEventLog::load_json(&path);
        assert!(
            matches!(
                load,
                Err(ClusterHistoryError::PersistenceFormat {
                    category,
                    line,
                    column,
                    ..
                }) if category == "Syntax" && line == 1 && column > 0
            ),
            "malformed persisted JSON should be surfaced as a stable format error"
        );
    }

    #[test]
    fn authoritative_event_log_load_rejects_snapshot_schema_mismatch() {
        let cluster_id = sample_cluster_id();
        let coordinator = sample_membership_record(
            &cluster_id,
            5101,
            NodeRole::CoordinatorOnly,
            ClusterMembershipStatus::Ready,
        );

        let mut log = ClusterEventLog::new(cluster_id.clone());
        let _ = log.append_event(ClusterEvent::MembershipReconciled {
            membership: coordinator,
        });
        let mut snapshot = log.snapshot().expect("snapshot should build");
        snapshot.schema_version = ClusterSchemaVersion::new(9, 0);

        let invalid = ClusterEventLog {
            cluster_id,
            schema_version: ClusterSchemaVersion::initial(),
            compacted_snapshot: Some(snapshot),
            events: Vec::new(),
        };

        let tempdir = tempdir().expect("tempdir should exist");
        let path = tempdir.path().join("cluster/state/ordered-state.json");
        fs::create_dir_all(path.parent().expect("fixture path should have a parent"))
            .expect("fixture parent should exist");
        let encoded =
            serde_json::to_vec_pretty(&invalid).expect("invalid persisted fixture should encode");
        fs::write(&path, encoded).expect("invalid fixture should write");

        let load = ClusterEventLog::load_json(&path);
        assert!(
            matches!(
                load,
                Err(ClusterHistoryError::SnapshotSchemaVersionMismatch { expected, actual })
                    if expected == ClusterSchemaVersion::initial()
                        && actual == ClusterSchemaVersion::new(9, 0)
            ),
            "persisted snapshots should keep the log schema boundary honest"
        );
    }
}
