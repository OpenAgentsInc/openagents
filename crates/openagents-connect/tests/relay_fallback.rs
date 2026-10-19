//! Real QUIC and a local iroh relay; no production service or firewall needed.
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use iroh::endpoint::Connection;
use iroh::{EndpointAddr, SecretKey};
use iroh_relay::server::{RelayConfig, Server, ServerConfig};
use openagents_connect::endpoint::{
    ConnectEndpoint, EndpointConfig, PATH_KEEP_ALIVE, PATH_MAX_IDLE, Relay,
};
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

const ALPN: &[u8] = b"openagents/test-relay/1";

// A switchable UDP route. Dropping both directions models a path that worked
// during validation, then stops acknowledging packets. The proxy stays
// bound, so no ICMP reply can shortcut the no-ack failure detection.
struct DirectRoute {
    addr: SocketAddr,
    blocked: Arc<AtomicBool>,
    task: JoinHandle<()>,
}

impl Drop for DirectRoute {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl DirectRoute {
    async fn new(host: SocketAddr) -> Self {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = socket.local_addr().unwrap();
        let blocked = Arc::new(AtomicBool::new(false));
        let drop_packets = blocked.clone();
        let task = tokio::spawn(async move {
            let mut phone = None;
            let mut buf = vec![0; 65536];
            loop {
                let (len, from) = socket.recv_from(&mut buf).await.unwrap();
                if drop_packets.load(Ordering::SeqCst) {
                    continue;
                }
                let to = if from == host {
                    let Some(phone) = phone else {
                        continue;
                    };
                    phone
                } else {
                    phone = Some(from);
                    host
                };
                socket.send_to(&buf[..len], to).await.unwrap();
            }
        });
        Self {
            addr,
            blocked,
            task,
        }
    }
}

async fn endpoint(relay: &iroh::RelayUrl, alpns: Vec<Vec<u8>>) -> ConnectEndpoint {
    ConnectEndpoint::bind(
        SecretKey::generate(),
        EndpointConfig {
            relay: Relay::Custom(relay.clone()),
            bind: vec![(Ipv4Addr::LOCALHOST, 0).into()],
            alpns,
        },
    )
    .await
    .unwrap()
}

fn selected_relay(conn: &Connection) -> bool {
    conn.paths().iter().any(|p| p.is_selected() && p.is_relay())
}

async fn round_trip(conn: &Connection) {
    let (mut send, mut recv) = conn.open_bi().await.unwrap();
    send.write_all(b"request").await.unwrap();
    send.finish().unwrap();
    assert_eq!(recv.read_to_end(128).await.unwrap(), b"request");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_silent_direct_path_is_abandoned_and_calls_stay_on_the_relay() {
    let mut config = ServerConfig::default();
    config.relay = Some(RelayConfig::new((Ipv4Addr::LOCALHOST, 0)));
    let relay = Server::spawn(config).await.unwrap();
    let url = format!("http://{}", relay.http_addr().unwrap())
        .parse()
        .unwrap();
    let host = endpoint(&url, vec![ALPN.to_vec()]).await;
    let phone = endpoint(&url, vec![]).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        host.endpoint.online().await;
        phone.endpoint.online().await;
    })
    .await
    .expect("local relay did not become ready");
    let route = DirectRoute::new(host.endpoint.bound_sockets()[0]).await;
    let addr = EndpointAddr::new(host.endpoint.id())
        .with_relay_url(url)
        .with_ip_addr(route.addr);
    let server = host.endpoint.clone();
    let echo = tokio::spawn(async move {
        let conn = server.accept().await.unwrap().await.unwrap();
        while let Ok((mut send, mut recv)) = conn.accept_bi().await {
            let request = recv.read_to_end(128).await.unwrap();
            send.write_all(&request).await.unwrap();
            send.finish().unwrap();
        }
    });
    let conn = tokio::time::timeout(Duration::from_secs(5), phone.endpoint.connect(addr, ALPN))
        .await
        .expect("local dial timed out")
        .unwrap();
    // Ensure this actually exercises loss of the selected direct path, with
    // an established relay backup, rather than testing an already relayed dial.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            round_trip(&conn).await;
            if conn.paths().iter().any(|p| p.is_selected() && p.is_ip())
                && conn.paths().iter().any(|p| p.is_relay())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("direct path and relay backup never established");
    // The shorter lease must preserve healthy direct paths during idle time.
    tokio::time::sleep(PATH_MAX_IDLE + PATH_KEEP_ALIVE).await;
    assert!(conn.paths().iter().any(|p| p.is_selected() && p.is_ip()));
    tokio::time::timeout(Duration::from_secs(1), round_trip(&conn))
        .await
        .expect("healthy idle direct path stopped answering");
    route.blocked.store(true, Ordering::SeqCst);
    let start = Instant::now();
    tokio::time::timeout(PATH_MAX_IDLE + Duration::from_secs(2), round_trip(&conn))
        .await
        .expect("call stalled on the unreachable direct path");
    assert!(
        selected_relay(&conn),
        "call bypassed the blocked route via another direct path"
    );
    eprintln!("first call after UDP blackhole: {:?}", start.elapsed());
    // Span the former 12s request deadline: idle and later calls must keep
    // using the relay rather than flipping back to the unusable direct path.
    for _ in 0..14 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        tokio::time::timeout(Duration::from_secs(1), round_trip(&conn))
            .await
            .expect("later call stalled after relay fallback");
        assert!(selected_relay(&conn));
    }
    phone.close().await;
    host.close().await;
    echo.abort();
    relay.shutdown().await.unwrap();
}
