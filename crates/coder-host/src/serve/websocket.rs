//! The WebSocket direct-channel listener.
//!
//! It answers each WebSocket upgrade, then serves the channel through the
//! same session as the TCP listener: `coder-reach` runs the same handshake,
//! encryption, sequencing, and frame bounds over binary messages, one frame
//! per message, and the session rechecks the grant and closes the channel
//! the same way. The upgrade must finish within the handshake timeout.

use std::net::SocketAddr;
use std::sync::Arc;

use coder_reach::channel::Acceptor;
use tokio::net::{TcpListener, TcpStream};

use super::{Shared, direct};
use crate::authority::Grants;
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

/// The hint URL for a bound listener. The listener serves every path.
pub(super) fn url(address: SocketAddr) -> String {
    format!("ws://{address}/")
}

pub(super) async fn listen(
    shared: Arc<Shared>,
    listener: TcpListener,
    acceptor: Arc<Acceptor<Grants>>,
) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        tokio::spawn(upgrade(shared.clone(), acceptor.clone(), stream));
    }
}

async fn upgrade(shared: Arc<Shared>, acceptor: Arc<Acceptor<Grants>>, stream: TcpStream) {
    let timeout = shared.config.handshake_timeout;
    let Ok(Ok(socket)) =
        tokio::time::timeout(timeout, coder_reach::websocket::accept(stream)).await
    else {
        return;
    };
    direct::session(shared, acceptor, socket).await;
}
