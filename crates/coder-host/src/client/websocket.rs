//! Dialing a `websocket` hint.
//!
//! A `ws` URL opens a plain TCP connection and a `wss` URL a TLS one, then
//! upgrades it with the direct channel's WebSocket limits. The channel
//! authenticates and encrypts itself either way; TLS lets it pass through a
//! TLS-terminating forwarder.

use std::time::Duration;

use coder_reach::websocket::{self, WebSocket};
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;

/// A WebSocket direct-channel stream, plain or TLS.
pub type Stream = WebSocket<MaybeTlsStream<TcpStream>>;

/// Connect and upgrade within `timeout`; `None` if the route does not answer.
pub(super) async fn dial(url: &str, timeout: Duration) -> Option<Stream> {
    let upgrade =
        tokio_tungstenite::connect_async_with_config(url, Some(websocket::config()), false);
    let (socket, _) = tokio::time::timeout(timeout, upgrade).await.ok()?.ok()?;
    Some(WebSocket::new(socket))
}
