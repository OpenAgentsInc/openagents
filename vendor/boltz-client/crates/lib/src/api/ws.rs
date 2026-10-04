use std::collections::HashSet;
use std::pin::Pin;

use futures::{Sink, SinkExt, Stream, StreamExt, TryStreamExt};
use platform_utils::ProxyConfig;
use platform_utils::tokio;
use tokio::sync::mpsc;
use tokio_tungstenite_wasm::Message;

use crate::error::BoltzError;

use super::types::{WsMessage, WsSubscribeMessage};

/// Keep-alive ping interval to prevent idle disconnects.
const KEEP_ALIVE_INTERVAL: platform_utils::time::Duration =
    platform_utils::time::Duration::from_secs(15);

/// Delay between reconnection attempts.
const RECONNECT_DELAY: platform_utils::time::Duration =
    platform_utils::time::Duration::from_secs(5);

/// JSON-encoded ping message for the Boltz WS protocol.
const PING_JSON: &str = r#"{"op":"ping"}"#;

// The two dial paths hand back different stream types (the wrapper crate's own
// on the direct path, a raw `tokio-tungstenite` stream over a SOCKS5 tunnel on
// the proxied one), so both are boxed into one shape the reader loop can drive.
// WASM futures are not `Send`, and never take the proxied path.
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
type WsSink = Pin<Box<dyn Sink<Message, Error = BoltzError> + Send>>;
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
type WsSource = Pin<Box<dyn Stream<Item = Result<Message, BoltzError>> + Send>>;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
type WsSink = Pin<Box<dyn Sink<Message, Error = BoltzError>>>;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
type WsSource = Pin<Box<dyn Stream<Item = Result<Message, BoltzError>>>>;

/// Reject a `ws_url` that can't possibly connect — an unparseable URL, a
/// non-`ws`/`wss` scheme (e.g. an `http://` typo), or one with no host. Catches
/// the common misconfigurations at construction; a valid-but-unreachable host is
/// left to the reconnect loop (indistinguishable from a transient outage here).
fn validate_ws_url(ws_url: &str) -> Result<(), BoltzError> {
    let url = url::Url::parse(ws_url)
        .map_err(|e| BoltzError::WebSocket(format!("Invalid ws_url '{ws_url}': {e}")))?;
    if !matches!(url.scheme(), "ws" | "wss") {
        return Err(BoltzError::WebSocket(format!(
            "ws_url must use a ws:// or wss:// scheme, got '{}://'",
            url.scheme()
        )));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(BoltzError::WebSocket(format!(
            "ws_url '{ws_url}' has no host"
        )));
    }
    Ok(())
}

/// Swap status update dispatched from the WebSocket.
#[derive(Debug, Clone)]
pub struct SwapStatusUpdate {
    pub swap_id: String,
    pub status: String,
    pub failure_reason: Option<String>,
    pub transaction: Option<super::types::SwapTransaction>,
}

/// Commands sent to the reader loop.
enum ReaderCommand {
    Subscribe(String),
    Unsubscribe(String),
    Shutdown,
}

/// WebSocket subscriber for Boltz swap status updates.
///
/// All updates are dispatched through a single global channel (provided at
/// construction). Callers use `subscribe()`/`unsubscribe()` to control which
/// swap IDs the subscriber tracks; status updates for all tracked swaps flow
/// through the same channel.
pub struct SwapStatusSubscriber {
    cmd_tx: mpsc::Sender<ReaderCommand>,
    /// Sync-safe handle used by `Drop` to abort the reader task if `close()`
    /// was never called.
    abort_handle: tokio::task::AbortHandle,
}

impl SwapStatusSubscriber {
    #[expect(clippy::unused_async)]
    pub async fn connect(
        ws_url: &str,
        global_tx: mpsc::Sender<SwapStatusUpdate>,
        proxy: Option<ProxyConfig>,
    ) -> Result<Self, BoltzError> {
        // The reader loop reconnects resiliently, so we don't dial here — but
        // that means a misconfigured `ws_url` would otherwise degrade silently
        // into an endless reconnect loop. Validate the URL up front so a bad
        // value is surfaced at construction instead. (Reachability is left to
        // the loop, as it can't be told apart from a transient outage.)
        validate_ws_url(ws_url)?;

        let (cmd_tx, cmd_rx) = mpsc::channel(16);

        let reader_handle = tokio::spawn(Self::reader_loop(
            ws_url.to_string(),
            global_tx,
            cmd_rx,
            proxy,
        ));
        let abort_handle = reader_handle.abort_handle();

        Ok(Self {
            cmd_tx,
            abort_handle,
        })
    }

    /// Start tracking a swap ID. Status updates will be sent through the
    /// global channel provided at construction.
    pub async fn subscribe(&self, swap_id: &str) -> Result<(), BoltzError> {
        self.cmd_tx
            .send(ReaderCommand::Subscribe(swap_id.to_string()))
            .await
            .map_err(|_| {
                BoltzError::WebSocket("Reader loop is not running, subscribe failed".into())
            })?;

        tracing::info!(swap_id, "Subscribed to swap status updates");
        Ok(())
    }

    /// Stop tracking a swap ID.
    pub async fn unsubscribe(&self, swap_id: &str) {
        if self
            .cmd_tx
            .send(ReaderCommand::Unsubscribe(swap_id.to_string()))
            .await
            .is_err()
        {
            tracing::warn!(
                swap_id,
                "Reader loop is not running, unsubscribe not delivered"
            );
        }

        tracing::info!(swap_id, "Unsubscribed from swap status updates");
    }

    pub async fn close(&self) {
        if self.cmd_tx.send(ReaderCommand::Shutdown).await.is_err() {
            tracing::warn!("Reader loop is not running, shutdown not delivered");
        }
        tracing::info!("WebSocket subscriber closed");
    }
}

impl Drop for SwapStatusSubscriber {
    fn drop(&mut self) {
        self.abort_handle.abort();
    }
}

impl SwapStatusSubscriber {
    async fn reader_loop(
        ws_url: String,
        global_tx: mpsc::Sender<SwapStatusUpdate>,
        mut cmd_rx: mpsc::Receiver<ReaderCommand>,
        proxy: Option<ProxyConfig>,
    ) {
        // The reader owns the authoritative set of subscribed IDs, driven by the
        // ordered `ReaderCommand` stream from subscribe/unsubscribe/close. Kept
        // local to the loop so resubscription on reconnect needs no shared lock
        // held across I/O.
        let mut local_ids: HashSet<String> = HashSet::new();

        loop {
            let connection = match Self::try_connect(&ws_url, proxy.as_ref()).await {
                Ok(connection) => connection,
                Err(e) => {
                    tracing::warn!("WebSocket connection failed: {e}, retrying in 5s");
                    tokio::select! {
                        () = tokio::time::sleep(RECONNECT_DELAY) => continue,
                        cmd = cmd_rx.recv() => {
                            match cmd {
                                Some(ReaderCommand::Subscribe(id)) => { local_ids.insert(id); }
                                Some(ReaderCommand::Unsubscribe(id)) => { local_ids.remove(&id); }
                                Some(ReaderCommand::Shutdown) | None => return,
                            }
                            continue;
                        }
                    }
                }
            };

            tracing::info!("WebSocket connected to {ws_url}");
            let (mut write, mut read) = connection;

            // Drain pending commands before resubscribing.
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    ReaderCommand::Subscribe(id) => {
                        local_ids.insert(id);
                    }
                    ReaderCommand::Unsubscribe(id) => {
                        local_ids.remove(&id);
                    }
                    ReaderCommand::Shutdown => return,
                }
            }

            // Re-subscribe all tracked IDs after (re)connect. We only
            // resubscribe — we do NOT re-fetch status — so a `swap.update`
            // emitted during the disconnect window is seen again only if Boltz
            // re-pushes the current status on resubscribe. A money-critical
            // transition missed that way is still recovered by the store-driven
            // `poll_pending_swaps` (Settling/Claiming) or a restart's
            // `resume_all`; accepted as a deliberate trade-off over polling every
            // tracked swap's status on each reconnect.
            if !local_ids.is_empty() {
                let ids: Vec<String> = local_ids.iter().cloned().collect();
                let msg = WsSubscribeMessage::subscribe(ids);
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = write.send(Message::Text(json.into())).await;
                }
            }

            // Read loop — also listens for new commands and sends keep-alive pings.
            let should_shutdown = loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                Self::handle_message(&text, &global_tx).await;
                            }
                            Some(Ok(Message::Binary(data))) => {
                                if let Ok(text) = String::from_utf8(data.to_vec()) {
                                    Self::handle_message(&text, &global_tx).await;
                                }
                            }
                            Some(Ok(Message::Close(_)) | Err(_)) | None => {
                                tracing::info!("WebSocket disconnected, reconnecting");
                                break false;
                            }
                        }
                    }
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(ReaderCommand::Subscribe(id)) => {
                                local_ids.insert(id.clone());
                                let msg = WsSubscribeMessage::subscribe(vec![id]);
                                if let Ok(json) = serde_json::to_string(&msg)
                                    && let Err(e) = write.send(Message::Text(json.into())).await
                                {
                                    tracing::warn!("Failed to send subscribe: {e}");
                                    break false; // Reconnect
                                }
                            }
                            Some(ReaderCommand::Unsubscribe(id)) => {
                                local_ids.remove(&id);
                                // No need to send an unsubscribe to Boltz WS —
                                // we simply stop caring about updates for this ID.
                            }
                            Some(ReaderCommand::Shutdown) | None => break true,
                        }
                    }
                    () = tokio::time::sleep(KEEP_ALIVE_INTERVAL) => {
                        if let Err(e) = write.send(Message::Text(PING_JSON.into())).await {
                            tracing::warn!("Failed to send keep-alive ping: {e}");
                            break false; // Reconnect
                        }
                    }
                }
            };

            if should_shutdown {
                return;
            }

            // Wait before reconnecting.
            tokio::select! {
                () = tokio::time::sleep(RECONNECT_DELAY) => {}
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(ReaderCommand::Subscribe(id)) => { local_ids.insert(id); }
                        Some(ReaderCommand::Unsubscribe(id)) => { local_ids.remove(&id); }
                        Some(ReaderCommand::Shutdown) | None => return,
                    }
                }
            }
        }
    }

    // ─── Shared helpers ──────────────────────────────────────────────

    async fn try_connect(
        url: &str,
        proxy: Option<&ProxyConfig>,
    ) -> Result<(WsSink, WsSource), BoltzError> {
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        if let Some(proxy) = proxy {
            return connect_via_proxy(url, proxy).await;
        }
        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        if proxy.is_some() {
            return Err(BoltzError::WebSocket(
                "a SOCKS5 proxy cannot be honoured on WASM: the browser owns connection setup"
                    .to_string(),
            ));
        }

        let stream = tokio_tungstenite_wasm::connect(url)
            .await
            .map_err(|e| BoltzError::WebSocket(format!("Connection failed: {e}")))?;
        let (write, read) = stream.split();
        Ok((
            Box::pin(write.sink_map_err(|e| BoltzError::WebSocket(e.to_string()))),
            Box::pin(read.map_err(|e| BoltzError::WebSocket(e.to_string()))),
        ))
    }

    async fn handle_message(text: &str, global_tx: &mpsc::Sender<SwapStatusUpdate>) {
        // Filter out control messages that don't fit the typed `WsMessage`
        // shape — subscribe/unsubscribe acks carry `args` as an array of
        // swap-id strings (not `WsSwapUpdate`s) and would otherwise noise
        // the debug log on every subscription.
        #[derive(serde::Deserialize)]
        struct WsEnvelope {
            #[serde(default)]
            event: Option<String>,
        }
        if let Ok(env) = serde_json::from_str::<WsEnvelope>(text)
            && matches!(
                env.event.as_deref(),
                Some("ping" | "pong" | "subscribe" | "unsubscribe")
            )
        {
            return;
        }

        let msg: WsMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!("Failed to parse WS message: {e}");
                return;
            }
        };

        if msg.channel.as_deref() != Some("swap.update") {
            return;
        }

        if let Some(args) = msg.args {
            for update in args {
                let status_update = SwapStatusUpdate {
                    swap_id: update.id.clone(),
                    status: update.status,
                    failure_reason: update.failure_reason,
                    transaction: update.transaction,
                };

                if global_tx.send(status_update).await.is_err() {
                    tracing::error!(
                        swap_id = update.id,
                        "Global receiver dropped, update discarded"
                    );
                }
            }
        }
    }
}

/// Dials `url` through `proxy` and hands back the same boxed pair the direct
/// path produces.
///
/// `tokio-tungstenite-wasm` owns its socket and exposes no proxy knob, so the
/// proxied path drops to `tokio-tungstenite` directly: SOCKS5 first, then the
/// WebSocket (and TLS, for `wss`) handshake on top of the tunnel. The target
/// host is sent to the proxy as a name, so only the proxy's own address is
/// resolved locally.
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
async fn connect_via_proxy(
    url: &str,
    proxy: &ProxyConfig,
) -> Result<(WsSink, WsSource), BoltzError> {
    use tokio_socks::tcp::Socks5Stream;
    use tokio_tungstenite::tungstenite::Message as TgMessage;

    let parsed = url::Url::parse(url)
        .map_err(|e| BoltzError::WebSocket(format!("Invalid ws_url '{url}': {e}")))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| BoltzError::WebSocket(format!("ws_url '{url}' has no host")))?
        .to_string();
    let port = parsed
        .port()
        .unwrap_or(if parsed.scheme() == "wss" { 443 } else { 80 });

    let proxy_addr = proxy.address();
    // `(String, u16)` becomes a `TargetAddr::Domain`, which is what makes the
    // proxy resolve the name instead of this process.
    let target = (host.clone(), port);
    let socks = match proxy.credentials() {
        Some((user, pass)) => {
            Socks5Stream::connect_with_password(proxy_addr.as_str(), target, user, pass).await
        }
        None => Socks5Stream::connect(proxy_addr.as_str(), target).await,
    }
    .map_err(|e| {
        BoltzError::WebSocket(format!(
            "SOCKS5 connection to {host}:{port} via {proxy_addr} failed: {e}"
        ))
    })?;

    let (stream, _response) =
        tokio_tungstenite::client_async_tls_with_config(url, socks.into_inner(), None, None)
            .await
            .map_err(|e| BoltzError::WebSocket(format!("Connection failed: {e}")))?;
    let (write, read) = stream.split();

    let write = write
        .sink_map_err(|e| BoltzError::WebSocket(e.to_string()))
        .with(|msg: Message| std::future::ready(Ok::<_, BoltzError>(TgMessage::from(msg))));
    let read = read.filter_map(|item| {
        std::future::ready(match item {
            // The wrapper's `Message` has no Ping/Pong/Frame variant and its
            // `From` impl panics on one, so drop them here the way the wrapper
            // does. tungstenite answers pings itself.
            Ok(TgMessage::Ping(_) | TgMessage::Pong(_) | TgMessage::Frame(_)) => None,
            Ok(msg) => Some(Ok(Message::from(msg))),
            Err(e) => Some(Err(BoltzError::WebSocket(e.to_string()))),
        })
    });

    Ok((Box::pin(write), Box::pin(read)))
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;

    #[macros::test_all]
    fn validate_ws_url_accepts_ws_and_wss() {
        assert!(validate_ws_url("wss://api.boltz.exchange/v2/ws").is_ok());
        assert!(validate_ws_url("ws://localhost:9001").is_ok());
        // Scheme is case-insensitive per the URL spec — must not false-reject.
        assert!(validate_ws_url("WSS://api.boltz.exchange/ws").is_ok());
    }

    #[macros::test_all]
    fn validate_ws_url_rejects_bad_scheme_and_malformed() {
        // Wrong scheme (the classic http/ws mix-up).
        assert!(validate_ws_url("https://api.boltz.exchange").is_err());
        assert!(validate_ws_url("http://api.boltz.exchange").is_err());
        // Unparseable / no scheme / empty.
        assert!(validate_ws_url("not a url").is_err());
        assert!(validate_ws_url("api.boltz.exchange/ws").is_err());
        assert!(validate_ws_url("").is_err());
        // Right scheme but no host.
        assert!(validate_ws_url("wss://").is_err());
    }

    #[macros::test_all]
    fn test_swap_status_update_clone() {
        let update = SwapStatusUpdate {
            swap_id: "test".to_string(),
            status: "transaction.confirmed".to_string(),
            failure_reason: None,
            transaction: None,
        };
        let cloned = update.clone();
        assert_eq!(cloned.swap_id, "test");
        assert_eq!(cloned.status, "transaction.confirmed");
    }

    #[macros::async_test_all]
    async fn test_handle_message_control_events_ignored() {
        let (tx, mut rx) = mpsc::channel(32);
        SwapStatusSubscriber::handle_message(r#"{"event":"ping"}"#, &tx).await;
        SwapStatusSubscriber::handle_message(r#"{"event":"pong"}"#, &tx).await;
        // Subscribe/unsubscribe acks carry `args` as string arrays (swap IDs),
        // which would fail typed parsing — the envelope peek must drop them.
        SwapStatusSubscriber::handle_message(
            r#"{"event":"subscribe","channel":"swap.update","args":["swap1"]}"#,
            &tx,
        )
        .await;
        SwapStatusSubscriber::handle_message(
            r#"{"event":"unsubscribe","channel":"swap.update","args":["swap1"]}"#,
            &tx,
        )
        .await;
        assert!(rx.try_recv().is_err());
    }

    #[macros::async_test_all]
    async fn test_handle_message_dispatches_update() {
        let (tx, mut rx) = mpsc::channel(32);

        let msg = r#"{
            "event": "update",
            "channel": "swap.update",
            "args": [{
                "id": "swap123",
                "status": "transaction.confirmed",
                "transaction": { "id": "0xabc", "hex": "0xdef" }
            }]
        }"#;

        SwapStatusSubscriber::handle_message(msg, &tx).await;

        let update = rx.recv().await.unwrap();
        assert_eq!(update.swap_id, "swap123");
        assert_eq!(update.status, "transaction.confirmed");
        assert!(update.transaction.is_some());
    }

    #[macros::async_test_all]
    async fn test_handle_message_wrong_channel_ignored() {
        let (tx, mut rx) = mpsc::channel(32);

        let msg = r#"{
            "channel": "some.other.channel",
            "args": [{
                "id": "swap1",
                "status": "transaction.confirmed"
            }]
        }"#;

        SwapStatusSubscriber::handle_message(msg, &tx).await;
        assert!(rx.try_recv().is_err());
    }
}
