//! Direct peer routes: reach another computer's engine while the sync device
//! room is down or refusing (#12130).
//!
//! Peer RPC normally rides the sync device room. During a sync outage (the
//! #12128 429s) every app lost its other computers even though they were up
//! on the LAN and on Tailscale. This module gives each engine a second way
//! in, over the same iroh transport phone pairing uses
//! (`openagents_connect`), carrying the same device frames as the room
//! (`openagents_rpc::serve_direct`).
//!
//! Authorization stays with the relay path. A peer asks for a route with
//! [`methods::PEER_DIRECT_ROUTE`] over an authorized link (the room checks the
//! account), and the host answers with its iroh endpoint ID, its direct
//! addresses (LAN, the Tailscale `100.x` address when `tailscale` is up, and
//! our iroh relay), and a fresh admission ticket bound to the caller's device
//! ID. The caller keeps the route on disk ([`DirectPeers`]) so it works while
//! sync is down. A direct dial must present an unexpired ticket the host
//! issued, over an iroh connection whose host key matches the route; the
//! channel is encrypted and the host is authenticated by its endpoint key.
//! Tickets and cached routes belong to the signed-in account: the host keeps
//! only ticket hashes, refuses direct dials while signed out or signed in to
//! another account, and closes every direct session on sign-out.
//!
//! [`methods::PEER_DIRECT_ROUTE`]: openagents_rpc::methods::PEER_DIRECT_ROUTE

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use async_trait::async_trait;
use openagents_connect::endpoint::{ConnectEndpoint, EndpointConfig, Relay};
use openagents_connect::iroh::{EndpointAddr, EndpointId, RelayUrl, SecretKey};
use openagents_connect::stream::IrohStream;
use openagents_rpc::{
    DirectRoutes, DirectStream, RpcClient, RpcError, RpcReply, RpcService, TokenError, TokenSource,
    methods, parse_params,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The ALPN direct peer RPC answers on.
pub const ALPN: &[u8] = b"openagents/peer-rpc/1";
/// The first line a dialing peer sends.
const HELLO: &str = "openagents.peer-direct-hello.v1";
/// The host's one-line answer.
const WELCOME: &str = "openagents.peer-direct-welcome.v1";
/// The longest handshake line either side reads.
const MAX_LINE: usize = 4096;
/// How long the handshake may take.
const HANDSHAKE_LIMIT: Duration = Duration::from_secs(10);
/// How long a ticket admits its device.
const TICKET_TTL_SECS: u64 = 14 * 24 * 60 * 60;
/// A caller asks for a fresh route at most this often per peer.
const REFRESH_AFTER_SECS: u64 = 60 * 60;
/// Tickets kept per caller device (a refresh whose answer was lost must not
/// strand the ticket the caller still holds).
const TICKETS_PER_DEVICE: usize = 2;
/// Tickets kept in all.
const MAX_TICKETS: usize = 256;
/// Direct sessions served at once.
const MAX_SESSIONS: usize = 32;
/// The port the host prefers, so a cached route survives a host restart.
pub const PREFERRED_PORT: u16 = 47111;
/// Set to `0` to turn direct peer routes off.
pub const ENV_VAR: &str = "OPENAGENTS_PEER_DIRECT";

/// The signed-in account on this computer, or `None` while signed out.
/// Tickets and cached routes belong to one account.
pub type AccountProbe = Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// Whether direct peer routes are on for this process (on unless `0`).
pub fn enabled(value: Option<&str>) -> bool {
    !matches!(value.map(str::trim), Some("0" | "false" | "off"))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hash(ticket: &str) -> [u8; 32] {
    Sha256::digest(ticket.as_bytes()).into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Constant-time comparison of two hashes.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Write `bytes` to `path` readable by this user only, replacing it whole.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        std::io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// A host's answer to [`methods::PEER_DIRECT_ROUTE`]: how to reach it
/// directly, and the ticket that admits the caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectRoute {
    /// The host's iroh endpoint ID (hex); the dial proves the host holds it.
    pub endpoint_id: String,
    /// Our iroh relay, when the host uses one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<String>,
    /// Direct socket addresses: Tailscale first, then LAN.
    pub addrs: Vec<String>,
    /// The admission ticket. A secret: kept in a private file, never logged.
    pub ticket: String,
    /// Unix seconds after which the host refuses the ticket.
    pub expires_at: u64,
}

impl std::fmt::Debug for Hello {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hello")
            .field("device", &self.device)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Hello {
    v: String,
    device: String,
    ticket: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Welcome {
    v: String,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Refusal codes in a [`Welcome`].
const REFUSED_TICKET: &str = "ticket";
const REFUSED_SIGNED_OUT: &str = "signed_out";
const REFUSED_MALFORMED: &str = "malformed";

async fn read_line<S: AsyncRead + Unpin>(stream: &mut S) -> std::io::Result<String> {
    // Byte by byte: what follows the line belongs to the WebSocket.
    let mut line = Vec::with_capacity(256);
    loop {
        let byte = stream.read_u8().await?;
        if byte == b'\n' {
            break;
        }
        if line.len() >= MAX_LINE {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        line.push(byte);
    }
    String::from_utf8(line).map_err(|_| std::io::ErrorKind::InvalidData.into())
}

async fn write_line<S: AsyncWrite + Unpin, T: Serialize>(
    stream: &mut S,
    value: &T,
) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    line.push(b'\n');
    stream.write_all(&line).await?;
    stream.flush().await
}

// ---------------------------------------------------------------------------
// Host: tickets
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TicketRecord {
    device: String,
    account: String,
    /// Hex SHA-256 of the ticket; the ticket itself is never stored.
    hash: String,
    issued_at: u64,
    expires_at: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct TicketFile {
    tickets: Vec<TicketRecord>,
}

/// The host's issued tickets, persisted so a restart doesn't strand callers.
struct Tickets {
    path: PathBuf,
    records: Vec<TicketRecord>,
}

impl Tickets {
    fn open(path: PathBuf) -> Self {
        let records = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<TicketFile>(&bytes).ok())
            .map(|file| file.tickets)
            .unwrap_or_default();
        Self { path, records }
    }

    fn save(&self) {
        let file = TicketFile {
            tickets: self.records.clone(),
        };
        let result = serde_json::to_vec(&file)
            .map_err(std::io::Error::other)
            .and_then(|bytes| write_private(&self.path, &bytes));
        if let Err(error) = result {
            tracing::warn!(%error, "peer-direct: couldn't save tickets");
        }
    }

    fn issue(&mut self, device: &str, account: &str, at: u64) -> (String, u64) {
        let ticket = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let expires_at = at + TICKET_TTL_SECS;
        self.records.retain(|r| r.expires_at > at);
        let mut mine: Vec<usize> = self
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.device == device)
            .map(|(i, _)| i)
            .collect();
        while mine.len() >= TICKETS_PER_DEVICE {
            let oldest = mine.remove(0);
            self.records.remove(oldest);
            for index in &mut mine {
                *index -= 1;
            }
        }
        while self.records.len() >= MAX_TICKETS {
            self.records.remove(0);
        }
        self.records.push(TicketRecord {
            device: device.into(),
            account: account.into(),
            hash: hex(&hash(&ticket)),
            issued_at: at,
            expires_at,
        });
        self.save();
        (ticket, expires_at)
    }

    fn admits(&self, device: &str, ticket: &str, account: &str, at: u64) -> bool {
        let presented = hex(&hash(ticket));
        self.records.iter().any(|r| {
            r.device == device
                && r.account == account
                && r.expires_at > at
                && same(r.hash.as_bytes(), presented.as_bytes())
        })
    }
}

// ---------------------------------------------------------------------------
// Host
// ---------------------------------------------------------------------------

/// How a [`DirectHost`] binds.
#[derive(Clone, Debug)]
pub struct DirectHostConfig {
    /// Where the endpoint key and tickets live.
    pub dir: PathBuf,
    pub relay: Relay,
    /// Sockets to try first; on failure the endpoint binds iroh's defaults.
    pub bind: Vec<SocketAddr>,
    /// Look up and publish this machine's Tailscale address.
    pub tailnet: bool,
    /// Publish loopback addresses (same-machine tests only).
    pub loopback: bool,
}

impl DirectHostConfig {
    /// Production: our iroh relay, the preferred port, Tailscale on.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            relay: Relay::openagents(),
            bind: vec![
                SocketAddr::from(([0, 0, 0, 0], PREFERRED_PORT)),
                SocketAddr::from((std::net::Ipv6Addr::UNSPECIFIED, PREFERRED_PORT)),
            ],
            tailnet: true,
            loopback: false,
        }
    }

    /// No relay, loopback only. For tests.
    pub fn loopback(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            relay: Relay::Disabled,
            bind: vec![SocketAddr::from(([127, 0, 0, 1], 0))],
            tailnet: false,
            loopback: true,
        }
    }
}

/// Load the host's iroh key, or make and keep one, so its endpoint ID (and so
/// every cached route) survives restarts.
fn endpoint_secret(dir: &Path) -> anyhow::Result<SecretKey> {
    let path = dir.join("endpoint.key");
    if let Ok(bytes) = std::fs::read(&path)
        && let Ok(bytes) = <[u8; 32]>::try_from(bytes.as_slice())
    {
        return Ok(SecretKey::from_bytes(&bytes));
    }
    let secret = SecretKey::generate();
    write_private(&path, &secret.to_bytes())?;
    Ok(secret)
}

/// This computer's direct peer listener: answers [`ALPN`] on its own iroh
/// endpoint, admits callers by ticket, and serves them the engine's RPC
/// surface. Dropping it closes the endpoint and every direct session.
pub struct DirectHost {
    endpoint: ConnectEndpoint,
    relay: Option<String>,
    loopback: bool,
    tickets: Mutex<Tickets>,
    tailnet: Mutex<Option<IpAddr>>,
    account: AccountProbe,
    token: Arc<dyn TokenSource>,
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    sessions: Mutex<tokio::task::JoinSet<()>>,
}

impl DirectHost {
    /// Bind the endpoint. Call [`DirectHost::serve`] to start admitting.
    pub async fn start(
        config: DirectHostConfig,
        account: AccountProbe,
        token: Arc<dyn TokenSource>,
    ) -> anyhow::Result<Arc<Self>> {
        std::fs::create_dir_all(&config.dir)?;
        let secret = endpoint_secret(&config.dir)?;
        let relay = match &config.relay {
            Relay::Disabled => None,
            Relay::Custom(url) => Some(url.as_str().to_owned()),
        };
        let bind = |bind: Vec<SocketAddr>| EndpointConfig {
            relay: config.relay.clone(),
            bind,
            alpns: vec![ALPN.to_vec()],
        };
        let endpoint = match ConnectEndpoint::bind(secret.clone(), bind(config.bind.clone())).await
        {
            Ok(endpoint) => endpoint,
            // The preferred port is taken (another profile, say): any port.
            Err(_) if !config.bind.is_empty() => ConnectEndpoint::bind(secret, bind(Vec::new()))
                .await
                .map_err(|error| anyhow::anyhow!("peer-direct endpoint: {error}"))?,
            Err(error) => anyhow::bail!("peer-direct endpoint: {error}"),
        };
        let host = Arc::new(Self {
            endpoint,
            relay,
            loopback: config.loopback,
            tickets: Mutex::new(Tickets::open(config.dir.join("tickets.json"))),
            tailnet: Mutex::new(None),
            account,
            token,
            tasks: Mutex::new(Vec::new()),
            sessions: Mutex::new(tokio::task::JoinSet::new()),
        });
        if config.tailnet {
            let weak = Arc::downgrade(&host);
            let task = tokio::spawn(async move {
                // Tailscale may come up after the app: look again now and then.
                loop {
                    let program = coder_host::tailnet::program();
                    let ip = coder_host::tailnet::me(&program)
                        .await
                        .ok()
                        .map(|me| IpAddr::V4(me.ip));
                    let Some(host) = weak.upgrade() else { return };
                    *lock(&host.tailnet) = ip;
                    drop(host);
                    tokio::time::sleep(Duration::from_secs(5 * 60)).await;
                }
            });
            lock(&host.tasks).push(task);
        }
        tracing::info!(endpoint = %host.endpoint.endpoint.id().fmt_short(), "peer-direct: listening");
        Ok(host)
    }

    /// This host's iroh endpoint ID.
    pub fn endpoint_id(&self) -> EndpointId {
        self.endpoint.endpoint.id()
    }

    /// Admit ticket holders and serve them `service`. Also closes every
    /// direct session when this computer signs out, as the room's do.
    pub fn serve(self: &Arc<Self>, service: Arc<dyn RpcService>) {
        let accept = tokio::spawn(accept_loop(Arc::downgrade(self), service));
        let mut tasks = lock(&self.tasks);
        tasks.push(accept);
        if let Some(mut changes) = self.token.subscribe() {
            let weak = Arc::downgrade(self);
            tasks.push(tokio::spawn(async move {
                while changes.changed().await.is_ok() {
                    let Some(host) = weak.upgrade() else { return };
                    if (host.account)().is_none()
                        || matches!(host.token.token().await, Err(TokenError::SignedOut))
                    {
                        host.close_sessions();
                    }
                }
            }));
        }
    }

    /// Close every direct session.
    pub fn close_sessions(&self) {
        lock(&self.sessions).abort_all();
        tracing::info!("peer-direct: signed out; direct sessions closed");
    }

    /// Wrap `inner` so it also answers [`methods::PEER_DIRECT_ROUTE`]. Serve
    /// the wrapped service over the room (where the call is authorized) and
    /// over direct sessions.
    pub fn wrap(self: &Arc<Self>, inner: Arc<dyn RpcService>) -> Arc<dyn RpcService> {
        // Weak: the accept task holds the wrapped service, so a strong
        // reference here would keep the host (and its endpoint) alive forever.
        Arc::new(WithDirectRoute {
            inner,
            host: Arc::downgrade(self),
        })
    }

    /// Issue a route and ticket for `device`.
    pub fn route_for(&self, device: &str) -> Result<DirectRoute, RpcError> {
        let device = device.trim();
        if device.is_empty() || device.len() > 200 {
            return Err(RpcError::BadParams("deviceId".into()));
        }
        let account = (self.account)()
            .ok_or_else(|| RpcError::Failed("this computer is signed out".into()))?;
        let (ticket, expires_at) = lock(&self.tickets).issue(device, &account, now());
        Ok(DirectRoute {
            endpoint_id: self.endpoint_id().to_string(),
            relay: self.relay.clone(),
            addrs: self.addrs().iter().map(ToString::to_string).collect(),
            ticket,
            expires_at,
        })
    }

    /// The direct addresses to publish: the Tailscale address first, then
    /// the LAN addresses iroh found on this machine's interfaces.
    fn addrs(&self) -> Vec<SocketAddr> {
        let bound = self.endpoint.endpoint.bound_sockets();
        let port_v4 = bound
            .iter()
            .find(|s| s.is_ipv4())
            .map(SocketAddr::port)
            .unwrap_or(PREFERRED_PORT);
        let mut addrs = Vec::new();
        if let Some(ip) = *lock(&self.tailnet) {
            addrs.push(SocketAddr::new(ip, port_v4));
        }
        let found = if self.loopback {
            self.endpoint.local_addr()
        } else {
            self.endpoint.endpoint.addr()
        };
        for addr in found.ip_addrs() {
            let usable = !addr.ip().is_unspecified() && (self.loopback || !addr.ip().is_loopback());
            if usable && !addrs.contains(addr) {
                addrs.push(*addr);
            }
        }
        addrs
    }

    /// Check one caller's hello.
    fn admit(&self, hello: &Hello) -> Result<(), &'static str> {
        if hello.v != HELLO || hello.device.is_empty() || hello.ticket.is_empty() {
            return Err(REFUSED_MALFORMED);
        }
        // A signed-out computer serves no one, directly or through the room.
        // The account is read locally, never from the token service: a token
        // the server can't refresh right now still belongs to a signed-in
        // computer, and that outage is what this path exists for.
        let Some(account) = (self.account)() else {
            return Err(REFUSED_SIGNED_OUT);
        };
        if !lock(&self.tickets).admits(&hello.device, &hello.ticket, &account, now()) {
            return Err(REFUSED_TICKET);
        }
        Ok(())
    }
}

impl Drop for DirectHost {
    fn drop(&mut self) {
        for task in lock(&self.tasks).drain(..) {
            task.abort();
        }
        lock(&self.sessions).abort_all();
        let endpoint = self.endpoint.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move { endpoint.close().await });
        }
    }
}

async fn accept_loop(host: Weak<DirectHost>, service: Arc<dyn RpcService>) {
    let endpoint = match host.upgrade() {
        Some(host) => host.endpoint.endpoint.clone(),
        None => return,
    };
    let limit = Arc::new(tokio::sync::Semaphore::new(MAX_SESSIONS));
    while let Some(incoming) = endpoint.accept().await {
        let Some(strong) = host.upgrade() else { return };
        let Ok(permit) = limit.clone().try_acquire_owned() else {
            tracing::warn!("peer-direct: too many direct sessions; refusing one");
            continue;
        };
        let weak = host.clone();
        let service = service.clone();
        let mut sessions = lock(&strong.sessions);
        while sessions.try_join_next().is_some() {}
        sessions.spawn(async move {
            let _permit = permit;
            let Ok(connection) = incoming.await else {
                return;
            };
            if connection.alpn() != ALPN {
                return;
            }
            let handshake = async {
                let mut stream = IrohStream::accept(connection).await.ok()?;
                let line = read_line(&mut stream).await.ok()?;
                let verdict = match serde_json::from_str::<Hello>(&line) {
                    Ok(hello) => match weak.upgrade() {
                        Some(host) => host.admit(&hello).map(|()| hello.device),
                        None => return None,
                    },
                    Err(_) => Err(REFUSED_MALFORMED),
                };
                let welcome = Welcome {
                    v: WELCOME.into(),
                    ok: verdict.is_ok(),
                    error: verdict.as_ref().err().map(|code| (*code).to_owned()),
                };
                write_line(&mut stream, &welcome).await.ok()?;
                match verdict {
                    Ok(device) => Some((stream, device)),
                    Err(code) => {
                        tracing::info!(%code, "peer-direct: refused a direct dial");
                        None
                    }
                }
            };
            let Ok(Some((stream, device))) = tokio::time::timeout(HANDSHAKE_LIMIT, handshake).await
            else {
                return;
            };
            tracing::info!(%device, "peer-direct: serving a direct session");
            openagents_rpc::serve_direct(stream, service).await;
            tracing::info!(%device, "peer-direct: direct session ended");
        });
    }
}

struct WithDirectRoute {
    inner: Arc<dyn RpcService>,
    host: Weak<DirectHost>,
}

#[async_trait]
impl RpcService for WithDirectRoute {
    async fn handle(&self, method: &str, params: serde_json::Value) -> Result<RpcReply, RpcError> {
        if method != methods::PEER_DIRECT_ROUTE {
            return self.inner.handle(method, params).await;
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            device_id: String,
        }
        let p: P = parse_params(params)?;
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| RpcError::Failed("direct routes are off".into()))?;
        RpcReply::value(&host.route_for(&p.device_id)?)
    }
}

// ---------------------------------------------------------------------------
// Caller
// ---------------------------------------------------------------------------

/// How [`DirectPeers`] dials.
#[derive(Clone)]
pub struct DirectPeersConfig {
    /// Where the route cache lives.
    pub dir: PathBuf,
    /// This computer's device ID, which tickets are bound to.
    pub device_id: String,
    pub relay: Relay,
    /// Routes are kept and used for this account only.
    pub account: AccountProbe,
}

impl DirectPeersConfig {
    pub fn new(
        dir: impl Into<PathBuf>,
        device_id: impl Into<String>,
        account: AccountProbe,
    ) -> Self {
        Self {
            dir: dir.into(),
            device_id: device_id.into(),
            relay: Relay::openagents(),
            account,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedRoute {
    route: DirectRoute,
    account: String,
    refreshed_at: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct RouteFile {
    peers: HashMap<String, CachedRoute>,
}

/// The direct routes this computer has learned, cached per peer device on
/// disk so they work while sync is down: the [`DirectRoutes`] the peer link
/// cache falls back to.
pub struct DirectPeers {
    config: DirectPeersConfig,
    path: PathBuf,
    routes: Mutex<HashMap<String, CachedRoute>>,
    endpoint: tokio::sync::OnceCell<ConnectEndpoint>,
    refreshing: Mutex<HashSet<String>>,
    /// Peers whose engines predate direct routes.
    unsupported: Mutex<HashSet<String>>,
    /// For the background refresh `learn` starts from `&self`.
    me: Weak<Self>,
}

impl DirectPeers {
    pub fn open(config: DirectPeersConfig) -> Arc<Self> {
        let path = config.dir.join("routes.json");
        let routes = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<RouteFile>(&bytes).ok())
            .map(|file| file.peers)
            .unwrap_or_default();
        Arc::new_cyclic(|me| Self {
            config,
            path,
            routes: Mutex::new(routes),
            endpoint: tokio::sync::OnceCell::new(),
            refreshing: Mutex::new(HashSet::new()),
            unsupported: Mutex::new(HashSet::new()),
            me: me.clone(),
        })
    }

    /// The cached, unexpired route to `device_id` for the signed-in account.
    pub fn route(&self, device_id: &str) -> Option<DirectRoute> {
        let account = (self.config.account)()?;
        lock(&self.routes)
            .get(device_id)
            .filter(|cached| cached.account == account && cached.route.expires_at > now())
            .map(|cached| cached.route.clone())
    }

    /// Keep `route` for `device_id` under the signed-in account, on disk too.
    pub fn remember(&self, device_id: &str, route: DirectRoute) {
        let Some(account) = (self.config.account)() else {
            return;
        };
        let mut routes = lock(&self.routes);
        routes.insert(
            device_id.to_owned(),
            CachedRoute {
                route,
                account,
                refreshed_at: now(),
            },
        );
        self.save(&routes);
    }

    /// Forget the route to `device_id` (its host refused the ticket).
    pub fn forget(&self, device_id: &str) {
        let mut routes = lock(&self.routes);
        if routes.remove(device_id).is_some() {
            self.save(&routes);
        }
    }

    fn save(&self, routes: &HashMap<String, CachedRoute>) {
        let file = RouteFile {
            peers: routes.clone(),
        };
        let result = serde_json::to_vec(&file)
            .map_err(std::io::Error::other)
            .and_then(|bytes| write_private(&self.path, &bytes));
        if let Err(error) = result {
            tracing::warn!(%error, "peer-direct: couldn't save routes");
        }
    }

    async fn endpoint(&self) -> Result<&ConnectEndpoint, RpcError> {
        self.endpoint
            .get_or_try_init(|| async {
                let bind = match self.config.relay {
                    Relay::Disabled => vec![SocketAddr::from(([127, 0, 0, 1], 0))],
                    Relay::Custom(_) => Vec::new(),
                };
                ConnectEndpoint::bind(
                    SecretKey::generate(),
                    EndpointConfig {
                        relay: self.config.relay.clone(),
                        bind,
                        alpns: Vec::new(),
                    },
                )
                .await
                .map_err(|error| RpcError::Transport(format!("direct route: {error}")))
            })
            .await
    }

    fn due(&self, device_id: &str) -> bool {
        let at = now();
        let account = (self.config.account)();
        match lock(&self.routes).get(device_id) {
            None => true,
            Some(cached) if account.as_deref() != Some(cached.account.as_str()) => true,
            Some(cached) => {
                at.saturating_sub(cached.refreshed_at) >= REFRESH_AFTER_SECS
                    || cached.route.expires_at <= at + TICKET_TTL_SECS / 2
            }
        }
    }

    /// Ask `device_id` for a fresh route over `client` and keep it.
    pub async fn refresh(&self, device_id: &str, client: &RpcClient) -> Result<(), RpcError> {
        let value = client
            .call(
                methods::PEER_DIRECT_ROUTE,
                serde_json::json!({ "deviceId": self.config.device_id }),
            )
            .await?;
        let route: DirectRoute = serde_json::from_value(value)
            .map_err(|e| RpcError::Failed(format!("direct route: {e}")))?;
        self.remember(device_id, route);
        Ok(())
    }
}

#[async_trait]
impl DirectRoutes for DirectPeers {
    fn knows(&self, device_id: &str) -> bool {
        self.route(device_id).is_some()
    }

    async fn dial(&self, device_id: &str) -> Result<DirectStream, RpcError> {
        let route = self
            .route(device_id)
            .ok_or_else(|| RpcError::Transport(format!("no direct route to {device_id}")))?;
        let bad = |what: &str| RpcError::Transport(format!("direct route to {device_id}: {what}"));
        let id: EndpointId = route.endpoint_id.parse().map_err(|_| bad("bad endpoint"))?;
        let mut addr = EndpointAddr::new(id);
        for socket in &route.addrs {
            if let Ok(socket) = socket.parse::<SocketAddr>() {
                addr = addr.with_ip_addr(socket);
            }
        }
        if let Some(relay) = route.relay.as_deref()
            && let Ok(url) = relay.parse::<RelayUrl>()
        {
            addr = addr.with_relay_url(url);
        }
        let endpoint = self.endpoint().await?;
        // The QUIC handshake proves the host holds the route's endpoint key.
        let connection = endpoint
            .endpoint
            .connect(addr, ALPN)
            .await
            .map_err(|e| bad(&format!("connect failed: {e}")))?;
        let mut stream = IrohStream::open(connection)
            .await
            .map_err(|e| bad(&format!("stream failed: {e}")))?;
        let hello = Hello {
            v: HELLO.into(),
            device: self.config.device_id.clone(),
            ticket: route.ticket.clone(),
        };
        let handshake = async {
            write_line(&mut stream, &hello).await?;
            read_line(&mut stream).await
        };
        let line = tokio::time::timeout(HANDSHAKE_LIMIT, handshake)
            .await
            .map_err(|_| bad("handshake timed out"))?
            .map_err(|e| bad(&format!("handshake failed: {e}")))?;
        let welcome: Welcome = serde_json::from_str(&line).map_err(|_| bad("bad answer"))?;
        if welcome.v != WELCOME {
            return Err(bad("bad answer"));
        }
        if !welcome.ok {
            let code = welcome.error.unwrap_or_default();
            if code == REFUSED_TICKET {
                // The host no longer knows this ticket: don't keep dialing it.
                self.forget(device_id);
            }
            return Err(bad(&format!("refused ({code})")));
        }
        Ok(Box::new(stream))
    }

    fn learn(&self, device_id: &str, client: Arc<RpcClient>) {
        if !self.due(device_id) || lock(&self.unsupported).contains(device_id) {
            return;
        }
        if !lock(&self.refreshing).insert(device_id.to_owned()) {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            lock(&self.refreshing).remove(device_id);
            return;
        };
        let device = device_id.to_owned();
        let this = self.me.clone();
        runtime.spawn(async move {
            let Some(this) = this.upgrade() else { return };
            match this.refresh(&device, &client).await {
                Ok(()) => tracing::info!(%device, "peer-direct: route learned"),
                Err(RpcError::UnknownMethod(_)) => {
                    lock(&this.unsupported).insert(device.clone());
                }
                Err(error) => tracing::debug!(%device, %error, "peer-direct: route refresh failed"),
            }
            lock(&this.refreshing).remove(&device);
        });
    }
}
