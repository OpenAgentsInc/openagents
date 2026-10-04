//! SOCKS5 proxy: traffic reaches the network through a real proxy, and never
//! around it.

use std::time::Duration;

use platform_utils::ProxyConfig;
use tokio::sync::mpsc;

use boltz_client::api::BoltzApiClient;
use boltz_client::api::ws::SwapStatusSubscriber;

use super::setup::{proxied_config, regtest_config};
use super::socks5::{Socks5Proxy, backend_url};

fn client(config: &boltz_client::BoltzConfig) -> BoltzApiClient {
    BoltzApiClient::new(
        config,
        platform_utils::create_http_client_with_proxy(None, config.proxy.as_ref())
            .expect("http client"),
    )
}

/// A proxy pointed at a port nothing listens on. Reaching anything through it
/// must fail.
fn dead_proxy() -> ProxyConfig {
    // Port 1 is reserved and never bound by the test environment.
    ProxyConfig::new("127.0.0.1", 1)
}

/// The backend is addressed by a container name only the proxy's network can
/// resolve, so a success here cannot have come from a direct connection.
#[tokio::test]
async fn proxied_http_reaches_the_backend() {
    let proxy = Socks5Proxy::start().expect("start SOCKS5 proxy");
    let config = proxied_config(proxy.config(), &backend_url());

    let pairs = client(&config)
        .get_reverse_swap_pairs()
        .await
        .expect("reverse pairs through the proxy");
    assert!(pairs.0.contains_key("BTC"));
}

/// The same, through a proxy that demands credentials, so the RFC 1929
/// exchange is covered too.
#[tokio::test]
async fn proxied_http_authenticates_to_the_proxy() {
    let proxy = Socks5Proxy::start_with_credentials("boltz", "hunter2")
        .expect("start authenticated SOCKS5 proxy");
    let config = proxied_config(proxy.config(), &backend_url());

    client(&config)
        .get_reverse_swap_pairs()
        .await
        .expect("reverse pairs through the authenticated proxy");
}

/// The control for the two above: without the proxy, the container name does
/// not resolve. Proves those tests pass because of the tunnel, not in spite
/// of it.
#[tokio::test]
async fn the_backend_url_is_unreachable_without_the_proxy() {
    let mut config = proxied_config(dead_proxy(), &backend_url());
    config.proxy = None;

    assert!(
        client(&config).get_reverse_swap_pairs().await.is_err(),
        "the container name resolved on the host, so the proxy tests prove nothing"
    );
}

/// The WebSocket has its own dial path: `tokio-tungstenite-wasm` takes no
/// proxy, so the proxied path hand-rolls SOCKS5 plus the handshake and
/// converts messages between two crates' types. A status update arriving here
/// exercises all of it end to end.
#[tokio::test]
async fn proxied_ws_delivers_status_updates() {
    let proxy = Socks5Proxy::start().expect("start SOCKS5 proxy");
    let config = proxied_config(proxy.config(), &backend_url());
    let api = client(&config);

    let km = boltz_client::keys::EvmKeyManager::from_seed(&super::setup::regtest_seed()).unwrap();
    let pairs = api.get_reverse_swap_pairs().await.unwrap();
    let rbtc_pair = &pairs.0["BTC"]["RBTC"];
    let req = super::create_swap_request(
        &config,
        &km,
        super::next_key_index(),
        &rbtc_pair.hash,
        rbtc_pair.limits.minimal,
    );
    let swap = api.create_reverse_swap(&req).await.unwrap();

    let (ws_tx, mut rx) = mpsc::channel(32);
    let ws = SwapStatusSubscriber::connect(&config.ws_url(), ws_tx, config.proxy.clone())
        .await
        .unwrap();
    ws.subscribe(&swap.id).await.unwrap();

    let update = tokio::time::timeout(Duration::from_secs(20), rx.recv())
        .await
        .expect("timed out waiting for a WS update through the proxy")
        .expect("WS channel closed unexpectedly");

    assert_eq!(update.swap_id, swap.id);
    assert_eq!(update.status, "swap.created");

    ws.close().await;
}

/// The test that pins "fails closed". The backend is addressed at the URL that
/// *does* resolve and is reachable directly, so a success would mean the
/// client went around the unreachable proxy.
#[tokio::test]
async fn a_dead_proxy_never_falls_back_to_a_direct_connection() {
    let config = boltz_client::BoltzConfig {
        proxy: Some(dead_proxy()),
        ..regtest_config()
    };

    assert!(
        client(&config).get_reverse_swap_pairs().await.is_err(),
        "the API answered through an unreachable proxy, so the request went direct"
    );

    // Same for the WebSocket: the reader loop retries forever rather than
    // erroring, so absence of an update within the window is the assertion.
    let (ws_tx, mut rx) = mpsc::channel(32);
    let ws = SwapStatusSubscriber::connect(&config.ws_url(), ws_tx, config.proxy.clone())
        .await
        .unwrap();
    ws.subscribe("nonexistent-swap-id").await.ok();

    assert!(
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .is_err(),
        "the WebSocket delivered through an unreachable proxy, so it connected direct"
    );

    ws.close().await;
}
