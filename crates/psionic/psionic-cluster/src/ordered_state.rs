use std::{collections::BTreeMap, net::SocketAddr};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{ClusterId, ClusterJoinRefusal, ClusterNodeIdentity, NodeId, PeerSnapshot};

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

/// Leader/coordinator fact recorded in authoritative cluster state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterLeadershipRecord {
    /// Election term for the leader.
    pub term: ClusterTerm,
    /// Node ID currently acting as the ordering authority.
    pub leader_id: NodeId,
    /// Highest committed global event index visible to the leader.
    pub committed_event_index: ClusterEventIndex,
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

/// Imperative cluster command submitted before the leader orders a result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClusterCommand {
    /// Request that the leader reconcile one membership record.
    ReconcileMembership { membership: ClusterMembershipRecord },
    /// Request that the leader remove one node from authoritative state.
    RemoveMember { node_id: NodeId, reason: String },
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

/// Authoritative global cluster event ordered by one leader.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClusterEvent {
    /// Insert or replace one cluster membership record.
    MembershipReconciled { membership: ClusterMembershipRecord },
    /// Remove one node from authoritative state.
    MembershipRemoved { node_id: NodeId, reason: String },
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
}

impl IndexedClusterEvent {
    /// Creates one indexed authoritative cluster event.
    #[must_use]
    pub fn new(cluster_id: ClusterId, index: ClusterEventIndex, event: ClusterEvent) -> Self {
        Self {
            cluster_id,
            index,
            event,
        }
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
    /// Current cluster links by canonical node pair.
    pub links: BTreeMap<ClusterLinkKey, ClusterLink>,
    /// Current node telemetry by node ID.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub telemetry: BTreeMap<NodeId, ClusterNodeTelemetry>,
    /// Current artifact residency/staging facts by node and artifact.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub artifact_residency: BTreeMap<ClusterArtifactResidencyKey, ClusterArtifactResidencyRecord>,
    /// Current leader/coordinator truth, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leadership: Option<ClusterLeadershipRecord>,
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
            links: BTreeMap::new(),
            telemetry: BTreeMap::new(),
            artifact_residency: BTreeMap::new(),
            leadership: None,
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
        hasher.update(self.artifact_residency_digest().as_bytes());
        hasher.update(b"|");
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

    /// Returns the current leader/coordinator record, when one exists.
    #[must_use]
    pub fn leadership(&self) -> Option<&ClusterLeadershipRecord> {
        self.snapshot.leadership.as_ref()
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

    /// Applies one indexed authoritative event using contiguous-index discipline.
    pub fn apply(&mut self, indexed_event: IndexedClusterEvent) -> Result<(), ClusterHistoryError> {
        validate_event_owner(self.cluster_id(), &indexed_event.cluster_id)?;
        validate_contiguous_index(self.last_applied_event_index(), indexed_event.index)?;

        match indexed_event.event {
            ClusterEvent::MembershipReconciled { membership } => {
                self.snapshot
                    .memberships
                    .insert(membership.identity.node_id.clone(), membership);
            }
            ClusterEvent::MembershipRemoved { node_id, .. } => {
                self.snapshot.memberships.remove(&node_id);
                self.snapshot
                    .links
                    .retain(|key, _| key.left_node_id != node_id && key.right_node_id != node_id);
                self.snapshot.telemetry.remove(&node_id);
                self.snapshot
                    .artifact_residency
                    .retain(|key, _| key.node_id != node_id);
                if self
                    .snapshot
                    .leadership
                    .as_ref()
                    .is_some_and(|leadership| leadership.leader_id == node_id)
                {
                    self.snapshot.leadership = None;
                }
            }
            ClusterEvent::ConnectionReconciled { link } => {
                self.snapshot.links.insert(link.key.clone(), link);
            }
            ClusterEvent::ConnectionRemoved { key } => {
                self.snapshot.links.remove(&key);
            }
            ClusterEvent::NodeTelemetryReconciled { telemetry } => {
                self.snapshot
                    .telemetry
                    .insert(telemetry.node_id.clone(), telemetry);
            }
            ClusterEvent::ArtifactResidencyReconciled { residency } => {
                self.snapshot
                    .artifact_residency
                    .insert(residency.key.clone(), residency);
            }
            ClusterEvent::ArtifactResidencyRemoved { key } => {
                self.snapshot.artifact_residency.remove(&key);
            }
            ClusterEvent::LeadershipReconciled { leadership } => {
                self.snapshot.leadership = Some(leadership);
            }
        }

        self.snapshot.last_applied_event_index = Some(indexed_event.index);
        Ok(())
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
}

/// Ordered-log and state-apply failures for the first authoritative seam.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClusterHistoryError {
    /// Event or request belongs to a different cluster than the owning log/state.
    #[error("cluster event belongs to a different cluster: expected {expected:?}, got {actual:?}")]
    ClusterIdMismatch {
        /// Expected cluster identity.
        expected: ClusterId,
        /// Observed cluster identity.
        actual: ClusterId,
    },
    /// Event index does not continue contiguously from the current apply point.
    #[error("cluster event index is out of order: expected {expected:?}, got {actual:?}")]
    OutOfOrderEvent {
        /// Next expected authoritative event index.
        expected: ClusterEventIndex,
        /// Actual authoritative event index presented.
        actual: ClusterEventIndex,
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

fn next_expected_index(last_applied_event_index: Option<ClusterEventIndex>) -> ClusterEventIndex {
    last_applied_event_index.map_or(ClusterEventIndex::initial(), ClusterEventIndex::next)
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
    use std::net::SocketAddr;

    use crate::{AdmissionToken, ClusterNamespace, NodeEpoch, NodeRole};

    use super::{
        ClusterArtifactReference, ClusterArtifactResidencyRecord, ClusterArtifactResidencyStatus,
        ClusterArtifactTransferMethod, ClusterBackendReadinessStatus, ClusterCatchupPayload,
        ClusterCatchupRequest, ClusterEvent, ClusterEventIndex, ClusterEventLog,
        ClusterHistoryError, ClusterLeadershipRecord, ClusterLink, ClusterLinkClass,
        ClusterLinkStatus, ClusterMembershipRecord, ClusterMembershipStatus, ClusterNodeTelemetry,
        ClusterRecoveryDisposition, ClusterRecoveryPolicy, ClusterRecoveryReason,
        ClusterSchemaVersion, ClusterStabilityPosture, ClusterState, ClusterTerm,
        ClusterTransportClass, IndexedClusterEvent,
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
}
