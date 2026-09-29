//! The NIP-REACH direct channel on the `openagents/reach/1` ALPN.
//!
//! The handshake, frames, bounds, and grant check are `coder_reach::channel`
//! unchanged; this module only carries them over one QUIC bidirectional
//! stream. The dialing device opens the stream and sends its hello; the host
//! accepts the first stream on the connection. An iroh `EndpointId` proves
//! nothing here beyond the route: a host that cannot prove the Nostr key the
//! device expects is refused, and a device whose grant the host's store
//! refuses gets that refusal in the verdict.

use std::sync::Arc;

use coder_reach::Refusal;
use coder_reach::channel::{Acceptor, Channel, ClientConfig, GrantCheck};
use iroh::protocol::{AcceptError, ProtocolHandler};
use iroh::{EndpointAddr, EndpointId};
use tokio::sync::mpsc;

use crate::REACH_ALPN;
use crate::endpoint::ConnectEndpoint;
use crate::stream::IrohStream;

/// A direct channel over iroh.
pub type IrohChannel = Channel<IrohStream>;

/// Dial `addr` on the reach ALPN and complete the client handshake.
///
/// # Errors
/// `unavailable` when the endpoint cannot be reached or the stream cannot
/// open; otherwise the handshake's own refusal, such as
/// `identity_mismatch` for a host that proves another key.
pub async fn dial(
    endpoint: &ConnectEndpoint,
    addr: impl Into<EndpointAddr>,
    config: &ClientConfig,
    now: u64,
) -> coder_reach::Result<IrohChannel> {
    let connect = endpoint.endpoint.connect(addr, REACH_ALPN);
    let connection = tokio::time::timeout(config.timeout, connect)
        .await
        .map_err(|_| unavailable("iroh connection timed out"))?
        .map_err(|_| unavailable("iroh connection failed"))?;
    let stream = IrohStream::open(connection)
        .await
        .map_err(|_| unavailable("could not open a stream"))?;
    coder_reach::channel::connect(stream, config, now).await
}

/// Accept the first stream on an incoming reach connection and complete the
/// host handshake.
///
/// # Errors
/// `unavailable` when no stream arrives; otherwise the handshake's refusal,
/// which the host has already sent to the device.
pub async fn accept<G: GrantCheck>(
    acceptor: &Acceptor<G>,
    connection: iroh::endpoint::Connection,
    now: u64,
) -> coder_reach::Result<IrohChannel> {
    let stream = IrohStream::accept(connection)
        .await
        .map_err(|_| unavailable("no stream arrived"))?;
    acceptor.accept(stream, now).await
}

/// An admitted channel handed to the host.
#[derive(Debug)]
pub struct ReachSession {
    /// The device's iroh key; the channel's binding names its Nostr key.
    pub remote: EndpointId,
    pub channel: IrohChannel,
}

/// Seconds since the Unix epoch; injected so tests can fix the clock.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// A router handler for [`REACH_ALPN`]: it runs the host handshake on each
/// connection and sends each admitted channel to `sessions`. Refused
/// handshakes end there; the device already has the refusal.
pub struct ReachProtocol<G> {
    acceptor: Arc<Acceptor<G>>,
    sessions: mpsc::Sender<ReachSession>,
    clock: Clock,
}

impl<G> ReachProtocol<G> {
    #[must_use]
    pub fn new(acceptor: Arc<Acceptor<G>>, sessions: mpsc::Sender<ReachSession>) -> Self {
        Self::with_clock(acceptor, sessions, Arc::new(crate::now))
    }

    #[must_use]
    pub fn with_clock(
        acceptor: Arc<Acceptor<G>>,
        sessions: mpsc::Sender<ReachSession>,
        clock: Clock,
    ) -> Self {
        Self {
            acceptor,
            sessions,
            clock,
        }
    }
}

impl<G> std::fmt::Debug for ReachProtocol<G> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReachProtocol").finish_non_exhaustive()
    }
}

impl<G: GrantCheck + 'static> ProtocolHandler for ReachProtocol<G> {
    async fn accept(&self, connection: iroh::endpoint::Connection) -> Result<(), AcceptError> {
        let remote = connection.remote_id();
        let channel = accept(&self.acceptor, connection, (self.clock)())
            .await
            .map_err(AcceptError::from_err)?;
        self.sessions
            .send(ReachSession { remote, channel })
            .await
            .map_err(|_| AcceptError::from_err(unavailable("the host stopped taking channels")))
    }
}

fn unavailable(detail: &'static str) -> coder_reach::Error {
    coder_reach::Error::new(Refusal::Unavailable, detail)
}
