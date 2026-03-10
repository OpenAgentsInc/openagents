//! Trusted-LAN cluster control-plane substrate for Psionic.

mod ordered_state;
mod replicated_serving;
mod scheduler;
mod serving_policy;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use rand::random;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    net::UdpSocket,
    sync::{Mutex, oneshot},
    task::JoinHandle,
    time::{MissedTickBehavior, interval},
};

pub use ordered_state::*;
pub use replicated_serving::*;
pub use scheduler::*;
pub use serving_policy::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "trusted-lan cluster control-plane substrate";

const HELLO_INTERVAL: Duration = Duration::from_millis(100);
const PING_INTERVAL: Duration = Duration::from_millis(75);
const MAX_DATAGRAM_BYTES: usize = 8 * 1024;

/// Errors returned by the local cluster transport.
#[derive(Debug, Error)]
pub enum ClusterError {
    /// The local cluster socket could not be bound.
    #[error("failed to bind local cluster socket: {0}")]
    Bind(#[source] std::io::Error),
    /// The local cluster socket address could not be read.
    #[error("failed to read local cluster socket address: {0}")]
    LocalAddr(#[source] std::io::Error),
    /// The background cluster task failed while running.
    #[error("cluster task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
    /// The background cluster task exited with a runtime failure.
    #[error("cluster runtime error: {0}")]
    Runtime(String),
    /// The configured identity policy could not be loaded or stored.
    #[error("failed to read or write local cluster identity: {0}")]
    IdentityIo(#[source] std::io::Error),
    /// The configured identity file contained invalid data.
    #[error("failed to parse local cluster identity: {0}")]
    IdentityFormat(#[source] serde_json::Error),
}

/// Namespace that scopes one trusted local cluster.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterNamespace(String);

impl ClusterNamespace {
    /// Creates a cluster namespace from the supplied string.
    #[must_use]
    pub fn new(namespace: impl Into<String>) -> Self {
        Self(namespace.into())
    }

    /// Returns the namespace as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Shared admission token for the first trusted-LAN seam.
#[derive(Clone, PartialEq, Eq)]
pub struct AdmissionToken(String);

impl AdmissionToken {
    /// Creates an admission token from the supplied string.
    #[must_use]
    pub fn new(admission_token: impl Into<String>) -> Self {
        Self(admission_token.into())
    }

    /// Returns the token as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for AdmissionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AdmissionToken(<redacted>)")
    }
}

/// Explicit namespace and admission configuration for the local cluster seam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterAdmissionConfig {
    /// Namespace that scopes the cluster.
    pub namespace: ClusterNamespace,
    /// Shared admission token for the first trusted-LAN seam.
    pub admission_token: AdmissionToken,
}

impl ClusterAdmissionConfig {
    /// Creates explicit namespace and admission configuration.
    #[must_use]
    pub fn new(namespace: impl Into<String>, admission_token: impl Into<String>) -> Self {
        Self {
            namespace: ClusterNamespace::new(namespace),
            admission_token: AdmissionToken::new(admission_token),
        }
    }
}

/// Node role visible to the local cluster transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeRole {
    /// Participates only in cluster control.
    CoordinatorOnly,
    /// Participates only in execution.
    ExecutorOnly,
    /// Participates in both control and execution.
    Mixed,
}

/// Stable cluster identity for one trusted local namespace/admission pair.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterId(String);

impl ClusterId {
    #[must_use]
    pub fn new(namespace: &ClusterNamespace, admission_token: &AdmissionToken) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(namespace.as_str().as_bytes());
        hasher.update([0]);
        hasher.update(admission_token.as_str().as_bytes());
        Self(hex::encode(hasher.finalize()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable node identity for one local cluster participant.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(String);

impl NodeId {
    #[must_use]
    pub fn new(node_id: impl Into<String>) -> Self {
        Self(node_id.into())
    }

    #[must_use]
    pub fn random() -> Self {
        let raw: [u8; 16] = random();
        Self(hex::encode(raw))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Monotonic node incarnation counter used to distinguish restarts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeEpoch(u64);

impl NodeEpoch {
    /// Initial node epoch for a never-before-seen node identity.
    #[must_use]
    pub const fn initial() -> Self {
        Self(1)
    }

    /// Next node epoch after one already-persisted incarnation.
    #[must_use]
    pub const fn next(previous: Self) -> Self {
        Self(previous.0.saturating_add(1))
    }

    /// Returns the raw epoch value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Identity persistence policy for the first local cluster seam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeIdentityPersistence {
    /// Generate a fresh node identity on every start.
    Ephemeral,
    /// Persist node identity and epoch state in one local JSON file.
    FileBacked { path: PathBuf },
}

/// Cluster/node identity facts surfaced by the transport.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterNodeIdentity {
    /// Shared cluster identity.
    pub cluster_id: ClusterId,
    /// Stable local node identity.
    pub node_id: NodeId,
    /// Monotonic node epoch for this running instance.
    pub node_epoch: NodeEpoch,
    /// Declared node role.
    pub role: NodeRole,
}

/// Minimal local-cluster transport configuration for the first trusted-LAN seam.
#[derive(Clone, Debug)]
pub struct LocalClusterConfig {
    /// Explicit namespace and admission configuration.
    pub admission: ClusterAdmissionConfig,
    /// Local socket address to bind.
    pub bind_addr: SocketAddr,
    /// Explicit seed peers for the first discovery mode.
    pub seed_peers: Vec<SocketAddr>,
    /// Declared role for this node.
    pub role: NodeRole,
    /// Identity persistence policy for this node.
    pub identity_persistence: NodeIdentityPersistence,
}

impl LocalClusterConfig {
    /// Creates a minimal local-cluster configuration.
    #[must_use]
    pub fn new(
        namespace: impl Into<String>,
        admission_token: impl Into<String>,
        bind_addr: SocketAddr,
        role: NodeRole,
    ) -> Self {
        Self {
            admission: ClusterAdmissionConfig::new(namespace, admission_token),
            bind_addr,
            seed_peers: Vec::new(),
            role,
            identity_persistence: NodeIdentityPersistence::Ephemeral,
        }
    }

    /// Attaches explicit seed peers for the first discovery mode.
    #[must_use]
    pub fn with_seed_peers(mut self, seed_peers: Vec<SocketAddr>) -> Self {
        self.seed_peers = seed_peers;
        self
    }

    /// Attaches a file-backed identity policy for stable restart identity.
    #[must_use]
    pub fn with_file_backed_identity(mut self, path: PathBuf) -> Self {
        self.identity_persistence = NodeIdentityPersistence::FileBacked { path };
        self
    }
}

/// Machine-checkable cluster-join refusal reason for the first local seam.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterJoinRefusalReason {
    /// The remote namespace did not match this node's namespace.
    NamespaceMismatch {
        /// Expected namespace.
        expected: ClusterNamespace,
        /// Observed namespace.
        actual: ClusterNamespace,
    },
    /// The remote admission token digest did not match.
    AdmissionMismatch,
    /// The remote cluster identity did not match the local cluster.
    ClusterIdMismatch {
        /// Expected local cluster identity.
        expected: ClusterId,
        /// Observed remote cluster identity.
        actual: ClusterId,
    },
    /// The remote node reused an older node epoch than the current known epoch.
    StaleNodeEpoch {
        /// Highest epoch already observed for this node ID.
        current: NodeEpoch,
        /// Epoch attempted by the stale peer.
        attempted: NodeEpoch,
    },
}

/// One refused cluster-join observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterJoinRefusal {
    /// Remote socket address that attempted the join.
    pub remote_addr: SocketAddr,
    /// Remote node identity, when one could be parsed.
    pub remote_node_id: Option<NodeId>,
    /// Remote cluster identity, when one could be parsed.
    pub remote_cluster_id: Option<ClusterId>,
    /// Remote node epoch, when one could be parsed.
    pub remote_node_epoch: Option<NodeEpoch>,
    /// Machine-checkable refusal reason.
    pub reason: ClusterJoinRefusalReason,
}

/// Peer handshake observations surfaced by the first transport path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerHandshakeState {
    /// Whether a typed hello was observed from this peer.
    pub saw_hello: bool,
    /// Highest ping sequence observed from this peer.
    pub last_ping_sequence: Option<u64>,
}

/// Snapshot of one discovered peer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSnapshot {
    /// Remote socket address used for the current local-cluster transport.
    pub remote_addr: SocketAddr,
    /// Cluster/node identity surfaced by the peer.
    pub identity: ClusterNodeIdentity,
    /// Hello/ping handshake facts observed so far.
    pub handshake: PeerHandshakeState,
}

/// Running local-cluster node for the first hello/ping seam.
pub struct LocalClusterNode {
    local_addr: SocketAddr,
    local_identity: ClusterNodeIdentity,
    state: Arc<Mutex<SharedState>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), String>>>,
}

impl LocalClusterNode {
    /// Starts the first local-cluster hello/ping transport.
    pub async fn spawn(config: LocalClusterConfig) -> Result<Self, ClusterError> {
        let local_identity = load_or_create_local_identity(&config)?;
        let transport_config = TransportConfig::from_config(config, local_identity.clone());
        let socket = Arc::new(
            UdpSocket::bind(transport_config.bind_addr)
                .await
                .map_err(ClusterError::Bind)?,
        );
        let local_addr = socket.local_addr().map_err(ClusterError::LocalAddr)?;
        let state = Arc::new(Mutex::new(SharedState::new(
            transport_config.seed_peers.clone(),
        )));
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(run_transport(
            socket,
            state.clone(),
            transport_config,
            shutdown_rx,
        ));
        Ok(Self {
            local_addr,
            local_identity,
            state,
            shutdown_tx: Some(shutdown_tx),
            task: Some(task),
        })
    }

    /// Returns the bound local transport address.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Returns the local cluster/node identity.
    #[must_use]
    pub fn local_identity(&self) -> &ClusterNodeIdentity {
        &self.local_identity
    }

    /// Returns the currently discovered peers.
    pub async fn peer_snapshots(&self) -> Vec<PeerSnapshot> {
        self.state.lock().await.peer_snapshots()
    }

    /// Returns machine-checkable join refusals observed by this node.
    pub async fn join_refusals(&self) -> Vec<ClusterJoinRefusal> {
        self.state.lock().await.join_refusals()
    }

    /// Shuts the local-cluster node down and waits for the background task.
    pub async fn shutdown(mut self) -> Result<(), ClusterError> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(task) = self.task.take() {
            let outcome = task.await?;
            return outcome.map_err(ClusterError::Runtime);
        }
        Ok(())
    }
}

impl Drop for LocalClusterNode {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[derive(Clone)]
struct TransportConfig {
    namespace: ClusterNamespace,
    admission_digest: String,
    bind_addr: SocketAddr,
    seed_peers: BTreeSet<SocketAddr>,
    local_identity: ClusterNodeIdentity,
}

impl TransportConfig {
    fn from_config(config: LocalClusterConfig, local_identity: ClusterNodeIdentity) -> Self {
        Self {
            namespace: config.admission.namespace,
            admission_digest: admission_digest(&config.admission.admission_token),
            bind_addr: config.bind_addr,
            seed_peers: config.seed_peers.into_iter().collect(),
            local_identity,
        }
    }
}

#[derive(Default)]
struct SharedState {
    peers: BTreeMap<NodeId, PeerSnapshot>,
    join_refusals: Vec<ClusterJoinRefusal>,
    seed_peers: BTreeSet<SocketAddr>,
    next_ping_sequence: u64,
}

impl SharedState {
    fn new(seed_peers: BTreeSet<SocketAddr>) -> Self {
        Self {
            peers: BTreeMap::new(),
            join_refusals: Vec::new(),
            seed_peers,
            next_ping_sequence: 0,
        }
    }

    fn peer_snapshots(&self) -> Vec<PeerSnapshot> {
        self.peers.values().cloned().collect()
    }

    fn join_refusals(&self) -> Vec<ClusterJoinRefusal> {
        self.join_refusals.clone()
    }

    fn push_join_refusal(&mut self, refusal: ClusterJoinRefusal) {
        self.join_refusals.push(refusal);
    }

    fn next_ping_sequence(&mut self) -> u64 {
        let sequence = self.next_ping_sequence;
        self.next_ping_sequence = self.next_ping_sequence.saturating_add(1);
        sequence
    }

    fn undiscovered_seed_peers(&self) -> Vec<SocketAddr> {
        self.seed_peers
            .iter()
            .copied()
            .filter(|addr| self.peers.values().all(|peer| peer.remote_addr != *addr))
            .collect()
    }

    fn discovered_peer_addrs(&self) -> Vec<SocketAddr> {
        self.peers.values().map(|peer| peer.remote_addr).collect()
    }

    fn record_hello(
        &mut self,
        remote_addr: SocketAddr,
        identity: ClusterNodeIdentity,
    ) -> Result<bool, Box<ClusterJoinRefusal>> {
        let outcome = self.validate_peer_epoch(remote_addr, &identity)?;
        let snapshot = self.ensure_peer_snapshot(remote_addr, identity);
        snapshot.handshake.saw_hello = true;
        Ok(outcome.should_reply_hello)
    }

    fn record_ping(
        &mut self,
        remote_addr: SocketAddr,
        identity: ClusterNodeIdentity,
        sequence: u64,
    ) -> Result<(), Box<ClusterJoinRefusal>> {
        let _ = self.validate_peer_epoch(remote_addr, &identity)?;
        let snapshot = self.ensure_peer_snapshot(remote_addr, identity);
        snapshot.handshake.last_ping_sequence = Some(sequence);
        Ok(())
    }

    fn ensure_peer_snapshot(
        &mut self,
        remote_addr: SocketAddr,
        identity: ClusterNodeIdentity,
    ) -> &mut PeerSnapshot {
        let entry = self
            .peers
            .entry(identity.node_id.clone())
            .or_insert_with(|| PeerSnapshot {
                remote_addr,
                identity: identity.clone(),
                handshake: PeerHandshakeState {
                    saw_hello: false,
                    last_ping_sequence: None,
                },
            });
        if identity.node_epoch > entry.identity.node_epoch {
            entry.handshake = PeerHandshakeState {
                saw_hello: false,
                last_ping_sequence: None,
            };
        }
        entry.remote_addr = remote_addr;
        entry.identity = identity;
        entry
    }

    fn validate_peer_epoch(
        &self,
        remote_addr: SocketAddr,
        identity: &ClusterNodeIdentity,
    ) -> Result<PeerEpochOutcome, Box<ClusterJoinRefusal>> {
        if let Some(existing) = self.peers.get(&identity.node_id) {
            if identity.node_epoch < existing.identity.node_epoch {
                return Err(Box::new(ClusterJoinRefusal {
                    remote_addr,
                    remote_node_id: Some(identity.node_id.clone()),
                    remote_cluster_id: Some(identity.cluster_id.clone()),
                    remote_node_epoch: Some(identity.node_epoch),
                    reason: ClusterJoinRefusalReason::StaleNodeEpoch {
                        current: existing.identity.node_epoch,
                        attempted: identity.node_epoch,
                    },
                }));
            }
            let should_reply_hello =
                identity.node_epoch > existing.identity.node_epoch || !existing.handshake.saw_hello;
            return Ok(PeerEpochOutcome { should_reply_hello });
        }
        Ok(PeerEpochOutcome {
            should_reply_hello: true,
        })
    }
}

struct PeerEpochOutcome {
    should_reply_hello: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HelloMessage {
    sender: ClusterNodeIdentity,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PingMessage {
    sender: ClusterNodeIdentity,
    sequence: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WireMessage {
    Hello(HelloMessage),
    Ping(PingMessage),
}

impl WireMessage {
    fn sender(&self) -> &ClusterNodeIdentity {
        match self {
            Self::Hello(message) => &message.sender,
            Self::Ping(message) => &message.sender,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WireEnvelope {
    namespace: ClusterNamespace,
    admission_digest: String,
    message: WireMessage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedNodeIdentityRecord {
    cluster_id: ClusterId,
    node_id: NodeId,
    last_epoch: NodeEpoch,
}

fn load_or_create_local_identity(
    config: &LocalClusterConfig,
) -> Result<ClusterNodeIdentity, ClusterError> {
    let cluster_id = ClusterId::new(
        &config.admission.namespace,
        &config.admission.admission_token,
    );
    match &config.identity_persistence {
        NodeIdentityPersistence::Ephemeral => Ok(ClusterNodeIdentity {
            cluster_id,
            node_id: NodeId::random(),
            node_epoch: NodeEpoch::initial(),
            role: config.role,
        }),
        NodeIdentityPersistence::FileBacked { path } => {
            let mut node_id = NodeId::random();
            let mut node_epoch = NodeEpoch::initial();
            if path.exists() {
                let bytes = fs::read(path).map_err(ClusterError::IdentityIo)?;
                let record: PersistedNodeIdentityRecord =
                    serde_json::from_slice(&bytes).map_err(ClusterError::IdentityFormat)?;
                if record.cluster_id == cluster_id {
                    node_id = record.node_id;
                    node_epoch = NodeEpoch::next(record.last_epoch);
                }
            } else if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(ClusterError::IdentityIo)?;
            }
            let record = PersistedNodeIdentityRecord {
                cluster_id: cluster_id.clone(),
                node_id: node_id.clone(),
                last_epoch: node_epoch,
            };
            let encoded =
                serde_json::to_vec_pretty(&record).map_err(ClusterError::IdentityFormat)?;
            fs::write(path, encoded).map_err(ClusterError::IdentityIo)?;
            Ok(ClusterNodeIdentity {
                cluster_id,
                node_id,
                node_epoch,
                role: config.role,
            })
        }
    }
}

async fn run_transport(
    socket: Arc<UdpSocket>,
    state: Arc<Mutex<SharedState>>,
    config: TransportConfig,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    send_hello_to_seed_peers(&socket, &config, &state).await?;

    let mut hello_tick = interval(HELLO_INTERVAL);
    hello_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut ping_tick = interval(PING_INTERVAL);
    ping_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut recv_buf = vec![0_u8; MAX_DATAGRAM_BYTES];

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => return Ok(()),
            _ = hello_tick.tick() => {
                send_hello_to_seed_peers(&socket, &config, &state).await?;
            }
            _ = ping_tick.tick() => {
                send_ping_to_discovered_peers(&socket, &config, &state).await?;
            }
            received = socket.recv_from(&mut recv_buf) => {
                let (len, remote_addr) = received.map_err(|error| error.to_string())?;
                handle_incoming_message(
                    &socket,
                    &state,
                    &config,
                    remote_addr,
                    &recv_buf[..len],
                )
                .await?;
            }
        }
    }
}

async fn send_hello_to_seed_peers(
    socket: &Arc<UdpSocket>,
    config: &TransportConfig,
    state: &Arc<Mutex<SharedState>>,
) -> Result<(), String> {
    let seed_peers = state.lock().await.undiscovered_seed_peers();
    for remote_addr in seed_peers {
        send_message(
            socket,
            remote_addr,
            &WireEnvelope {
                namespace: config.namespace.clone(),
                admission_digest: config.admission_digest.clone(),
                message: WireMessage::Hello(HelloMessage {
                    sender: config.local_identity.clone(),
                }),
            },
        )
        .await?;
    }
    Ok(())
}

async fn send_ping_to_discovered_peers(
    socket: &Arc<UdpSocket>,
    config: &TransportConfig,
    state: &Arc<Mutex<SharedState>>,
) -> Result<(), String> {
    let (peer_addrs, sequence) = {
        let mut guard = state.lock().await;
        (guard.discovered_peer_addrs(), guard.next_ping_sequence())
    };
    for remote_addr in peer_addrs {
        send_message(
            socket,
            remote_addr,
            &WireEnvelope {
                namespace: config.namespace.clone(),
                admission_digest: config.admission_digest.clone(),
                message: WireMessage::Ping(PingMessage {
                    sender: config.local_identity.clone(),
                    sequence,
                }),
            },
        )
        .await?;
    }
    Ok(())
}

async fn handle_incoming_message(
    socket: &Arc<UdpSocket>,
    state: &Arc<Mutex<SharedState>>,
    config: &TransportConfig,
    remote_addr: SocketAddr,
    payload: &[u8],
) -> Result<(), String> {
    let envelope = match serde_json::from_slice::<WireEnvelope>(payload) {
        Ok(envelope) => envelope,
        Err(_) => return Ok(()),
    };

    if envelope.namespace != config.namespace {
        state.lock().await.push_join_refusal(ClusterJoinRefusal {
            remote_addr,
            remote_node_id: Some(envelope.message.sender().node_id.clone()),
            remote_cluster_id: Some(envelope.message.sender().cluster_id.clone()),
            remote_node_epoch: Some(envelope.message.sender().node_epoch),
            reason: ClusterJoinRefusalReason::NamespaceMismatch {
                expected: config.namespace.clone(),
                actual: envelope.namespace.clone(),
            },
        });
        return Ok(());
    }
    if envelope.admission_digest != config.admission_digest {
        state.lock().await.push_join_refusal(ClusterJoinRefusal {
            remote_addr,
            remote_node_id: Some(envelope.message.sender().node_id.clone()),
            remote_cluster_id: Some(envelope.message.sender().cluster_id.clone()),
            remote_node_epoch: Some(envelope.message.sender().node_epoch),
            reason: ClusterJoinRefusalReason::AdmissionMismatch,
        });
        return Ok(());
    }

    match envelope.message {
        WireMessage::Hello(hello) => {
            if hello.sender.node_id == config.local_identity.node_id {
                return Ok(());
            }
            if hello.sender.cluster_id != config.local_identity.cluster_id {
                state.lock().await.push_join_refusal(ClusterJoinRefusal {
                    remote_addr,
                    remote_node_id: Some(hello.sender.node_id),
                    remote_cluster_id: Some(hello.sender.cluster_id.clone()),
                    remote_node_epoch: Some(hello.sender.node_epoch),
                    reason: ClusterJoinRefusalReason::ClusterIdMismatch {
                        expected: config.local_identity.cluster_id.clone(),
                        actual: hello.sender.cluster_id,
                    },
                });
                return Ok(());
            }

            let should_reply_hello = {
                let mut guard = state.lock().await;
                match guard.record_hello(remote_addr, hello.sender) {
                    Ok(should_reply) => should_reply,
                    Err(refusal) => {
                        guard.push_join_refusal(*refusal);
                        return Ok(());
                    }
                }
            };

            if should_reply_hello {
                send_message(
                    socket,
                    remote_addr,
                    &WireEnvelope {
                        namespace: config.namespace.clone(),
                        admission_digest: config.admission_digest.clone(),
                        message: WireMessage::Hello(HelloMessage {
                            sender: config.local_identity.clone(),
                        }),
                    },
                )
                .await?;
            }

            let sequence = state.lock().await.next_ping_sequence();
            send_message(
                socket,
                remote_addr,
                &WireEnvelope {
                    namespace: config.namespace.clone(),
                    admission_digest: config.admission_digest.clone(),
                    message: WireMessage::Ping(PingMessage {
                        sender: config.local_identity.clone(),
                        sequence,
                    }),
                },
            )
            .await?;
        }
        WireMessage::Ping(ping) => {
            if ping.sender.node_id == config.local_identity.node_id {
                return Ok(());
            }
            if ping.sender.cluster_id != config.local_identity.cluster_id {
                state.lock().await.push_join_refusal(ClusterJoinRefusal {
                    remote_addr,
                    remote_node_id: Some(ping.sender.node_id),
                    remote_cluster_id: Some(ping.sender.cluster_id.clone()),
                    remote_node_epoch: Some(ping.sender.node_epoch),
                    reason: ClusterJoinRefusalReason::ClusterIdMismatch {
                        expected: config.local_identity.cluster_id.clone(),
                        actual: ping.sender.cluster_id,
                    },
                });
                return Ok(());
            }

            let mut guard = state.lock().await;
            if let Err(refusal) = guard.record_ping(remote_addr, ping.sender, ping.sequence) {
                guard.push_join_refusal(*refusal);
            }
        }
    }
    Ok(())
}

async fn send_message(
    socket: &Arc<UdpSocket>,
    remote_addr: SocketAddr,
    envelope: &WireEnvelope,
) -> Result<(), String> {
    let encoded = serde_json::to_vec(envelope).map_err(|error| error.to_string())?;
    socket
        .send_to(&encoded, remote_addr)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn admission_digest(admission_token: &AdmissionToken) -> String {
    let mut hasher = Sha256::new();
    hasher.update(admission_token.as_str().as_bytes());
    hex::encode(hasher.finalize())
}
