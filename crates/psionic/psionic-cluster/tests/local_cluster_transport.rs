use std::{
    net::{SocketAddr, UdpSocket as StdUdpSocket},
    path::Path,
    time::Duration,
};

use psionic_cluster::{
    ClusterJoinRefusalReason, ClusterTrustPosture, ConfiguredClusterPeer, LocalClusterConfig,
    LocalClusterNode, NodeRole,
};
use tempfile::tempdir;
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
            .with_authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                sender_bootstrap.node_id.clone(),
                sender_addr,
                sender_bootstrap.auth_public_key.clone(),
            )]),
    )
    .await;
    assert!(receiver.is_ok(), "receiver should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender = LocalClusterNode::spawn(
        base_config(sender_addr, NodeRole::ExecutorOnly)
            .with_file_backed_identity(sender_path)
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
