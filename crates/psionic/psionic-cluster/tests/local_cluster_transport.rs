use std::{
    net::{SocketAddr, UdpSocket as StdUdpSocket},
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use ed25519_dalek::SigningKey;
use psionic_cluster::{
    ClusterDiscoveryPosture, ClusterJoinRefusalReason, ClusterLogicalStreamKind,
    ClusterNonLanDiscoveryDisposition, ClusterNonLanDiscoveryRefusalReason,
    ClusterOperatorManifest, ClusterRelayEndpoint, ClusterRelayServer, ClusterStreamError,
    ClusterTransportPathKind, ClusterTrustPolicy, ClusterTrustPosture,
    ClusterTrustRolloutDisposition, ClusterTunnelError, ClusterTunnelHttpRequest,
    ClusterTunnelPolicy, ClusterTunnelServiceKind, ClusterTunnelServicePolicy, ClusterTunnelState,
    ConfiguredClusterPeer, ConfiguredPeerDialPolicy, ConfiguredPeerKeyMatch,
    ConfiguredPeerReachability, LocalClusterConfig, LocalClusterNode, NodeRole,
};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::{Instant, sleep, timeout};

const CLUSTER_NAMESPACE: &str = "lan-alpha";
const CLUSTER_ADMISSION_TOKEN: &str = "shared-secret";

fn loopback_addr(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn reserve_loopback_addr() -> SocketAddr {
    let socket = StdUdpSocket::bind(loopback_addr(0));
    assert!(socket.is_ok(), "port reservation socket should bind");
    let socket = socket
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let addr = socket.local_addr();
    assert!(
        addr.is_ok(),
        "port reservation socket should expose an address"
    );
    addr.ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"))
}

fn base_config(bind_addr: SocketAddr, role: NodeRole) -> LocalClusterConfig {
    LocalClusterConfig::new(CLUSTER_NAMESPACE, CLUSTER_ADMISSION_TOKEN, bind_addr, role)
}

async fn bootstrap_file_backed_identity(
    path: &Path,
    bind_addr: SocketAddr,
    role: NodeRole,
) -> psionic_cluster::ClusterNodeIdentity {
    let node = LocalClusterNode::spawn(
        base_config(bind_addr, role).with_file_backed_identity(path.to_path_buf()),
    )
    .await;
    assert!(node.is_ok(), "bootstrap node should start");
    let node = node
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let identity = node.local_identity().clone();
    let shutdown = node.shutdown().await;
    assert!(shutdown.is_ok(), "bootstrap node should shut down cleanly");
    identity
}

fn store_operator_manifest(path: &Path, config: &LocalClusterConfig) {
    let manifest = ClusterOperatorManifest::from_local_config(config);
    let stored = manifest.store_json(path);
    assert!(stored.is_ok(), "operator manifest should store");
}

async fn wait_for_single_peer(node: &LocalClusterNode) -> psionic_cluster::PeerSnapshot {
    let wait = timeout(Duration::from_secs(3), async {
        loop {
            let peers = node.peer_snapshots().await;
            if peers.len() == 1 {
                let peer = peers[0].clone();
                if peer.handshake.saw_hello && peer.handshake.last_ping_sequence.is_some() {
                    return peer;
                }
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(wait.is_ok(), "peer discovery timed out");
    wait.ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"))
}

async fn wait_for_peer_epoch(
    node: &LocalClusterNode,
    remote_node_id: &psionic_cluster::NodeId,
    remote_epoch: u64,
) -> psionic_cluster::PeerSnapshot {
    let wait = timeout(Duration::from_secs(3), async {
        loop {
            let peers = node.peer_snapshots().await;
            if let Some(peer) = peers.into_iter().find(|peer| {
                peer.identity.node_id == *remote_node_id
                    && peer.identity.node_epoch.as_u64() == remote_epoch
                    && peer.handshake.saw_hello
                    && peer.handshake.last_ping_sequence.is_some()
            }) {
                return peer;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(wait.is_ok(), "peer epoch update timed out");
    wait.ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"))
}

async fn wait_for_no_peers(node: &LocalClusterNode, duration: Duration) {
    let deadline = Instant::now() + duration;
    loop {
        let peers = node.peer_snapshots().await;
        assert!(peers.is_empty(), "unexpected peers discovered: {peers:?}");
        if Instant::now() >= deadline {
            return;
        }
        sleep(Duration::from_millis(25)).await;
    }
}

async fn wait_for_refusal<F>(node: &LocalClusterNode, predicate: F)
where
    F: Fn(&psionic_cluster::ClusterJoinRefusal) -> bool,
{
    let wait = timeout(Duration::from_secs(3), async {
        loop {
            let refusals = node.join_refusals().await;
            if refusals.iter().any(&predicate) {
                return;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(wait.is_ok(), "expected join refusal was not observed");
}

async fn wait_for_configured_peer_health<F>(
    node: &LocalClusterNode,
    predicate: F,
) -> psionic_cluster::ConfiguredPeerHealthSnapshot
where
    F: Fn(&psionic_cluster::ConfiguredPeerHealthSnapshot) -> bool,
{
    let wait = timeout(Duration::from_secs(3), async {
        loop {
            let snapshots = node.configured_peer_health_snapshots().await;
            if let Some(snapshot) = snapshots.into_iter().find(|snapshot| predicate(snapshot)) {
                return snapshot;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(wait.is_ok(), "configured peer health timed out");
    wait.ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"))
}

async fn wait_for_rollout_diagnostic<F>(
    node: &LocalClusterNode,
    predicate: F,
) -> psionic_cluster::ClusterTrustRolloutDiagnostic
where
    F: Fn(&psionic_cluster::ClusterTrustRolloutDiagnostic) -> bool,
{
    let wait = timeout(Duration::from_secs(3), async {
        loop {
            let diagnostics = node.trust_rollout_diagnostics().await;
            if let Some(diagnostic) = diagnostics
                .into_iter()
                .find(|diagnostic| predicate(diagnostic))
            {
                return diagnostic;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(wait.is_ok(), "trust rollout diagnostic timed out");
    wait.ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"))
}

fn auth_public_key_for_test(byte: u8) -> String {
    let signing_key = SigningKey::from_bytes(&[byte; 32]);
    hex::encode(signing_key.verifying_key().to_bytes())
}

async fn spawn_http_echo_service() -> (
    SocketAddr,
    Arc<Mutex<Vec<String>>>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind(loopback_addr(0)).await;
    assert!(listener.is_ok(), "HTTP echo service should bind");
    let listener = listener
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let local_addr = listener.local_addr();
    assert!(
        local_addr.is_ok(),
        "HTTP echo service should expose local address"
    );
    let local_addr = local_addr
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let observed_requests = Arc::new(Mutex::new(Vec::new()));
    let observed_requests_for_task = Arc::clone(&observed_requests);
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accepted = listener.accept() => {
                    let Ok((mut stream, _peer_addr)) = accepted else {
                        break;
                    };
                    let observed_requests = Arc::clone(&observed_requests_for_task);
                    tokio::spawn(async move {
                        let mut request_bytes = Vec::new();
                        let read = stream.read_to_end(&mut request_bytes).await;
                        if read.is_err() {
                            return;
                        }
                        let request_text = String::from_utf8_lossy(&request_bytes);
                        let Some(header_end) = request_text.find("\r\n\r\n") else {
                            return;
                        };
                        let header_text = &request_text[..header_end];
                        let body_text = &request_text[header_end + 4..];
                        let mut lines = header_text.split("\r\n");
                        let first_line = lines.next().unwrap_or("");
                        if let Ok(mut observed) = observed_requests.lock() {
                            observed.push(first_line.to_string());
                        }
                        let mut parts = first_line.split_whitespace();
                        let method = parts.next().unwrap_or("UNKNOWN");
                        let path = parts.next().unwrap_or("/");
                        let response_body =
                            format!("method={method};path={path};body={body_text}");
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            response_body.len(),
                            response_body
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                        let _ = stream.shutdown().await;
                    });
                }
            }
        }
    });
    (local_addr, observed_requests, shutdown_tx, task)
}

async fn wait_for_tunnel_state(
    node: &LocalClusterNode,
    tunnel_id: psionic_cluster::ClusterTunnelId,
    expected_state: ClusterTunnelState,
) -> psionic_cluster::ClusterTunnelSnapshot {
    let wait = timeout(Duration::from_secs(3), async {
        loop {
            let tunnels = node.tunnel_snapshots().await;
            if let Some(tunnel) = tunnels
                .into_iter()
                .find(|tunnel| tunnel.tunnel_id == tunnel_id && tunnel.state == expected_state)
            {
                return tunnel;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(wait.is_ok(), "tunnel state did not converge");
    wait.ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn seeded_local_nodes_discover_each_other_and_exchange_hello_and_ping() {
    let executor =
        LocalClusterNode::spawn(base_config(loopback_addr(0), NodeRole::ExecutorOnly)).await;
    assert!(executor.is_ok(), "executor node should start");
    let executor = executor
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let coordinator = LocalClusterNode::spawn(
        base_config(loopback_addr(0), NodeRole::CoordinatorOnly)
            .with_seed_peers(vec![executor.local_addr()]),
    )
    .await;
    assert!(coordinator.is_ok(), "coordinator node should start");
    let coordinator = coordinator
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let executor_peer = wait_for_single_peer(&executor).await;
    let coordinator_peer = wait_for_single_peer(&coordinator).await;

    assert_eq!(
        executor.local_identity().cluster_id,
        coordinator.local_identity().cluster_id
    );
    assert_eq!(
        executor_peer.identity.node_id,
        coordinator.local_identity().node_id
    );
    assert_eq!(
        coordinator_peer.identity.node_id,
        executor.local_identity().node_id
    );
    assert_eq!(executor_peer.identity.role, NodeRole::CoordinatorOnly);
    assert_eq!(coordinator_peer.identity.role, NodeRole::ExecutorOnly);
    assert_eq!(executor_peer.identity.node_epoch.as_u64(), 1);
    assert_eq!(coordinator_peer.identity.node_epoch.as_u64(), 1);
    assert_eq!(
        executor.discovery_posture(),
        ClusterDiscoveryPosture::TrustedLanSeedPeers
    );
    assert_eq!(
        coordinator.non_lan_discovery_assessment().disposition,
        ClusterNonLanDiscoveryDisposition::Refused
    );
    assert_eq!(
        coordinator.non_lan_discovery_assessment().refusal_reasons,
        vec![ClusterNonLanDiscoveryRefusalReason::TrustedLanSeedPeersOnly]
    );

    let coordinator_shutdown = coordinator.shutdown().await;
    assert!(
        coordinator_shutdown.is_ok(),
        "coordinator should shut down cleanly"
    );
    let executor_shutdown = executor.shutdown().await;
    assert!(
        executor_shutdown.is_ok(),
        "executor should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_backed_identity_persists_node_id_and_increments_epoch() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let identity_path = temp.path().join("node-identity.json");

    let first = LocalClusterNode::spawn(
        base_config(loopback_addr(0), NodeRole::Mixed)
            .with_file_backed_identity(identity_path.clone()),
    )
    .await;
    assert!(first.is_ok(), "first node should start");
    let first = first
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let first_identity = first.local_identity().clone();
    assert_eq!(first_identity.node_epoch.as_u64(), 1);

    let first_shutdown = first.shutdown().await;
    assert!(
        first_shutdown.is_ok(),
        "first node should shut down cleanly"
    );

    let second = LocalClusterNode::spawn(
        base_config(loopback_addr(0), NodeRole::Mixed).with_file_backed_identity(identity_path),
    )
    .await;
    assert!(second.is_ok(), "second node should start");
    let second = second
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let second_identity = second.local_identity().clone();
    assert_eq!(second_identity.node_id, first_identity.node_id);
    assert_eq!(second_identity.node_epoch.as_u64(), 2);
    assert_eq!(second_identity.role, NodeRole::Mixed);

    let second_shutdown = second.shutdown().await;
    assert!(
        second_shutdown.is_ok(),
        "second node should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mismatched_admission_token_prevents_discovery_and_records_refusal() {
    let receiver = LocalClusterNode::spawn(base_config(loopback_addr(0), NodeRole::Mixed)).await;
    assert!(receiver.is_ok(), "receiver node should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender = LocalClusterNode::spawn(
        LocalClusterConfig::new(
            CLUSTER_NAMESPACE,
            "different-secret",
            loopback_addr(0),
            NodeRole::Mixed,
        )
        .with_seed_peers(vec![receiver.local_addr()]),
    )
    .await;
    assert!(sender.is_ok(), "sender node should start");
    let sender = sender
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    wait_for_no_peers(&receiver, Duration::from_millis(350)).await;
    wait_for_no_peers(&sender, Duration::from_millis(350)).await;
    wait_for_refusal(&receiver, |refusal| {
        refusal.remote_node_id.as_ref() == Some(&sender.local_identity().node_id)
            && matches!(refusal.reason, ClusterJoinRefusalReason::AdmissionMismatch)
    })
    .await;

    let sender_shutdown = sender.shutdown().await;
    assert!(sender_shutdown.is_ok(), "sender should shut down cleanly");
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restarted_node_rejoins_cluster_with_advanced_epoch() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let identity_path = temp.path().join("rejoin-node.json");

    let receiver =
        LocalClusterNode::spawn(base_config(loopback_addr(0), NodeRole::CoordinatorOnly)).await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender_epoch_one = LocalClusterNode::spawn(
        base_config(loopback_addr(0), NodeRole::ExecutorOnly)
            .with_seed_peers(vec![receiver.local_addr()])
            .with_file_backed_identity(identity_path.clone()),
    )
    .await;
    assert!(sender_epoch_one.is_ok(), "first sender should start");
    let sender_epoch_one = sender_epoch_one
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let first_identity = sender_epoch_one.local_identity().clone();
    let first_peer = wait_for_peer_epoch(&receiver, &first_identity.node_id, 1).await;
    assert_eq!(first_peer.identity.node_epoch.as_u64(), 1);

    let sender_epoch_one_shutdown = sender_epoch_one.shutdown().await;
    assert!(
        sender_epoch_one_shutdown.is_ok(),
        "first sender should shut down cleanly"
    );

    let sender_epoch_two = LocalClusterNode::spawn(
        base_config(loopback_addr(0), NodeRole::ExecutorOnly)
            .with_seed_peers(vec![receiver.local_addr()])
            .with_file_backed_identity(identity_path),
    )
    .await;
    assert!(sender_epoch_two.is_ok(), "second sender should start");
    let sender_epoch_two = sender_epoch_two
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let second_identity = sender_epoch_two.local_identity().clone();
    assert_eq!(second_identity.node_id, first_identity.node_id);
    assert_eq!(second_identity.node_epoch.as_u64(), 2);

    let second_peer = wait_for_peer_epoch(&receiver, &second_identity.node_id, 2).await;
    assert_eq!(second_peer.identity.node_id, first_identity.node_id);
    assert_eq!(second_peer.identity.node_epoch.as_u64(), 2);

    let sender_epoch_two_shutdown = sender_epoch_two.shutdown().await;
    assert!(
        sender_epoch_two_shutdown.is_ok(),
        "second sender should shut down cleanly"
    );
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_node_epoch_is_refused_after_newer_epoch_is_observed() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let identity_path = temp.path().join("shared-node.json");

    let receiver =
        LocalClusterNode::spawn(base_config(loopback_addr(0), NodeRole::CoordinatorOnly)).await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender_epoch_one = LocalClusterNode::spawn(
        base_config(loopback_addr(0), NodeRole::ExecutorOnly)
            .with_seed_peers(vec![receiver.local_addr()])
            .with_file_backed_identity(identity_path.clone()),
    )
    .await;
    assert!(sender_epoch_one.is_ok(), "first sender should start");
    let sender_epoch_one = sender_epoch_one
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let first_peer = wait_for_single_peer(&receiver).await;
    assert_eq!(
        first_peer.identity.node_id,
        sender_epoch_one.local_identity().node_id
    );
    assert_eq!(first_peer.identity.node_epoch.as_u64(), 1);

    let sender_epoch_two = LocalClusterNode::spawn(
        base_config(loopback_addr(0), NodeRole::Mixed)
            .with_seed_peers(vec![receiver.local_addr()])
            .with_file_backed_identity(identity_path),
    )
    .await;
    assert!(sender_epoch_two.is_ok(), "second sender should start");
    let sender_epoch_two = sender_epoch_two
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let wait_for_epoch_two = timeout(Duration::from_secs(3), async {
        loop {
            let peers = receiver.peer_snapshots().await;
            if peers.len() == 1 {
                let peer = peers[0].clone();
                if peer.identity.node_id == sender_epoch_two.local_identity().node_id
                    && peer.identity.node_epoch == sender_epoch_two.local_identity().node_epoch
                    && peer.identity.role == NodeRole::Mixed
                {
                    return;
                }
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(
        wait_for_epoch_two.is_ok(),
        "receiver should adopt newer epoch"
    );

    wait_for_refusal(&receiver, |refusal| {
        refusal.remote_node_id.as_ref() == Some(&sender_epoch_one.local_identity().node_id)
            && matches!(
                refusal.reason,
                ClusterJoinRefusalReason::StaleNodeEpoch { current, attempted }
                    if current == sender_epoch_two.local_identity().node_epoch
                        && attempted == sender_epoch_one.local_identity().node_epoch
            )
    })
    .await;

    let sender_epoch_two_shutdown = sender_epoch_two.shutdown().await;
    assert!(
        sender_epoch_two_shutdown.is_ok(),
        "second sender should shut down cleanly"
    );
    let sender_epoch_one_shutdown = sender_epoch_one.shutdown().await;
    assert!(
        sender_epoch_one_shutdown.is_ok(),
        "first sender should shut down cleanly"
    );
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authenticated_configured_peers_discover_each_other_with_signed_control_plane_messages() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let receiver_path = temp.path().join("receiver-auth.json");
    let sender_path = temp.path().join("sender-auth.json");
    let receiver_addr = reserve_loopback_addr();
    let sender_addr = reserve_loopback_addr();

    let receiver_bootstrap =
        bootstrap_file_backed_identity(&receiver_path, receiver_addr, NodeRole::CoordinatorOnly)
            .await;
    let sender_bootstrap =
        bootstrap_file_backed_identity(&sender_path, sender_addr, NodeRole::ExecutorOnly).await;

    let receiver = LocalClusterNode::spawn(
        base_config(receiver_addr, NodeRole::CoordinatorOnly)
            .with_file_backed_identity(receiver_path.clone())
            .with_authenticated_configured_peers(vec![
                ConfiguredClusterPeer::new(
                    sender_bootstrap.node_id.clone(),
                    sender_addr,
                    sender_bootstrap.auth_public_key.clone(),
                )
                .with_max_concurrent_streams(2),
            ]),
    )
    .await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender = LocalClusterNode::spawn(
        base_config(sender_addr, NodeRole::ExecutorOnly)
            .with_file_backed_identity(sender_path)
            .with_authenticated_configured_peers(vec![
                ConfiguredClusterPeer::new(
                    receiver_bootstrap.node_id.clone(),
                    receiver_addr,
                    receiver_bootstrap.auth_public_key.clone(),
                )
                .with_max_concurrent_streams(2),
            ]),
    )
    .await;
    assert!(sender.is_ok(), "sender should start");
    let sender = sender
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    assert_eq!(
        receiver.trust_policy().posture,
        ClusterTrustPosture::AuthenticatedConfiguredPeers
    );
    assert!(receiver.trust_policy().require_message_authentication);
    assert_eq!(
        sender.trust_policy().posture,
        ClusterTrustPosture::AuthenticatedConfiguredPeers
    );
    assert!(sender.trust_policy().require_message_authentication);
    assert_eq!(
        receiver.discovery_posture(),
        ClusterDiscoveryPosture::OperatorManagedConfiguredPeers
    );
    assert_eq!(
        receiver.non_lan_discovery_assessment().refusal_reasons,
        vec![ClusterNonLanDiscoveryRefusalReason::OperatorManagedConfiguredPeersOnly]
    );
    assert_eq!(
        sender.discovery_posture(),
        ClusterDiscoveryPosture::OperatorManagedConfiguredPeers
    );

    let receiver_peer = wait_for_single_peer(&receiver).await;
    let sender_peer = wait_for_single_peer(&sender).await;

    assert_eq!(
        receiver_peer.identity.node_id,
        sender.local_identity().node_id
    );
    assert_eq!(
        receiver_peer.identity.auth_public_key,
        sender.local_identity().auth_public_key
    );
    assert_eq!(
        sender_peer.identity.node_id,
        receiver.local_identity().node_id
    );
    assert_eq!(
        sender_peer.identity.auth_public_key,
        receiver.local_identity().auth_public_key
    );
    assert_eq!(
        receiver_peer.transport.path.kind,
        ClusterTransportPathKind::DirectDatagram
    );
    assert_eq!(
        sender_peer.transport.path.kind,
        ClusterTransportPathKind::DirectDatagram
    );
    assert_eq!(
        receiver_peer
            .transport
            .multiplex_profile
            .max_concurrent_streams,
        2
    );
    let first_stream = receiver
        .open_logical_stream(
            &sender.local_identity().node_id,
            ClusterLogicalStreamKind::Serving,
        )
        .await;
    assert!(first_stream.is_ok(), "first logical stream should reserve");
    let second_stream = receiver
        .open_logical_stream(
            &sender.local_identity().node_id,
            ClusterLogicalStreamKind::Collective,
        )
        .await;
    assert!(
        second_stream.is_ok(),
        "second logical stream should reserve"
    );
    let third_stream = receiver
        .open_logical_stream(
            &sender.local_identity().node_id,
            ClusterLogicalStreamKind::Artifact,
        )
        .await;
    assert_eq!(
        third_stream,
        Err(ClusterStreamError::CapacityExceeded {
            peer_node_id: sender.local_identity().node_id.clone(),
            max_concurrent_streams: 2,
        })
    );
    let second_stream = second_stream
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let close = receiver.close_logical_stream(&second_stream).await;
    assert!(close.is_ok(), "closing a logical stream should succeed");
    let active_streams = receiver.active_logical_streams().await;
    assert_eq!(active_streams.len(), 1);
    assert_eq!(
        sender.local_identity().auth_public_key,
        sender_bootstrap.auth_public_key
    );
    assert_eq!(
        receiver.local_identity().auth_public_key,
        receiver_bootstrap.auth_public_key
    );

    let sender_shutdown = sender.shutdown().await;
    assert!(sender_shutdown.is_ok(), "sender should shut down cleanly");
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_authenticated_peer_is_refused_under_configured_peer_posture() {
    let receiver = LocalClusterNode::spawn(
        base_config(loopback_addr(0), NodeRole::CoordinatorOnly)
            .with_authenticated_configured_peers(Vec::new()),
    )
    .await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let intruder = LocalClusterNode::spawn(
        base_config(loopback_addr(0), NodeRole::ExecutorOnly)
            .with_seed_peers(vec![receiver.local_addr()]),
    )
    .await;
    assert!(intruder.is_ok(), "intruder should start");
    let intruder = intruder
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    assert_eq!(
        receiver.trust_policy().posture,
        ClusterTrustPosture::AuthenticatedConfiguredPeers
    );
    wait_for_no_peers(&receiver, Duration::from_millis(350)).await;
    wait_for_refusal(&receiver, |refusal| {
        refusal.remote_node_id.as_ref() == Some(&intruder.local_identity().node_id)
            && matches!(
                refusal.reason,
                ClusterJoinRefusalReason::ConfiguredPeerUnknown
            )
    })
    .await;

    let intruder_shutdown = intruder.shutdown().await;
    assert!(
        intruder_shutdown.is_ok(),
        "intruder should shut down cleanly"
    );
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authenticated_nodes_can_boot_from_operator_manifest() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let receiver_identity_path = temp.path().join("receiver-manifest-identity.json");
    let sender_identity_path = temp.path().join("sender-manifest-identity.json");
    let receiver_manifest_path = temp.path().join("receiver-manifest.json");
    let sender_manifest_path = temp.path().join("sender-manifest.json");
    let receiver_addr = reserve_loopback_addr();
    let sender_addr = reserve_loopback_addr();

    let receiver_bootstrap = bootstrap_file_backed_identity(
        &receiver_identity_path,
        receiver_addr,
        NodeRole::CoordinatorOnly,
    )
    .await;
    let sender_bootstrap =
        bootstrap_file_backed_identity(&sender_identity_path, sender_addr, NodeRole::ExecutorOnly)
            .await;

    let receiver_manifest_config = base_config(receiver_addr, NodeRole::CoordinatorOnly)
        .with_authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
            sender_bootstrap.node_id.clone(),
            sender_addr,
            sender_bootstrap.auth_public_key.clone(),
        )]);
    store_operator_manifest(&receiver_manifest_path, &receiver_manifest_config);

    let sender_manifest_config = base_config(sender_addr, NodeRole::ExecutorOnly)
        .with_authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
            receiver_bootstrap.node_id.clone(),
            receiver_addr,
            receiver_bootstrap.auth_public_key.clone(),
        )]);
    store_operator_manifest(&sender_manifest_path, &sender_manifest_config);

    let receiver_config = LocalClusterConfig::load_operator_manifest(&receiver_manifest_path);
    assert!(
        receiver_config.is_ok(),
        "receiver manifest config should load"
    );
    let receiver = LocalClusterNode::spawn(
        receiver_config
            .ok()
            .unwrap_or_else(|| unreachable!("assert above ensures success"))
            .with_file_backed_identity(receiver_identity_path),
    )
    .await;
    assert!(receiver.is_ok(), "receiver should start from manifest");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender_config = LocalClusterConfig::load_operator_manifest(&sender_manifest_path);
    assert!(sender_config.is_ok(), "sender manifest config should load");
    let sender = LocalClusterNode::spawn(
        sender_config
            .ok()
            .unwrap_or_else(|| unreachable!("assert above ensures success"))
            .with_file_backed_identity(sender_identity_path),
    )
    .await;
    assert!(sender.is_ok(), "sender should start from manifest");
    let sender = sender
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let receiver_peer = wait_for_single_peer(&receiver).await;
    let sender_peer = wait_for_single_peer(&sender).await;

    assert_eq!(
        receiver.trust_policy().posture,
        ClusterTrustPosture::AuthenticatedConfiguredPeers
    );
    assert_eq!(
        sender.trust_policy().posture,
        ClusterTrustPosture::AuthenticatedConfiguredPeers
    );
    assert_eq!(
        receiver_peer.identity.node_id,
        sender.local_identity().node_id
    );
    assert_eq!(
        sender_peer.identity.node_id,
        receiver.local_identity().node_id
    );

    let sender_shutdown = sender.shutdown().await;
    assert!(sender_shutdown.is_ok(), "sender should shut down cleanly");
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nat_rendezvous_relays_surface_nat_traversal_paths() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let receiver_identity_path = temp.path().join("receiver-nat-identity.json");
    let sender_identity_path = temp.path().join("sender-nat-identity.json");
    let receiver_addr = reserve_loopback_addr();
    let sender_addr = reserve_loopback_addr();
    let receiver_placeholder_addr = reserve_loopback_addr();
    let sender_placeholder_addr = reserve_loopback_addr();
    let relay = ClusterRelayServer::spawn(loopback_addr(0)).await;
    assert!(relay.is_ok(), "relay should start");
    let relay = relay
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let relay_endpoint = ClusterRelayEndpoint::new("relay-nat", relay.local_addr(), "pair-nat");
    let dial_policy = ConfiguredPeerDialPolicy {
        base_backoff_ticks: 1,
        max_backoff_ticks: 1,
        degraded_after_unanswered_hellos: 1,
        unreachable_after_unanswered_hellos: 4,
    };

    let receiver_bootstrap = bootstrap_file_backed_identity(
        &receiver_identity_path,
        receiver_addr,
        NodeRole::CoordinatorOnly,
    )
    .await;
    let sender_bootstrap =
        bootstrap_file_backed_identity(&sender_identity_path, sender_addr, NodeRole::ExecutorOnly)
            .await;

    let receiver = LocalClusterNode::spawn(
        base_config(receiver_addr, NodeRole::CoordinatorOnly)
            .with_file_backed_identity(receiver_identity_path)
            .with_authenticated_configured_peers(vec![
                ConfiguredClusterPeer::new(
                    sender_bootstrap.node_id.clone(),
                    sender_placeholder_addr,
                    sender_bootstrap.auth_public_key.clone(),
                )
                .with_nat_rendezvous_relays(vec![relay_endpoint.clone()]),
            ])
            .with_configured_peer_dial_policy(dial_policy),
    )
    .await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender = LocalClusterNode::spawn(
        base_config(sender_addr, NodeRole::ExecutorOnly)
            .with_file_backed_identity(sender_identity_path)
            .with_authenticated_configured_peers(vec![
                ConfiguredClusterPeer::new(
                    receiver_bootstrap.node_id.clone(),
                    receiver_placeholder_addr,
                    receiver_bootstrap.auth_public_key.clone(),
                )
                .with_nat_rendezvous_relays(vec![relay_endpoint.clone()]),
            ])
            .with_configured_peer_dial_policy(dial_policy),
    )
    .await;
    assert!(sender.is_ok(), "sender should start");
    let sender = sender
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let receiver_peer = wait_for_single_peer(&receiver).await;
    let sender_peer = wait_for_single_peer(&sender).await;

    assert_eq!(
        receiver_peer.transport.path.kind,
        ClusterTransportPathKind::NatTraversalDatagram
    );
    assert_eq!(
        sender_peer.transport.path.kind,
        ClusterTransportPathKind::NatTraversalDatagram
    );
    assert_eq!(receiver_peer.remote_addr, sender.local_addr());
    assert_eq!(sender_peer.remote_addr, receiver.local_addr());
    assert_eq!(
        receiver_peer
            .transport
            .path
            .relay
            .as_ref()
            .map(|relay| relay.relay_addr),
        Some(relay.local_addr())
    );

    let sender_shutdown = sender.shutdown().await;
    assert!(sender_shutdown.is_ok(), "sender should shut down cleanly");
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
    let relay_shutdown = relay.shutdown().await;
    assert!(relay_shutdown.is_ok(), "relay should shut down cleanly");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relay_fallback_surfaces_relayed_transport_path() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let receiver_identity_path = temp.path().join("receiver-relay-identity.json");
    let sender_identity_path = temp.path().join("sender-relay-identity.json");
    let receiver_addr = reserve_loopback_addr();
    let sender_addr = reserve_loopback_addr();
    let receiver_placeholder_addr = reserve_loopback_addr();
    let sender_placeholder_addr = reserve_loopback_addr();
    let relay = ClusterRelayServer::spawn(loopback_addr(0)).await;
    assert!(relay.is_ok(), "relay should start");
    let relay = relay
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let relay_endpoint =
        ClusterRelayEndpoint::new("relay-forward", relay.local_addr(), "pair-forward");
    let dial_policy = ConfiguredPeerDialPolicy {
        base_backoff_ticks: 1,
        max_backoff_ticks: 1,
        degraded_after_unanswered_hellos: 1,
        unreachable_after_unanswered_hellos: 1,
    };

    let receiver_bootstrap = bootstrap_file_backed_identity(
        &receiver_identity_path,
        receiver_addr,
        NodeRole::CoordinatorOnly,
    )
    .await;
    let sender_bootstrap =
        bootstrap_file_backed_identity(&sender_identity_path, sender_addr, NodeRole::ExecutorOnly)
            .await;

    let receiver = LocalClusterNode::spawn(
        base_config(receiver_addr, NodeRole::CoordinatorOnly)
            .with_file_backed_identity(receiver_identity_path)
            .with_authenticated_configured_peers(vec![
                ConfiguredClusterPeer::new(
                    sender_bootstrap.node_id.clone(),
                    sender_placeholder_addr,
                    sender_bootstrap.auth_public_key.clone(),
                )
                .with_relay_fallback_relays(vec![relay_endpoint.clone()]),
            ])
            .with_configured_peer_dial_policy(dial_policy),
    )
    .await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender = LocalClusterNode::spawn(
        base_config(sender_addr, NodeRole::ExecutorOnly)
            .with_file_backed_identity(sender_identity_path)
            .with_authenticated_configured_peers(vec![
                ConfiguredClusterPeer::new(
                    receiver_bootstrap.node_id.clone(),
                    receiver_placeholder_addr,
                    receiver_bootstrap.auth_public_key.clone(),
                )
                .with_relay_fallback_relays(vec![relay_endpoint.clone()]),
            ])
            .with_configured_peer_dial_policy(dial_policy),
    )
    .await;
    assert!(sender.is_ok(), "sender should start");
    let sender = sender
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let receiver_peer = wait_for_single_peer(&receiver).await;
    let sender_peer = wait_for_single_peer(&sender).await;

    assert_eq!(
        receiver_peer.transport.path.kind,
        ClusterTransportPathKind::RelayedDatagram
    );
    assert_eq!(
        sender_peer.transport.path.kind,
        ClusterTransportPathKind::RelayedDatagram
    );
    assert_eq!(receiver_peer.remote_addr, relay.local_addr());
    assert_eq!(sender_peer.remote_addr, relay.local_addr());
    assert_eq!(
        receiver_peer
            .transport
            .path
            .relay
            .as_ref()
            .map(|relay| relay.relay_addr),
        Some(relay.local_addr())
    );

    let sender_shutdown = sender.shutdown().await;
    assert!(sender_shutdown.is_ok(), "sender should shut down cleanly");
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
    let relay_shutdown = relay.shutdown().await;
    assert!(relay_shutdown.is_ok(), "relay should shut down cleanly");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bounded_http_tunnels_forward_requests_and_surface_lifecycle() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let receiver_identity_path = temp.path().join("receiver-tunnel-identity.json");
    let sender_identity_path = temp.path().join("sender-tunnel-identity.json");
    let receiver_addr = reserve_loopback_addr();
    let sender_addr = reserve_loopback_addr();
    let (service_addr, observed_requests, service_shutdown_tx, service_task) =
        spawn_http_echo_service().await;

    let receiver_bootstrap = bootstrap_file_backed_identity(
        &receiver_identity_path,
        receiver_addr,
        NodeRole::CoordinatorOnly,
    )
    .await;
    let sender_bootstrap =
        bootstrap_file_backed_identity(&sender_identity_path, sender_addr, NodeRole::ExecutorOnly)
            .await;

    let receiver = LocalClusterNode::spawn(
        base_config(receiver_addr, NodeRole::CoordinatorOnly)
            .with_file_backed_identity(receiver_identity_path)
            .with_authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                sender_bootstrap.node_id.clone(),
                sender_addr,
                sender_bootstrap.auth_public_key.clone(),
            )])
            .with_tunnel_policy(ClusterTunnelPolicy::new(vec![
                ClusterTunnelServicePolicy::new_http(
                    "desktop-control",
                    ClusterTunnelServiceKind::DesktopControlHttp,
                    service_addr,
                )
                .with_allowed_peer_node_ids(vec![sender_bootstrap.node_id.clone()])
                .with_max_request_body_bytes(2048)
                .with_max_response_body_bytes(4096),
            ])),
    )
    .await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender = LocalClusterNode::spawn(
        base_config(sender_addr, NodeRole::ExecutorOnly)
            .with_file_backed_identity(sender_identity_path)
            .with_authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                receiver_bootstrap.node_id.clone(),
                receiver_addr,
                receiver_bootstrap.auth_public_key.clone(),
            )]),
    )
    .await;
    assert!(sender.is_ok(), "sender should start");
    let sender = sender
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let _receiver_peer = wait_for_single_peer(&receiver).await;
    let _sender_peer = wait_for_single_peer(&sender).await;

    let receiver_services = receiver.tunnel_service_snapshots().await;
    assert_eq!(receiver_services.len(), 1);
    assert!(!receiver_services[0].active);

    let activated = receiver.activate_tunnel_service("desktop-control").await;
    assert!(activated.is_ok(), "approved tunnel service should activate");
    let activated = activated
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    assert!(activated.active);

    let lease = sender
        .open_http_tunnel(&receiver.local_identity().node_id, "desktop-control")
        .await;
    assert!(lease.is_ok(), "open tunnel should succeed");
    let lease = lease
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender_tunnel =
        wait_for_tunnel_state(&sender, lease.tunnel_id, ClusterTunnelState::Open).await;
    let receiver_tunnel =
        wait_for_tunnel_state(&receiver, lease.tunnel_id, ClusterTunnelState::Open).await;
    assert_eq!(sender_tunnel.service_id, "desktop-control");
    assert_eq!(receiver_tunnel.service_id, "desktop-control");

    let response = sender
        .send_tunneled_http_request(
            &lease,
            ClusterTunnelHttpRequest::new("POST", "/health?ready=1")
                .with_header("content-type", "text/plain; charset=utf-8")
                .with_utf8_body("hello tunnel"),
        )
        .await;
    assert!(response.is_ok(), "HTTP tunnel request should succeed");
    let response = response
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    assert_eq!(response.status_code, 200);
    let response_body = response.body_bytes();
    assert!(response_body.is_ok(), "response body should decode");
    let response_body = String::from_utf8(
        response_body
            .ok()
            .unwrap_or_else(|| unreachable!("assert above ensures success")),
    );
    assert!(response_body.is_ok(), "response body should be valid UTF-8");
    let response_body = response_body
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    assert!(response_body.contains("method=POST"));
    assert!(response_body.contains("path=/health?ready=1"));
    assert!(response_body.contains("body=hello tunnel"));

    let observed_requests = observed_requests.lock();
    assert!(
        observed_requests.is_ok(),
        "observed requests lock should succeed"
    );
    let observed_requests = observed_requests
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    assert!(
        observed_requests
            .iter()
            .any(|line| line.contains("POST /health?ready=1 HTTP/1.1")),
        "local service should observe the forwarded request line"
    );
    drop(observed_requests);

    let sender_tunnel = sender
        .tunnel_snapshots()
        .await
        .into_iter()
        .find(|tunnel| tunnel.tunnel_id == lease.tunnel_id);
    assert!(
        sender_tunnel.is_some(),
        "sender tunnel snapshot should exist"
    );
    let sender_tunnel =
        sender_tunnel.unwrap_or_else(|| unreachable!("assert above ensures success"));
    assert_eq!(sender_tunnel.transport.requests_sent, 1);
    assert_eq!(sender_tunnel.transport.responses_received, 1);
    assert!(sender_tunnel.transport.bytes_sent > 0);
    assert!(sender_tunnel.transport.bytes_received > 0);

    let receiver_service = receiver
        .tunnel_service_snapshots()
        .await
        .into_iter()
        .find(|service| service.service_id == "desktop-control");
    assert!(
        receiver_service.is_some(),
        "receiver tunnel service should exist"
    );
    let receiver_service =
        receiver_service.unwrap_or_else(|| unreachable!("assert above ensures success"));
    assert_eq!(receiver_service.request_count, 1);
    assert_eq!(receiver_service.response_count, 1);
    assert!(receiver_service.bytes_in > 0);
    assert!(receiver_service.bytes_out > 0);

    let close = sender.close_tunnel(&lease).await;
    assert!(close.is_ok(), "tunnel should close");
    let _sender_closed =
        wait_for_tunnel_state(&sender, lease.tunnel_id, ClusterTunnelState::Closed).await;
    let _receiver_closed =
        wait_for_tunnel_state(&receiver, lease.tunnel_id, ClusterTunnelState::Closed).await;

    let service_shutdown = service_shutdown_tx.send(());
    assert!(
        service_shutdown.is_ok(),
        "service shutdown signal should send cleanly"
    );
    let service_task = service_task.await;
    assert!(service_task.is_ok(), "service task should stop cleanly");

    let sender_shutdown = sender.shutdown().await;
    assert!(sender_shutdown.is_ok(), "sender should shut down cleanly");
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn inactive_tunnel_services_refuse_open_requests() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let receiver_identity_path = temp.path().join("receiver-tunnel-refusal.json");
    let sender_identity_path = temp.path().join("sender-tunnel-refusal.json");
    let receiver_addr = reserve_loopback_addr();
    let sender_addr = reserve_loopback_addr();
    let (service_addr, _observed_requests, service_shutdown_tx, service_task) =
        spawn_http_echo_service().await;

    let receiver_bootstrap = bootstrap_file_backed_identity(
        &receiver_identity_path,
        receiver_addr,
        NodeRole::CoordinatorOnly,
    )
    .await;
    let sender_bootstrap =
        bootstrap_file_backed_identity(&sender_identity_path, sender_addr, NodeRole::ExecutorOnly)
            .await;

    let receiver = LocalClusterNode::spawn(
        base_config(receiver_addr, NodeRole::CoordinatorOnly)
            .with_file_backed_identity(receiver_identity_path)
            .with_authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                sender_bootstrap.node_id.clone(),
                sender_addr,
                sender_bootstrap.auth_public_key.clone(),
            )])
            .with_tunnel_policy(ClusterTunnelPolicy::new(vec![
                ClusterTunnelServicePolicy::new_http(
                    "desktop-control",
                    ClusterTunnelServiceKind::DesktopControlHttp,
                    service_addr,
                )
                .with_allowed_peer_node_ids(vec![sender_bootstrap.node_id.clone()]),
            ])),
    )
    .await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender = LocalClusterNode::spawn(
        base_config(sender_addr, NodeRole::ExecutorOnly)
            .with_file_backed_identity(sender_identity_path)
            .with_authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                receiver_bootstrap.node_id.clone(),
                receiver_addr,
                receiver_bootstrap.auth_public_key.clone(),
            )]),
    )
    .await;
    assert!(sender.is_ok(), "sender should start");
    let sender = sender
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let _receiver_peer = wait_for_single_peer(&receiver).await;
    let _sender_peer = wait_for_single_peer(&sender).await;

    let lease = sender
        .open_http_tunnel(&receiver.local_identity().node_id, "desktop-control")
        .await;
    assert_eq!(
        lease,
        Err(ClusterTunnelError::OpenRefused {
            service_id: String::from("desktop-control"),
            reason: psionic_cluster::ClusterTunnelOpenRefusalReason::ServiceInactive,
        })
    );

    let wait_for_refused = timeout(Duration::from_secs(3), async {
        loop {
            if sender.tunnel_snapshots().await.into_iter().any(|tunnel| {
                tunnel.service_id == "desktop-control"
                    && tunnel.state == ClusterTunnelState::Refused
            }) {
                return;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(
        wait_for_refused.is_ok(),
        "refused tunnel snapshot should be retained"
    );

    let service_shutdown = service_shutdown_tx.send(());
    assert!(
        service_shutdown.is_ok(),
        "service shutdown signal should send cleanly"
    );
    let service_task = service_task.await;
    assert!(service_task.is_ok(), "service task should stop cleanly");

    let sender_shutdown = sender.shutdown().await;
    assert!(sender_shutdown.is_ok(), "sender should shut down cleanly");
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unreachable_configured_peer_surfaces_explicit_health_and_backoff() {
    let dial_policy = ConfiguredPeerDialPolicy {
        base_backoff_ticks: 1,
        max_backoff_ticks: 2,
        degraded_after_unanswered_hellos: 2,
        unreachable_after_unanswered_hellos: 3,
    };
    let node = LocalClusterNode::spawn(
        base_config(loopback_addr(0), NodeRole::CoordinatorOnly)
            .with_authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                psionic_cluster::NodeId::new("missing-peer"),
                reserve_loopback_addr(),
                "00".repeat(32),
            )])
            .with_configured_peer_dial_policy(dial_policy),
    )
    .await;
    assert!(node.is_ok(), "node should start");
    let node = node
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let health = wait_for_configured_peer_health(&node, |snapshot| {
        snapshot.node_id == psionic_cluster::NodeId::new("missing-peer")
            && snapshot.reachability == ConfiguredPeerReachability::Unreachable
            && snapshot.remaining_backoff_ticks > 0
    })
    .await;
    assert!(health.unanswered_hello_attempts >= 3);

    let shutdown = node.shutdown().await;
    assert!(shutdown.is_ok(), "node should shut down cleanly");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_joining_configured_peer_recovers_health_after_degraded_attempts() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let receiver_identity_path = temp.path().join("receiver-dial-identity.json");
    let sender_identity_path = temp.path().join("sender-dial-identity.json");
    let receiver_addr = reserve_loopback_addr();
    let sender_addr = reserve_loopback_addr();
    let dial_policy = ConfiguredPeerDialPolicy {
        base_backoff_ticks: 1,
        max_backoff_ticks: 2,
        degraded_after_unanswered_hellos: 2,
        unreachable_after_unanswered_hellos: 4,
    };

    let receiver_bootstrap = bootstrap_file_backed_identity(
        &receiver_identity_path,
        receiver_addr,
        NodeRole::CoordinatorOnly,
    )
    .await;
    let sender_bootstrap =
        bootstrap_file_backed_identity(&sender_identity_path, sender_addr, NodeRole::ExecutorOnly)
            .await;

    let receiver = LocalClusterNode::spawn(
        base_config(receiver_addr, NodeRole::CoordinatorOnly)
            .with_file_backed_identity(receiver_identity_path.clone())
            .with_authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                sender_bootstrap.node_id.clone(),
                sender_addr,
                sender_bootstrap.auth_public_key.clone(),
            )])
            .with_configured_peer_dial_policy(dial_policy),
    )
    .await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let degraded = wait_for_configured_peer_health(&receiver, |snapshot| {
        snapshot.node_id == sender_bootstrap.node_id
            && snapshot.reachability == ConfiguredPeerReachability::Degraded
    })
    .await;
    assert!(degraded.unanswered_hello_attempts >= 2);

    let sender = LocalClusterNode::spawn(
        base_config(sender_addr, NodeRole::ExecutorOnly)
            .with_file_backed_identity(sender_identity_path)
            .with_authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                receiver_bootstrap.node_id.clone(),
                receiver_addr,
                receiver_bootstrap.auth_public_key.clone(),
            )])
            .with_configured_peer_dial_policy(dial_policy),
    )
    .await;
    assert!(sender.is_ok(), "sender should start");
    let sender = sender
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let recovered = wait_for_configured_peer_health(&receiver, |snapshot| {
        snapshot.node_id == sender_bootstrap.node_id
            && snapshot.reachability == ConfiguredPeerReachability::Reachable
            && snapshot.successful_handshakes >= 1
    })
    .await;
    assert_eq!(recovered.unanswered_hello_attempts, 0);

    let sender_shutdown = sender.shutdown().await;
    assert!(sender_shutdown.is_ok(), "sender should shut down cleanly");
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rotated_key_overlap_is_surfaceable_during_bundle_rollout() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let receiver_identity_path = temp.path().join("receiver-rotation-identity.json");
    let sender_identity_path = temp.path().join("sender-rotation-identity.json");
    let receiver_addr = reserve_loopback_addr();
    let sender_addr = reserve_loopback_addr();

    let receiver_bootstrap = bootstrap_file_backed_identity(
        &receiver_identity_path,
        receiver_addr,
        NodeRole::CoordinatorOnly,
    )
    .await;
    let sender_bootstrap =
        bootstrap_file_backed_identity(&sender_identity_path, sender_addr, NodeRole::ExecutorOnly)
            .await;
    let sender_future_key = auth_public_key_for_test(61);

    let receiver = LocalClusterNode::spawn(
        base_config(receiver_addr, NodeRole::CoordinatorOnly)
            .with_file_backed_identity(receiver_identity_path)
            .with_trust_policy(
                ClusterTrustPolicy::authenticated_configured_peers(vec![
                    ConfiguredClusterPeer::new(
                        sender_bootstrap.node_id.clone(),
                        sender_addr,
                        sender_future_key,
                    )
                    .with_previous_auth_public_keys(vec![sender_bootstrap.auth_public_key.clone()]),
                ])
                .with_trust_bundle_version(2)
                .with_accepted_trust_bundle_versions(vec![1]),
            ),
    )
    .await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender = LocalClusterNode::spawn(
        base_config(sender_addr, NodeRole::ExecutorOnly)
            .with_file_backed_identity(sender_identity_path)
            .with_trust_policy(
                ClusterTrustPolicy::authenticated_configured_peers(vec![
                    ConfiguredClusterPeer::new(
                        receiver_bootstrap.node_id.clone(),
                        receiver_addr,
                        receiver_bootstrap.auth_public_key.clone(),
                    ),
                ])
                .with_trust_bundle_version(1)
                .with_accepted_trust_bundle_versions(vec![2]),
            ),
    )
    .await;
    assert!(sender.is_ok(), "sender should start");
    let sender = sender
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let receiver_peer = wait_for_single_peer(&receiver).await;
    assert_eq!(
        receiver_peer.identity.node_id,
        sender.local_identity().node_id
    );
    let diagnostic = wait_for_rollout_diagnostic(&receiver, |diagnostic| {
        diagnostic.remote_node_id == sender.local_identity().node_id
            && diagnostic.disposition == ClusterTrustRolloutDisposition::AcceptedOverlap
            && diagnostic.actual_trust_bundle_version == Some(1)
            && diagnostic.key_match == Some(ConfiguredPeerKeyMatch::Previous)
    })
    .await;
    assert_eq!(diagnostic.expected_trust_bundle_version, 2);

    let sender_shutdown = sender.shutdown().await;
    assert!(sender_shutdown.is_ok(), "sender should shut down cleanly");
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_trust_bundle_version_is_refused_and_diagnostic_is_recorded() {
    let temp = tempdir();
    assert!(temp.is_ok(), "temp dir should exist");
    let temp = temp
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));
    let receiver_identity_path = temp.path().join("receiver-stale-bundle-identity.json");
    let sender_identity_path = temp.path().join("sender-stale-bundle-identity.json");
    let receiver_addr = reserve_loopback_addr();
    let sender_addr = reserve_loopback_addr();

    let receiver_bootstrap = bootstrap_file_backed_identity(
        &receiver_identity_path,
        receiver_addr,
        NodeRole::CoordinatorOnly,
    )
    .await;
    let sender_bootstrap =
        bootstrap_file_backed_identity(&sender_identity_path, sender_addr, NodeRole::ExecutorOnly)
            .await;

    let receiver = LocalClusterNode::spawn(
        base_config(receiver_addr, NodeRole::CoordinatorOnly)
            .with_file_backed_identity(receiver_identity_path)
            .with_trust_policy(
                ClusterTrustPolicy::authenticated_configured_peers(vec![
                    ConfiguredClusterPeer::new(
                        sender_bootstrap.node_id.clone(),
                        sender_addr,
                        sender_bootstrap.auth_public_key.clone(),
                    ),
                ])
                .with_trust_bundle_version(2),
            ),
    )
    .await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender = LocalClusterNode::spawn(
        base_config(sender_addr, NodeRole::ExecutorOnly)
            .with_file_backed_identity(sender_identity_path)
            .with_trust_policy(
                ClusterTrustPolicy::authenticated_configured_peers(vec![
                    ConfiguredClusterPeer::new(
                        receiver_bootstrap.node_id.clone(),
                        receiver_addr,
                        receiver_bootstrap.auth_public_key.clone(),
                    ),
                ])
                .with_trust_bundle_version(1),
            ),
    )
    .await;
    assert!(sender.is_ok(), "sender should start");
    let sender = sender
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    wait_for_refusal(&receiver, |refusal| {
        refusal.remote_node_id.as_ref() == Some(&sender.local_identity().node_id)
            && matches!(
                refusal.reason,
                ClusterJoinRefusalReason::TrustBundleVersionMismatch {
                    expected: 2,
                    actual: Some(1),
                    ..
                }
            )
    })
    .await;
    let diagnostic = wait_for_rollout_diagnostic(&receiver, |diagnostic| {
        diagnostic.remote_node_id == sender.local_identity().node_id
            && diagnostic.disposition == ClusterTrustRolloutDisposition::RefusedVersionMismatch
            && diagnostic.actual_trust_bundle_version == Some(1)
    })
    .await;
    assert_eq!(diagnostic.expected_trust_bundle_version, 2);

    let sender_shutdown = sender.shutdown().await;
    assert!(sender_shutdown.is_ok(), "sender should shut down cleanly");
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}
