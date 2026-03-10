//! Trusted-LAN cluster control-plane substrate for Psionic.

use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
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
    pub fn new(namespace: &str, admission_token: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(namespace.as_bytes());
        hasher.update([0]);
        hasher.update(admission_token.as_bytes());
        Self(hex::encode(hasher.finalize()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Generated node identity for the first local cluster seam.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(String);

impl NodeId {
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

/// Cluster/node identity facts surfaced by the transport.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterNodeIdentity {
    /// Shared cluster identity.
    pub cluster_id: ClusterId,
    /// Generated local node identity.
    pub node_id: NodeId,
    /// Declared node role.
    pub role: NodeRole,
}

/// Minimal local-cluster transport configuration for the first trusted-LAN seam.
#[derive(Clone, Debug)]
pub struct LocalClusterConfig {
    /// Namespace that scopes the local cluster.
    pub namespace: String,
    /// Shared admission token for the first trusted-LAN seam.
    pub admission_token: String,
    /// Local socket address to bind.
    pub bind_addr: SocketAddr,
    /// Explicit seed peers for the first discovery mode.
    pub seed_peers: Vec<SocketAddr>,
    /// Declared role for this node.
    pub role: NodeRole,
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
            namespace: namespace.into(),
            admission_token: admission_token.into(),
            bind_addr,
            seed_peers: Vec::new(),
            role,
        }
    }

    /// Attaches explicit seed peers for the first discovery mode.
    #[must_use]
    pub fn with_seed_peers(mut self, seed_peers: Vec<SocketAddr>) -> Self {
        self.seed_peers = seed_peers;
        self
    }
}

/// Peer handshake observations surfaced by the first transport path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerHandshakeState {
    /// Whether a typed hello was observed from this peer.
    pub saw_hello: bool,
    /// Highest ping sequence observed from this peer.
    pub last_ping_sequence: Option<u64>,
}

/// Snapshot of one discovered peer.
#[derive(Clone, Debug, PartialEq, Eq)]
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
        let cluster_id = ClusterId::new(&config.namespace, &config.admission_token);
        let local_identity = ClusterNodeIdentity {
            cluster_id,
            node_id: NodeId::random(),
            role: config.role,
        };
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
    namespace: String,
    admission_digest: String,
    bind_addr: SocketAddr,
    seed_peers: BTreeSet<SocketAddr>,
    local_identity: ClusterNodeIdentity,
}

impl TransportConfig {
    fn from_config(config: LocalClusterConfig, local_identity: ClusterNodeIdentity) -> Self {
        Self {
            namespace: config.namespace,
            admission_digest: admission_digest(&config.admission_token),
            bind_addr: config.bind_addr,
            seed_peers: config.seed_peers.into_iter().collect(),
            local_identity,
        }
    }
}

#[derive(Default)]
struct SharedState {
    peers: BTreeMap<NodeId, PeerSnapshot>,
    seed_peers: BTreeSet<SocketAddr>,
    next_ping_sequence: u64,
}

impl SharedState {
    fn new(seed_peers: BTreeSet<SocketAddr>) -> Self {
        Self {
            peers: BTreeMap::new(),
            seed_peers,
            next_ping_sequence: 0,
        }
    }

    fn peer_snapshots(&self) -> Vec<PeerSnapshot> {
        self.peers.values().cloned().collect()
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

    fn record_hello(&mut self, remote_addr: SocketAddr, identity: ClusterNodeIdentity) -> bool {
        let snapshot = self
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
        let first_hello = !snapshot.handshake.saw_hello;
        snapshot.remote_addr = remote_addr;
        snapshot.identity = identity;
        snapshot.handshake.saw_hello = true;
        first_hello
    }

    fn record_ping(
        &mut self,
        remote_addr: SocketAddr,
        identity: ClusterNodeIdentity,
        sequence: u64,
    ) {
        let snapshot = self
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
        snapshot.remote_addr = remote_addr;
        snapshot.identity = identity;
        snapshot.handshake.last_ping_sequence = Some(sequence);
    }
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WireEnvelope {
    namespace: String,
    admission_digest: String,
    message: WireMessage,
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
    if envelope.namespace != config.namespace
        || envelope.admission_digest != config.admission_digest
    {
        return Ok(());
    }

    match envelope.message {
        WireMessage::Hello(hello) => {
            if hello.sender.node_id == config.local_identity.node_id
                || hello.sender.cluster_id != config.local_identity.cluster_id
            {
                return Ok(());
            }
            let should_reply_hello = state.lock().await.record_hello(remote_addr, hello.sender);
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
            if ping.sender.node_id == config.local_identity.node_id
                || ping.sender.cluster_id != config.local_identity.cluster_id
            {
                return Ok(());
            }
            state
                .lock()
                .await
                .record_ping(remote_addr, ping.sender, ping.sequence);
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

fn admission_digest(admission_token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(admission_token.as_bytes());
    hex::encode(hasher.finalize())
}
