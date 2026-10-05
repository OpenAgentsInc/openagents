//! The chamber over a NIP-REACH direct channel.
//!
//! The channel proves the host key and the device key, binds the device's
//! NIP-HOST grant and epoch, and encrypts every frame, so the chamber needs
//! no TLS certificate of its own. The chamber's length-prefixed frames and
//! wire types pass through unchanged: each end sees the channel as one
//! ordered byte stream, split into data frames of at most
//! [`MAX_DATA_BYTES`].
//!
//! Admission comes from the grant. The host's [`GrantCheck`] must admit only
//! a current, unrevoked grant at its epoch that holds the `world` right
//! (`coder_host::authority::WorldGrants` is that check over a Coder host's
//! access store). The host asks it at the handshake, before every chamber
//! request, and on a timer, and closes the connection when it refuses. A
//! channel carries only its own device's requests: an `authenticate` request
//! must name the channel's device key. A granted key the chamber has not
//! enrolled joins as a spectator; the host's role table still assigns
//! adventurers.
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use coder_reach::channel::{Acceptor, Binding, Channel, MAX_DATA_BYTES, connect};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    time::timeout,
};

pub use coder_reach::channel::{ClientConfig, GrantCheck, GrantRefusal};
pub use coder_reach::websocket;

use super::{
    auth::Gateway,
    client::Client,
    net::{self, Event, Exit, Guard, Listen},
    persistence::Store,
    wire::{Body, Request},
};

/// How often an open connection's grant is rechecked while it sends nothing.
pub const RECHECK: Duration = Duration::from_secs(5);
const HANDSHAKE: Duration = Duration::from_secs(5);
const BUFFER: usize = 256 * 1024;

/// The byte stream a direct channel runs over.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Carrier {
    /// A TCP connection.
    #[default]
    Tcp,
    /// A WebSocket upgrade on a TCP connection, for browsers.
    WebSocket,
}

/// A host's side of world channels: its key, generation, and grant check.
pub struct Server<G> {
    acceptor: Acceptor<Shared<G>>,
    grants: Arc<G>,
    carrier: Carrier,
    recheck: Duration,
}

struct Shared<G>(Arc<G>);
impl<G: GrantCheck> GrantCheck for Shared<G> {
    fn check(&self, device: &str, grant: &str, epoch: u64, now: u64) -> Result<(), GrantRefusal> {
        self.0.check(device, grant, epoch, now)
    }
}

impl<G: GrantCheck + 'static> Server<G> {
    /// `grants` must refuse any grant that lacks the `world` right.
    pub fn new(host: SecretKey, generation: u64, grants: G, carrier: Carrier) -> Self {
        let grants = Arc::new(grants);
        Self {
            acceptor: Acceptor::new(host, generation, Shared(grants.clone()), HANDSHAKE),
            grants,
            carrier,
            recheck: RECHECK,
        }
    }
    /// Sets how often an idle connection's grant is rechecked.
    pub fn with_recheck(mut self, every: Duration) -> Self {
        self.recheck = every;
        self
    }
    /// The host key the channel proves.
    pub fn host_key(&self) -> &str {
        self.acceptor.host_key()
    }
    async fn accept(&self, socket: TcpStream) -> Result<(DuplexStream, Box<dyn Guard>), String> {
        socket
            .set_nodelay(true)
            .map_err(|_| "Cannot configure chamber socket")?;
        let now = unix_now()?;
        let refused = |error: coder_reach::Error| format!("Chamber channel refused: {error}");
        let (stream, binding) = match self.carrier {
            Carrier::Tcp => {
                let channel = self.acceptor.accept(socket, now).await.map_err(refused)?;
                let binding = channel.binding().clone();
                (self.bridge(channel, &binding), binding)
            }
            Carrier::WebSocket => {
                let socket = timeout(HANDSHAKE, websocket::accept(socket))
                    .await
                    .map_err(|_| "Chamber WebSocket upgrade timed out")?
                    .map_err(refused)?;
                let channel = self.acceptor.accept(socket, now).await.map_err(refused)?;
                let binding = channel.binding().clone();
                (self.bridge(channel, &binding), binding)
            }
        };
        let device = key_bytes(&binding.client)?;
        let guard = ChannelGuard {
            grants: self.grants.clone(),
            binding,
            device,
        };
        Ok((stream, Box::new(guard)))
    }
    fn bridge<S>(&self, channel: Channel<S>, binding: &Binding) -> DuplexStream
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let grants = self.grants.clone();
        let binding = binding.clone();
        let admitted = move || check(grants.as_ref(), &binding).is_ok();
        bridge(channel, Some((Box::new(admitted), self.recheck)))
    }
}

/// Opens one world connection; the network loop holds it type-erased.
pub(super) trait Admit: Send + Sync {
    fn open(
        self: Arc<Self>,
        socket: TcpStream,
    ) -> Pin<Box<dyn Future<Output = Result<(DuplexStream, Box<dyn Guard>), String>> + Send>>;
}
impl<G: GrantCheck + 'static> Admit for Server<G> {
    fn open(
        self: Arc<Self>,
        socket: TcpStream,
    ) -> Pin<Box<dyn Future<Output = Result<(DuplexStream, Box<dyn Guard>), String>> + Send>> {
        Box::pin(async move { self.accept(socket).await })
    }
}

pub(super) async fn connection(
    socket: TcpStream,
    admit: Arc<dyn Admit>,
    send: mpsc::Sender<Event>,
) -> Result<(), String> {
    let (stream, guard) = admit.open(socket).await?;
    net::session(stream, Some(guard), send).await
}

struct ChannelGuard<G> {
    grants: Arc<G>,
    binding: Binding,
    device: [u8; 32],
}
impl<G: GrantCheck> Guard for ChannelGuard<G> {
    fn admit(&self, request: &[u8]) -> Result<(), String> {
        check(self.grants.as_ref(), &self.binding)?;
        if let Ok(Request {
            body: Body::Authenticate { public_key, .. },
            ..
        }) = Request::decode(request)
            && public_key != self.device
        {
            return Err("Chamber channel carries only its own device's key".into());
        }
        Ok(())
    }
    fn device(&self) -> Option<[u8; 32]> {
        Some(self.device)
    }
}

/// The channel's grant, at its epoch, must still admit it.
fn check<G: GrantCheck + ?Sized>(grants: &G, binding: &Binding) -> Result<(), String> {
    let now = unix_now()?;
    grants
        .check(&binding.client, &binding.grant, binding.epoch, now)
        .map_err(|refusal| {
            let code = match refusal {
                GrantRefusal::Unknown => "not_admitted",
                GrantRefusal::Revoked => "revoked",
                GrantRefusal::EpochMismatch | GrantRefusal::Expired => "stale",
            };
            format!("Chamber grant refused: {code}")
        })
}

type Watch = (Box<dyn Fn() -> bool + Send>, Duration);

/// Carries a byte stream over an open channel. The returned end reads what
/// the peer sent and writes data frames of at most [`MAX_DATA_BYTES`]. With a
/// watch, the stream ends as soon as the check fails.
fn bridge<S>(channel: Channel<S>, watch: Option<Watch>) -> DuplexStream
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (near, far) = tokio::io::duplex(BUFFER);
    let (mut far_read, mut far_write) = tokio::io::split(far);
    let (mut reader, mut writer) = channel.into_split();
    let (stop, mut stopped) = oneshot::channel::<()>();
    // A data frame read is not cancellation safe; a stop discards the reader.
    let mut up = tokio::spawn(async move {
        loop {
            tokio::select! {
                received = reader.recv() => match received {
                    Ok(Some(data)) => {
                        if far_write.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    Ok(None) | Err(_) => break,
                },
                _ = &mut stopped => break,
            }
        }
        let _ = far_write.shutdown().await;
    });
    let mut down = tokio::spawn(async move {
        let mut buffer = vec![0; MAX_DATA_BYTES];
        loop {
            match far_read.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if writer.send(&buffer[..n]).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = writer.close().await;
    });
    tokio::spawn(async move {
        let Some((admitted, every)) = watch else {
            tokio::select! {
                _ = &mut up => {}
                _ = &mut down => up.abort(),
            }
            return;
        };
        let mut ticker = tokio::time::interval(every);
        ticker.tick().await;
        loop {
            tokio::select! {
                _ = &mut up => return,
                _ = &mut down => {
                    up.abort();
                    return;
                }
                _ = ticker.tick() => {
                    if !admitted() {
                        // The near end reads end of stream, its session
                        // closes, and the writer sends a close frame.
                        let _ = stop.send(());
                        let _ = down.await;
                        return;
                    }
                }
            }
        }
    });
    near
}

/// Serves world channels on a bound listener without durable storage.
pub async fn serve<G, F>(
    listener: TcpListener,
    server: Server<G>,
    gateway: Gateway,
    shutdown: F,
) -> Exit
where
    G: GrantCheck + 'static,
    F: Future<Output = ()>,
{
    let listen = Listen::Reach(Arc::new(server));
    net::serve_with_store(listener, listen, gateway, None, shutdown).await
}

/// Serves world channels, committing world mutations before replies.
pub async fn serve_durable<G, F>(
    listener: TcpListener,
    server: Server<G>,
    gateway: Gateway,
    store: Store,
    shutdown: F,
) -> Exit
where
    G: GrantCheck + 'static,
    F: Future<Output = ()>,
{
    let listen = Listen::Reach(Arc::new(server));
    net::serve_with_store(listener, listen, gateway, Some(store), shutdown).await
}

/// Joins a world instance over a direct channel on `stream`: a TCP
/// connection, or one a caller upgraded with [`websocket::client`]. The
/// device key in `config` signs the chamber challenge.
pub async fn join<S>(
    stream: S,
    config: &ClientConfig,
    instance: u64,
    content: Option<[u8; 32]>,
) -> Result<Client, String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let channel = connect(stream, config, unix_now()?)
        .await
        .map_err(|error| format!("Chamber channel refused: {error}"))?;
    let key = Keypair::from_secret_key(&Secp256k1::new(), &config.device);
    Client::connect_stream(Box::new(bridge(channel, None)), instance, content, &key).await
}

fn unix_now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|_| "System clock is before the Unix epoch".into())
}

fn key_bytes(hex: &str) -> Result<[u8; 32], String> {
    hex.parse::<secp256k1::XOnlyPublicKey>()
        .map(|key| key.serialize())
        .map_err(|_| "Channel device key is malformed".into())
}

#[cfg(test)]
#[path = "reach_tests.rs"]
mod tests;
