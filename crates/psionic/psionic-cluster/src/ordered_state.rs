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
    /// Unknown or not-yet-classified transport.
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
        }
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
    /// Request that the leader advance authoritative leadership truth.
    UpdateLeadership {
        leader_id: NodeId,
        term: ClusterTerm,
    },
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
    /// Highest authoritative event applied into this snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_applied_event_index: Option<ClusterEventIndex>,
    /// Current cluster membership by node ID.
    pub memberships: BTreeMap<NodeId, ClusterMembershipRecord>,
    /// Current cluster links by canonical node pair.
    pub links: BTreeMap<ClusterLinkKey, ClusterLink>,
    /// Current leader/coordinator truth, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leadership: Option<ClusterLeadershipRecord>,
}

impl ClusterSnapshot {
    /// Creates an empty cluster snapshot for one cluster identity.
    #[must_use]
    pub fn new(cluster_id: ClusterId) -> Self {
        Self {
            cluster_id,
            last_applied_event_index: None,
            memberships: BTreeMap::new(),
            links: BTreeMap::new(),
            leadership: None,
        }
    }

    /// Returns a stable digest of the authoritative cluster snapshot.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(
            self.last_applied_event_index
                .map_or(0, ClusterEventIndex::as_u64)
                .to_string()
                .as_bytes(),
        );
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
        Self {
            snapshot: ClusterSnapshot::new(cluster_id),
        }
    }

    /// Rehydrates authoritative cluster state from one prior snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: ClusterSnapshot) -> Self {
        Self { snapshot }
    }

    /// Returns the cluster identity owned by this state.
    #[must_use]
    pub fn cluster_id(&self) -> &ClusterId {
        &self.snapshot.cluster_id
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

    /// Returns a stable digest of the current authoritative state.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        self.snapshot.stable_digest()
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
    events: Vec<IndexedClusterEvent>,
}

impl ClusterEventLog {
    /// Creates an empty authoritative event log for one cluster identity.
    #[must_use]
    pub fn new(cluster_id: ClusterId) -> Self {
        Self {
            cluster_id,
            events: Vec::new(),
        }
    }

    /// Returns the ordered authoritative event history.
    #[must_use]
    pub fn events(&self) -> &[IndexedClusterEvent] {
        &self.events
    }

    /// Returns the highest authoritative event index in the log, when one exists.
    #[must_use]
    pub fn last_index(&self) -> Option<ClusterEventIndex> {
        self.events.last().map(|event| event.index)
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

    /// Rebuilds authoritative cluster state from the ordered event history.
    pub fn replay(&self) -> Result<ClusterState, ClusterHistoryError> {
        let mut state = ClusterState::new(self.cluster_id.clone());
        for event in &self.events {
            state.apply(event.clone())?;
        }
        Ok(state)
    }
}

/// Ordered-log and state-apply failures for the first authoritative seam.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClusterHistoryError {
    /// Event belongs to a different cluster than the owning log/state.
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

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use crate::{AdmissionToken, ClusterNamespace, NodeEpoch, NodeRole};

    use super::{
        ClusterEvent, ClusterEventIndex, ClusterEventLog, ClusterHistoryError,
        ClusterLeadershipRecord, ClusterLink, ClusterLinkStatus, ClusterMembershipRecord,
        ClusterMembershipStatus, ClusterState, ClusterTerm, ClusterTransportClass,
        IndexedClusterEvent,
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
}
