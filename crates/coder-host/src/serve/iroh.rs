//! The iroh listener: enrollment and direct channels on the host's iroh
//! endpoint.
//!
//! iroh is transport. Its TLS proves the far end holds the `EndpointId` it
//! dialed and nothing more; an `EndpointId` never admits anything here.
//!
//! - `openagents/enroll/1` carries one signed NIP-HOST `enroll.redeem`.
//!   The host admits it through the same redemption as the relay binding,
//!   bound to the invitation's own relay, so a second device redeeming the
//!   same invitation gets `forbidden`. Any other operation, an unknown
//!   invitation, or a wrong capability gets no signed reply.
//! - `openagents/reach/1` runs the unchanged NIP-REACH handshake over one
//!   QUIC stream, with the grant store behind its grant check, and then the
//!   same session as a TCP or WebSocket channel: NIP-HOST calls, NIP-TERM
//!   terminals, rechecks before every message, and closing on revocation.
//! - Nearby approval shares the enroll ALPN: the endpoint advertises itself
//!   over mDNS, and a nearby request goes to the gate in [`super::nearby`],
//!   which grants only after a click on the computer.

use std::net::SocketAddr;
use std::sync::Arc;

use coder_reach::channel::Acceptor;
use coder_reach::hints::v2::{IrohHint, IrohTransport, MAX_DIRECT};
use coder_reach::hints::{Class, Status};
use nostr::domain::Event;
use openagents_connect::endpoint::{ConnectEndpoint, EndpointConfig, Relay};
use openagents_connect::enroll::{EnrollCall, EnrollProtocol, EnrollReply};
use openagents_connect::iroh::protocol::Router;
use openagents_connect::iroh::{EndpointId, RelayUrl, SecretKey};
use openagents_connect::nearby::{HostKeys, NearbyCall};
use openagents_connect::reach::{ReachProtocol, ReachSession};
use openagents_connect::{ENROLL_ALPN, REACH_ALPN};
use tokio::sync::{Semaphore, mpsc};
use tokio::task::JoinHandle;

use super::nearby::NearbyGate;
use super::{Shared, direct};
use crate::authority::Grants;
use crate::config::Iroh as IrohConfig;
use crate::{Error, Result, unix_time};

/// Enrollment calls the host answers at once; more wait their turn.
const ENROLL_CONCURRENCY: usize = 16;

/// A bound iroh endpoint and its protocol router.
pub(crate) struct Listener {
    pub(crate) endpoint: ConnectEndpoint,
    router: Router,
    /// The configured relay URL, as the operator spelled it.
    relay: Option<String>,
    /// Nearby requests and the click that answers one.
    pub(crate) nearby: NearbyGate,
}

impl Listener {
    /// The host's `EndpointId`.
    pub(crate) fn id(&self) -> EndpointId {
        self.endpoint.endpoint.id()
    }

    /// The relay this endpoint keeps its home connection on.
    pub(crate) fn relay(&self) -> Option<&str> {
        self.relay.as_deref()
    }

    /// Up to eight direct addresses the endpoint knows for itself, each
    /// valid as a hint address.
    pub(crate) fn direct(&self) -> Vec<SocketAddr> {
        let mut direct: Vec<SocketAddr> = self
            .endpoint
            .endpoint
            .addr()
            .ip_addrs()
            .copied()
            .chain(self.endpoint.local_addr().ip_addrs().copied())
            .filter(|socket| {
                let ip = socket.ip();
                socket.port() != 0
                    && !ip.is_unspecified()
                    && !ip.is_multicast()
                    && ip != std::net::IpAddr::V4(std::net::Ipv4Addr::BROADCAST)
            })
            .collect();
        // Addresses a phone on another network can use first; loopback
        // last, since only the same machine can use it.
        direct.sort_by_key(|socket| (socket.ip().is_loopback(), *socket));
        direct.dedup();
        direct.truncate(MAX_DIRECT);
        direct
    }

    /// Whether the endpoint has a relay or a direct address a phone could
    /// reach.
    pub(crate) fn online(&self) -> bool {
        let addr = self.endpoint.endpoint.addr();
        addr.relay_urls().next().is_some()
            || addr.ip_addrs().any(|socket| !socket.ip().is_loopback())
    }

    /// The host's signed `iroh` hint, or `None` when it has nothing to dial.
    pub(crate) fn hint(&self, now: u64) -> Option<IrohHint> {
        let hint = IrohHint {
            class: Class::Public,
            transport: IrohTransport::Iroh,
            address: hex(self.id().as_bytes()),
            relay: self.relay.clone(),
            direct: self.direct().iter().map(ToString::to_string).collect(),
            status: Status::Reachable,
            observed_at: now,
        };
        hint.validate().is_ok().then_some(hint)
    }

    pub(crate) async fn shutdown(&self) {
        let _ = self.router.shutdown().await;
        self.endpoint.close().await;
    }
}

/// Bind the endpoint with `secret`, answer both ALPNs, and spawn the tasks
/// that hand enrollment calls and admitted channels to the host.
pub(super) async fn start(
    shared: &Arc<Shared>,
    acceptor: Arc<Acceptor<Grants>>,
    secret: SecretKey,
    config: &IrohConfig,
) -> Result<(Listener, Vec<JoinHandle<()>>)> {
    let relay = config
        .relay
        .as_deref()
        .map(str::parse::<RelayUrl>)
        .transpose()
        .map_err(|_| Error::Config("the iroh relay is not a URL".into()))?;
    // The canonical spelling, as a connect code requires it.
    let relay_text = relay.as_ref().map(|url| url.as_str().to_owned());
    let endpoint = ConnectEndpoint::bind(
        secret,
        EndpointConfig {
            relay: relay.map_or(Relay::Disabled, Relay::Custom),
            bind: config.bind.clone(),
            alpns: openagents_connect::endpoint::host_alpns(),
        },
    )
    .await
    .map_err(|_| Error::Config("the iroh endpoint cannot bind".into()))?;
    let (calls, call_queue) = mpsc::channel::<EnrollCall>(ENROLL_CONCURRENCY);
    let (sessions, session_queue) = mpsc::channel::<ReachSession>(64);
    let (nearby_calls, nearby_queue) = mpsc::channel::<NearbyCall>(ENROLL_CONCURRENCY);
    let router = Router::builder(endpoint.endpoint.clone())
        .accept(
            ENROLL_ALPN,
            EnrollProtocol::new(calls).with_nearby(nearby_calls),
        )
        .accept(REACH_ALPN, ReachProtocol::new(acceptor, sessions))
        .spawn();
    let gate = NearbyGate::new(nearby_mint(shared));
    let host = HostKeys {
        endpoint: *endpoint.endpoint.id().as_bytes(),
        nostr: openagents_connect::nearby::parse_key(&shared.host_key)
            .map_err(|_| Error::Config("the host key is not 64 hex characters".into()))?,
    };
    // Phones on this network list the computer by its label. A network
    // without multicast only loses the nearby list; codes still work.
    let _ = openagents_connect::nearby::advertise(&endpoint, &shared.config.label);
    let mut tasks = vec![
        tokio::spawn(enroll(shared.clone(), call_queue)),
        tokio::spawn(reach(shared.clone(), session_queue)),
        tokio::spawn(nearby(gate.clone(), host, nearby_queue)),
    ];
    if let (Some(chats), true) = (&shared.config.chats, shared.config.serve_chats) {
        tasks.push(serve_chats(chats, &shared.config)?);
    }
    Ok((
        Listener {
            endpoint,
            router,
            relay: relay_text,
            nearby: gate,
        },
        tasks,
    ))
}

/// Answer each enrollment call. The reply carries the host's clock, so a
/// phone can name a clock that is off instead of reading a refusal as
/// `expired`.
async fn enroll(shared: Arc<Shared>, mut calls: mpsc::Receiver<EnrollCall>) {
    let limit = Arc::new(Semaphore::new(ENROLL_CONCURRENCY));
    while let Some(call) = calls.recv().await {
        let Ok(permit) = limit.clone().acquire_owned().await else {
            return;
        };
        let shared = shared.clone();
        tokio::spawn(async move {
            let reply = redeem(&shared, &call.request.request).await;
            let chats = match reply {
                Some(_) => chat_invitation(&shared, &call.request.request).await,
                None => None,
            };
            let now = unix_time().unwrap_or_default();
            let _ = call
                .reply
                .send(EnrollReply::new(now, reply).with_chats(chats));
            drop(permit);
        });
    }
}

/// The signed reply to one redemption, as JSON text, or `None` when the
/// host sends no signed reply.
pub(super) async fn redeem(shared: &Arc<Shared>, request: &str) -> Option<String> {
    if request.len() > openagents_connect::enroll::MAX_MESSAGE_BYTES {
        return None;
    }
    let value = nostr::contracts::parse_strict_bounded(
        request.as_bytes(),
        openagents_connect::enroll::MAX_MESSAGE_BYTES,
    )
    .ok()?;
    let event: Event = serde_json::from_value(value).ok()?;
    let authority = shared.authority.clone();
    let reply = tokio::task::spawn_blocking(move || authority.redeem(&event))
        .await
        .ok()?
        .ok()?;
    serde_json::to_string(&reply).ok()
}

/// Serve the read-only chat history observer on the relay, as tailnet
/// admission does, so a phone that paired with a code reads its Coder chats.
fn serve_chats(
    chats: &crate::tailnet::Chats,
    config: &crate::config::Config,
) -> Result<JoinHandle<()>> {
    coder_connect::host::ensure_parent(&chats.observer)
        .map_err(|_| Error::Config("the chat history store cannot be created".into()))?;
    let observer = coder_connect::host::Host::new(&chats.observer, config.policy).coder_only();
    let relay = config.primary()?.to_owned();
    let policy = config.policy;
    Ok(tokio::spawn(async move {
        if coder_connect::cli::serve_observer(observer, relay, policy)
            .await
            .is_err()
        {
            eprintln!("coder host: chat history stopped");
        }
    }))
}

/// A `coder-pair:` chat invitation for the device that signed `request`,
/// only when it now holds a current grant with `observe`: a device the
/// host refused never gets one. `None` when this host serves no chats.
async fn chat_invitation(shared: &Arc<Shared>, request: &str) -> Option<String> {
    shared.config.chats.as_ref()?;
    let event: Event = serde_json::from_str(request).ok()?;
    let device = event.pubkey;
    let shared = shared.clone();
    tokio::task::spawn_blocking(move || {
        let admitted = shared
            .authority
            .local(|host, now| host.devices(now))
            .ok()?
            .into_iter()
            .any(|entry| {
                entry.device == device
                    && entry.state == coder_access::protocol::DeviceState::Active
                    && entry.rights.contains(coder_access::Right::Observe)
            });
        if !admitted {
            return None;
        }
        super::dispatch::chat_invitation(&shared.config)
            .ok()
            .map(|(invitation, _)| invitation)
    })
    .await
    .ok()
    .flatten()
}

/// Signs a nearby device's grant after the click: an approval grant on the
/// host's relay with the connect-code rights (NIP-HOST, nearby approval).
fn nearby_mint(shared: &Arc<Shared>) -> super::nearby::Mint {
    let shared = Arc::downgrade(shared);
    Arc::new(move |device: &str, rights: coder_access::Rights| {
        let shared = shared.upgrade().ok_or("the host stopped")?;
        let relay = shared
            .config
            .primary()
            .map_err(|_| "the host serves no relay")?
            .to_owned();
        let envelope = shared
            .authority
            .local(|host, now| host.approve_nearby(device, &relay, rights.clone(), now))
            .map_err(|error| error.message.clone())?;
        serde_json::to_value(envelope).map_err(|error| error.to_string())
    })
}

/// Run each nearby exchange; the gate admits one at a time.
async fn nearby(gate: NearbyGate, host: HostKeys, mut calls: mpsc::Receiver<NearbyCall>) {
    let limit = Arc::new(Semaphore::new(ENROLL_CONCURRENCY));
    while let Some(call) = calls.recv().await {
        let Ok(permit) = limit.clone().acquire_owned().await else {
            return;
        };
        let gate = gate.clone();
        tokio::spawn(async move {
            let NearbyCall {
                remote,
                request,
                stream,
                done,
            } = call;
            let (reader, writer) = tokio::io::split(stream);
            let _ = super::nearby::serve(request, reader, writer, host, *remote.as_bytes(), &gate)
                .await;
            drop(done);
            drop(permit);
        });
    }
}

/// Serve each admitted channel like any other direct channel.
async fn reach(shared: Arc<Shared>, mut sessions: mpsc::Receiver<ReachSession>) {
    while let Some(session) = sessions.recv().await {
        tokio::spawn(direct::serve(shared.clone(), session.channel));
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}
