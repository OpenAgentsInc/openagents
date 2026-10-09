//! The WebSocket direct-channel listener.
//!
//! It answers each WebSocket upgrade, then serves the channel through the
//! same session as the TCP listener: `coder-reach` runs the same handshake,
//! encryption, sequencing, and frame bounds over binary messages, one frame
//! per binary message, and the session rechecks the grant and closes the
//! channel the same way. With TLS configured, the listener completes a TLS
//! handshake first and serves `wss`; otherwise it serves plain `ws`. The TLS
//! handshake and the upgrade must both finish within the handshake timeout.

use std::net::SocketAddr;
use std::sync::Arc;

use coder_reach::channel::Acceptor;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

use super::{Shared, direct};
use crate::authority::Grants;
use crate::config::WebsocketTls;
use crate::{Error, Result};

/// Bind the WebSocket listener and return it with its bound address.
pub(super) async fn bind(address: SocketAddr) -> Result<(TcpListener, SocketAddr)> {
    let listener = TcpListener::bind(address)
        .await
        .map_err(|_| Error::Config("the WebSocket listener cannot bind".into()))?;
    let bound = listener
        .local_addr()
        .map_err(|_| Error::Config("the WebSocket listener has no local address".into()))?;
    Ok((listener, bound))
}

/// The hint URL for a bound listener: `ws://ADDR/` for plain `ws`, and
/// `wss://NAME:PORT/` with TLS, because a certificate names the host by DNS
/// name. The listener serves every path.
pub(super) fn url(address: SocketAddr, tls: Option<&WebsocketTls>) -> String {
    match tls {
        Some(tls) => format!("wss://{}:{}/", tls.name, address.port()),
        None => format!("ws://{address}/"),
    }
}

pub(super) async fn listen(
    shared: Arc<Shared>,
    listener: TcpListener,
    acceptor: Arc<Acceptor<Grants>>,
    tls: Option<TlsAcceptor>,
) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let _ = stream.set_nodelay(true);
        tokio::spawn(upgrade(
            shared.clone(),
            acceptor.clone(),
            tls.clone(),
            stream,
        ));
    }
}

async fn upgrade(
    shared: Arc<Shared>,
    acceptor: Arc<Acceptor<Grants>>,
    tls: Option<TlsAcceptor>,
    stream: TcpStream,
) {
    let timeout = shared.config.handshake_timeout;
    let deadline = tokio::time::Instant::now() + timeout;
    match tls {
        Some(tls) => {
            let Ok(Ok(stream)) = tokio::time::timeout_at(deadline, tls.accept(stream)).await else {
                return;
            };
            let Ok(Ok(socket)) =
                tokio::time::timeout_at(deadline, coder_reach::websocket::accept(stream)).await
            else {
                return;
            };
            direct::session(shared, acceptor, socket).await;
        }
        None => {
            let Ok(Ok(socket)) =
                tokio::time::timeout_at(deadline, coder_reach::websocket::accept(stream)).await
            else {
                return;
            };
            direct::session(shared, acceptor, socket).await;
        }
    }
}
