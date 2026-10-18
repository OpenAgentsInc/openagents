//! Direct peer routes (#12130): with the sync device room answering 429, a
//! peer call reaches the other computer over its direct iroh route, the
//! route survives a restart, and a direct dial is admitted only with a ticket
//! the host issued to this device and account.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use openagents_connect::endpoint::Relay;
use openagents_engine::peer_direct::{
    AccountProbe, DirectHost, DirectHostConfig, DirectPeers, DirectPeersConfig,
};
use openagents_rpc::{
    DirectRoutes, LinkCache, LinkCacheConfig, LinkRoute, RpcError, RpcReply, RpcService,
    STATUS_CONNECTED_DIRECTLY, StaticToken, memory_client, methods,
};

const HOST: &str = "host-device";
const CALLER: &str = "caller-device";

struct Service;

#[async_trait]
impl RpcService for Service {
    async fn handle(&self, method: &str, params: serde_json::Value) -> Result<RpcReply, RpcError> {
        match method {
            methods::LIST_HARNESSES => Ok(RpcReply::Value(serde_json::json!([]))),
            "Echo" => Ok(RpcReply::Value(
                serde_json::json!({ "host": HOST, "params": params }),
            )),
            _ => Err(RpcError::UnknownMethod(method.into())),
        }
    }
}

fn account(value: &Arc<Mutex<Option<String>>>) -> AccountProbe {
    let value = value.clone();
    Arc::new(move || value.lock().expect("lock").clone())
}

/// A device room that refuses every socket with 429, as the sync server did
/// during the #12128 outage.
async fn refusing_room() -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let dials = Arc::new(AtomicUsize::new(0));
    let counter = dials.clone();
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            counter.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request).await;
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 429 Too Many Requests\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                    )
                    .await;
            });
        }
    });
    (format!("http://127.0.0.1:{port}"), dials, task)
}

fn peers(dir: &std::path::Path, account: AccountProbe) -> Arc<DirectPeers> {
    let mut config = DirectPeersConfig::new(dir, CALLER, account);
    config.relay = Relay::Disabled;
    DirectPeers::open(config)
}

fn links(edge: &str, peers: &Arc<DirectPeers>) -> Arc<LinkCache> {
    let mut config = LinkCacheConfig::new(edge, Arc::new(StaticToken("test-user".into())));
    config.probe_timeout = Duration::from_secs(5);
    config.direct = Some(peers.clone() as Arc<dyn DirectRoutes>);
    LinkCache::new(config)
}

struct Host {
    host: Arc<DirectHost>,
    account: Arc<Mutex<Option<String>>>,
    _dir: tempfile::TempDir,
}

async fn host() -> Host {
    let dir = tempfile::tempdir().expect("tempdir");
    let signed_in = Arc::new(Mutex::new(Some("user-1".to_string())));
    let host = DirectHost::start(
        DirectHostConfig::loopback(dir.path()),
        account(&signed_in),
        Arc::new(StaticToken("host-token".into())),
    )
    .await
    .expect("direct host binds");
    let service = host.wrap(Arc::new(Service));
    host.serve(service);
    Host {
        host,
        account: signed_in,
        _dir: dir,
    }
}

/// Learn the host's route the way a live room link does: by calling
/// `PeerDirectRoute` on the host's (wrapped) RPC surface.
async fn learn(host: &Host, peers: &DirectPeers) {
    let client = memory_client(host.host.wrap(Arc::new(Service)));
    peers.refresh(HOST, &client).await.expect("route issued");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_429_room_falls_back_to_the_direct_route_and_the_route_survives_a_restart() {
    let host = host().await;
    let caller_dir = tempfile::tempdir().expect("tempdir");
    let caller_account = Arc::new(Mutex::new(Some("user-1".to_string())));
    let (edge, room_dials, _room) = refusing_room().await;

    let first = peers(caller_dir.path(), account(&caller_account));
    learn(&host, &first).await;
    let route = first.route(HOST).expect("route cached");
    assert!(!route.addrs.is_empty(), "the host publishes its addresses");

    let cache = links(&edge, &first);
    let client = cache.client(HOST).await.expect("connected directly");
    let reply = client
        .call("Echo", serde_json::json!({ "n": 1 }))
        .await
        .expect("echo over the direct route");
    assert_eq!(reply["host"], HOST);
    assert_eq!(cache.route(HOST), Some(LinkRoute::Direct));
    assert_eq!(cache.status(HOST), Some(STATUS_CONNECTED_DIRECTLY));
    assert_eq!(
        room_dials.load(Ordering::SeqCst),
        1,
        "one refusal, then direct"
    );
    drop(client);
    drop(cache);
    drop(first);

    // A restart: the route comes back from disk while sync is still down.
    let second = peers(caller_dir.path(), account(&caller_account));
    assert!(second.knows(HOST), "the cached route survives a restart");
    let cache = links(&edge, &second);
    let client = cache.client(HOST).await.expect("direct after restart");
    let reply = client
        .call("Echo", serde_json::json!({}))
        .await
        .expect("echo after restart");
    assert_eq!(reply["host"], HOST);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_direct_dial_needs_the_ticket_the_host_issued() {
    let host = host().await;
    let caller_dir = tempfile::tempdir().expect("tempdir");
    let caller_account = Arc::new(Mutex::new(Some("user-1".to_string())));
    let peers = peers(caller_dir.path(), account(&caller_account));
    learn(&host, &peers).await;

    // A forged ticket is refused, and the caller stops dialing it.
    let mut forged = peers.route(HOST).expect("route");
    forged.ticket = "0".repeat(64);
    peers.remember(HOST, forged);
    let Err(error) = peers.dial(HOST).await else {
        panic!("a forged ticket must not be admitted")
    };
    assert!(
        error.to_string().contains("refused (ticket)"),
        "got: {error}"
    );
    assert!(!peers.knows(HOST), "a refused route is forgotten");

    // A real ticket is admitted.
    learn(&host, &peers).await;
    peers
        .dial(HOST)
        .await
        .expect("the issued ticket is admitted");

    // The host signs out: no direct dial is admitted, ticket or not.
    *host.account.lock().expect("lock") = None;
    let Err(error) = peers.dial(HOST).await else {
        panic!("a signed-out host must refuse")
    };
    assert!(error.to_string().contains("signed_out"), "got: {error}");

    // Signed back in to another account: the old ticket no longer admits.
    *host.account.lock().expect("lock") = Some("user-2".into());
    let Err(error) = peers.dial(HOST).await else {
        panic!("another account's ticket must not be admitted")
    };
    assert!(
        error.to_string().contains("refused (ticket)"),
        "got: {error}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn routes_belong_to_the_account_that_learned_them() {
    let host = host().await;
    let caller_dir = tempfile::tempdir().expect("tempdir");
    let caller_account = Arc::new(Mutex::new(Some("user-1".to_string())));
    let peers = peers(caller_dir.path(), account(&caller_account));
    learn(&host, &peers).await;
    assert!(peers.knows(HOST));
    *caller_account.lock().expect("lock") = Some("user-2".into());
    assert!(!peers.knows(HOST), "another account doesn't see the route");
    *caller_account.lock().expect("lock") = None;
    assert!(!peers.knows(HOST), "a signed-out computer dials no one");
}
