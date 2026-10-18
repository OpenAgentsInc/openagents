mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use archil::{Archil, ClientOptions, CreateDiskRequest, Error};
use common::*;
use serde_json::json;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer};

#[tokio::test]
async fn reads_retry_transient_statuses_until_success() {
    for code in [429, 500, 502, 503, 504] {
        let server = MockServer::start().await;
        let seq = Seq::new(vec![
            err(code, "later", None),
            err(code, "later", None),
            ok(disk_json("d", "n")),
        ]);
        let hits = seq.hits.clone();
        Mock::given(method("GET"))
            .and(path("/api/disks/d"))
            .respond_with(seq)
            .mount(&server)
            .await;
        let disk = client(&server)
            .disks()
            .get("d")
            .await
            .unwrap_or_else(|e| panic!("{code}: {e}"));
        assert_eq!(disk.id, "d");
        assert_eq!(hits.load(Ordering::SeqCst), 3, "status {code}");
    }
}

#[tokio::test]
async fn reads_give_up_after_three_retries_and_report_the_last_error() {
    let server = MockServer::start().await;
    let seq = Seq::new(vec![err(503, "still down", Some("UNAVAILABLE"))]);
    let hits = seq.hits.clone();
    Mock::given(method("GET"))
        .and(path("/api/disks/d"))
        .respond_with(seq)
        .mount(&server)
        .await;
    let e = client(&server).disks().get("d").await.unwrap_err();
    assert_eq!((e.status(), e.code()), (Some(503), Some("UNAVAILABLE")));
    assert_eq!(hits.load(Ordering::SeqCst), 4, "1 attempt + 3 retries");
}

#[tokio::test]
async fn client_errors_are_not_retried() {
    for code in [400, 401, 403, 404, 409, 501] {
        let server = MockServer::start().await;
        let seq = Seq::new(vec![err(code, "no", None)]);
        let hits = seq.hits.clone();
        Mock::given(method("GET"))
            .and(path("/api/disks/d"))
            .respond_with(seq)
            .mount(&server)
            .await;
        assert!(client(&server).disks().get("d").await.is_err());
        assert_eq!(hits.load(Ordering::SeqCst), 1, "status {code}");
    }
}

#[tokio::test]
async fn creations_and_deletes_do_not_retry_on_status() {
    // Replaying a create could duplicate work; replaying a delete could
    // report a lost success as a 404.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d"))
        .respond_with(ok(disk_json("d", "n")))
        .mount(&server)
        .await;
    let mut hits = Vec::new();
    for (m, p, first) in [
        ("POST", "/api/disks", err(503, "x", None)),
        ("DELETE", "/api/disks/d", err(503, "x", None)),
        ("DELETE", "/api/tokens/t", err(502, "x", None)),
        ("POST", "/api/disks/d/exec", err(504, "x", None)),
    ] {
        let seq = Seq::new(vec![first, ok(json!({}))]);
        hits.push(seq.hits.clone());
        Mock::given(method(m))
            .and(path(p))
            .respond_with(seq)
            .mount(&server)
            .await;
    }
    let c = client(&server);
    assert!(c.disks().create(CreateDiskRequest::new("x")).await.is_err());
    assert!(c.disks().get("d").await.unwrap().delete().await.is_err());
    assert!(c.tokens().delete("t").await.is_err());
    assert!(c.disks().exec("d", "true").await.is_err());
    let counts: Vec<_> = hits.iter().map(|h| h.load(Ordering::SeqCst)).collect();
    assert_eq!(counts, [1, 1, 1, 1]);
}

#[tokio::test]
async fn idempotent_writes_do_retry() {
    // PUT allowed-ips and sandbox stop are "transient" in the TS SDK.
    let server = MockServer::start().await;
    let seq = Seq::new(vec![err(503, "x", None), ok(json!({"allowedIps": []}))]);
    let hits = seq.hits.clone();
    Mock::given(method("GET"))
        .and(path("/api/disks/d"))
        .respond_with(ok(disk_json("d", "n")))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/disks/d/allowed-ips"))
        .respond_with(seq)
        .mount(&server)
        .await;
    let disk = client(&server).disks().get("d").await.unwrap();
    disk.set_allowed_ips(&[]).await.unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

/// Accepts connections, reads the request, then hangs up without answering:
/// the connection was established, so the request may have reached the server.
async fn hangup_server() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let accepts = Arc::new(AtomicUsize::new(0));
    let n = accepts.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            n.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf).await;
                // drop => connection closed with no response
            });
        }
    });
    (url, accepts)
}

fn client_for(url: &str) -> Archil {
    Archil::new(
        ClientOptions::new()
            .api_key("k")
            .region("aws-us-east-1")
            .base_url(url),
    )
    .unwrap()
}

#[tokio::test]
async fn transport_errors_after_connect_retry_reads_but_not_creations() {
    let (url, accepts) = hangup_server().await;
    let c = client_for(&url);

    let e = c.disks().get("d").await.unwrap_err();
    assert!(matches!(e, Error::Transport(_)), "{e}");
    assert_eq!(
        accepts.load(Ordering::SeqCst),
        4,
        "GET: 1 attempt + 3 retries"
    );

    accepts.store(0, Ordering::SeqCst);
    let e = c
        .disks()
        .create(CreateDiskRequest::new("x"))
        .await
        .unwrap_err();
    assert!(matches!(e, Error::Transport(_)), "{e}");
    assert_eq!(
        accepts.load(Ordering::SeqCst),
        1,
        "POST must not replay after the request was sent"
    );
}

#[tokio::test]
async fn connection_refused_is_retried_even_for_creations() {
    // Reserve a port, release it, and only start the server after the first
    // attempt has been refused. The client must keep trying.
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = probe.local_addr().unwrap();
    drop(probe);
    let c = client_for(&format!("http://{addr}"));

    let starter = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        let listener = std::net::TcpListener::bind(addr).unwrap();
        let server = MockServer::builder().listener(listener).start().await;
        Mock::given(method("POST"))
            .and(path("/api/disks"))
            .respond_with(ok(json!({"diskId": "d9"})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/disks/d9"))
            .respond_with(ok(disk_json("d9", "n")))
            .mount(&server)
            .await;
        server
    });
    let created = c.disks().create(CreateDiskRequest::new("x")).await;
    let _server = starter.await.unwrap();
    assert_eq!(created.expect("connect failures are retried").disk.id, "d9");
}
