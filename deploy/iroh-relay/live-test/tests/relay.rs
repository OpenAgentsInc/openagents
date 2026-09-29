//! Live checks against the deployed relay. Both are `#[ignore]`: they need
//! the internet and the relay at `IROH_RELAY_URL` (default
//! `https://iroh.openagents.com`). No n0 relay or n0 address lookup is
//! configured on any endpoint here.

use std::time::Duration;

use iroh::{Endpoint, RelayMode, RelayUrl, Watcher, endpoint::presets};

const ALPN: &[u8] = b"openagents/iroh-relay-live-test/0";
const DEADLINE: Duration = Duration::from_secs(30);

fn relay_url() -> RelayUrl {
    std::env::var("IROH_RELAY_URL")
        .unwrap_or_else(|_| "https://iroh.openagents.com".to_owned())
        .parse()
        .expect("IROH_RELAY_URL must be a URL")
}

/// An endpoint whose only transport is our relay: IP transports cleared,
/// no address lookup (the Minimal preset has none), relay map = ours only.
async fn relay_only(alpns: Vec<Vec<u8>>) -> Endpoint {
    Endpoint::builder(presets::Minimal)
        .clear_ip_transports()
        .relay_mode(RelayMode::custom([relay_url()]))
        .alpns(alpns)
        .bind()
        .await
        .expect("bind relay-only endpoint")
}

#[tokio::test]
#[ignore = "live: needs the deployed relay"]
async fn two_relay_only_endpoints_exchange_data_through_the_relay() {
    let server = relay_only(vec![ALPN.to_vec()]).await;
    let client = relay_only(vec![]).await;
    tokio::time::timeout(DEADLINE, server.online())
        .await
        .expect("server reached the relay");

    let addr = server.addr();
    assert!(
        addr.ip_addrs().next().is_none(),
        "server must advertise no direct address: {addr:?}"
    );
    assert_eq!(addr.relay_urls().next(), Some(&relay_url()));

    let accept = tokio::spawn({
        let server = server.clone();
        async move {
            let conn = server
                .accept()
                .await
                .expect("incoming")
                .await
                .expect("handshake");
            let (mut send, mut recv) = conn.accept_bi().await.expect("accept_bi");
            let got = recv.read_to_end(1 << 20).await.expect("read");
            send.write_all(&got).await.expect("echo");
            send.finish().expect("finish");
            conn.closed().await;
            got.len()
        }
    });

    let payload: Vec<u8> = (0..64 * 1024).map(|i| (i % 251) as u8).collect();
    let conn = tokio::time::timeout(DEADLINE, client.connect(addr, ALPN))
        .await
        .expect("connect in time")
        .expect("connect");
    let (mut send, mut recv) = conn.open_bi().await.expect("open_bi");
    send.write_all(&payload).await.expect("write");
    send.finish().expect("finish");
    let echoed = tokio::time::timeout(DEADLINE, recv.read_to_end(1 << 20))
        .await
        .expect("echo in time")
        .expect("read echo");
    assert_eq!(echoed, payload);

    let paths = conn.paths();
    assert!(!paths.is_empty(), "connection has a path");
    for path in paths.iter() {
        assert!(
            path.is_relay(),
            "every path is a relay path: {:?}",
            path.remote_addr()
        );
        println!("path {:?} rtt {:?}", path.remote_addr(), path.rtt());
    }

    conn.close(0u32.into(), b"done");
    assert_eq!(accept.await.expect("server task"), payload.len());
    client.close().await;
    server.close().await;
}

#[tokio::test]
#[ignore = "live: needs the deployed relay"]
async fn quic_address_discovery_reports_a_public_address() {
    // IP transports on, so net_report runs QAD over UDP 7842 against our relay.
    let ep = Endpoint::builder(presets::Minimal)
        .relay_mode(RelayMode::custom([relay_url()]))
        .bind()
        .await
        .expect("bind");
    let mut watcher = ep.net_report();
    let report = tokio::time::timeout(DEADLINE, async {
        loop {
            if let Some(r) = watcher.get() {
                if r.udp_v4 || r.udp_v6 {
                    return r;
                }
            }
            watcher.updated().await.expect("net_report watcher");
        }
    })
    .await
    .expect("QAD answered in time");
    println!(
        "QAD: udp_v4={} global_v4={:?} udp_v6={} global_v6={:?}",
        report.udp_v4, report.global_v4, report.udp_v6, report.global_v6
    );
    assert!(report.global_v4.is_some() || report.global_v6.is_some());
    ep.close().await;
}
