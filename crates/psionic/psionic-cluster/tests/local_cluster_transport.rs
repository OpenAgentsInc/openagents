use std::{net::SocketAddr, time::Duration};

use psionic_cluster::{LocalClusterConfig, LocalClusterNode, NodeRole};
use tokio::time::{Instant, sleep, timeout};

fn loopback_addr(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn seeded_local_nodes_discover_each_other_and_exchange_hello_and_ping() {
    let executor = LocalClusterNode::spawn(LocalClusterConfig::new(
        "lan-alpha",
        "shared-secret",
        loopback_addr(0),
        NodeRole::ExecutorOnly,
    ))
    .await;
    assert!(executor.is_ok(), "executor node should start");
    let executor = executor
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let coordinator = LocalClusterNode::spawn(
        LocalClusterConfig::new(
            "lan-alpha",
            "shared-secret",
            loopback_addr(0),
            NodeRole::CoordinatorOnly,
        )
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
async fn mismatched_admission_token_prevents_discovery() {
    let receiver = LocalClusterNode::spawn(LocalClusterConfig::new(
        "lan-alpha",
        "shared-secret",
        loopback_addr(0),
        NodeRole::Mixed,
    ))
    .await;
    assert!(receiver.is_ok(), "receiver node should start");
    let receiver = receiver
        .ok()
        .unwrap_or_else(|| unreachable!("assert above ensures success"));

    let sender = LocalClusterNode::spawn(
        LocalClusterConfig::new(
            "lan-alpha",
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

    let sender_shutdown = sender.shutdown().await;
    assert!(sender_shutdown.is_ok(), "sender should shut down cleanly");
    let receiver_shutdown = receiver.shutdown().await;
    assert!(
        receiver_shutdown.is_ok(),
        "receiver should shut down cleanly"
    );
}
