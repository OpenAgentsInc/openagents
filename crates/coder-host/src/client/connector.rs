//! A `coder-link` connector that proves routes to Coder hosts.
//!
//! Each attempt reads the host's presence and hints from the device's relay,
//! tries the selected direct routes in order, over TCP or WebSocket as each
//! hint names, and falls back to the relay.
//! Selection never offers a loopback route to a device on another machine.
//! A handshake refusal the host signed after proving its key blocks the
//! attempt instead of falling back: a revoked grant is revoked on every
//! route. Outcomes return through a channel the application drains into
//! `Registry::report`, because a connector must not call the registry.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_link::{AttemptId, BlockReason, ConnectionId, Failure, HostKey, Report, Stage};
use coder_reach::channel::UNAUTHENTICATED;
use coder_reach::hints::{Hint, Locality, Transport, select};
use coder_reach::presence::{ClientProfile, VersionRange};
use coder_reach::{PROTOCOL_VERSION, Refusal};
use nostr_transport::Connection;
use tokio::net::TcpStream;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::{Device, Link, Route, fetch_reach, websocket};
use crate::{Error, unix_time};

/// Outcomes for the application to pass to `Registry::report`.
pub type Reports = mpsc::UnboundedReceiver<(HostKey, Report)>;

/// How long one TCP connect, or one WebSocket connect and upgrade, may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// How long one handshake may take.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

type Links = Arc<Mutex<HashMap<(HostKey, u64), Arc<Link>>>>;

/// Proves routes for the hosts a registry supervises.
pub struct Connector {
    runtime: Handle,
    locality: Locality,
    tls: websocket::Tls,
    devices: HashMap<HostKey, Arc<Device>>,
    reports: mpsc::UnboundedSender<(HostKey, Report)>,
    links: Links,
    attempts: HashMap<(HostKey, u64), JoinHandle<()>>,
}

impl std::fmt::Debug for Connector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connector")
            .field("locality", &self.locality)
            .field("hosts", &self.devices.len())
            .finish_non_exhaustive()
    }
}

impl Connector {
    /// A connector that spawns its work on `runtime`. Claim
    /// `Locality::SameMachine` only from local evidence.
    #[must_use]
    pub fn new(runtime: Handle, locality: Locality) -> (Self, Reports) {
        let (reports, receiver) = mpsc::unbounded_channel();
        (
            Self {
                runtime,
                locality,
                tls: websocket::Tls::webpki(),
                devices: HashMap::new(),
                reports,
                links: Arc::default(),
                attempts: HashMap::new(),
            },
            receiver,
        )
    }

    /// Verify `wss` hints as `tls` says instead of against the WebPKI roots.
    /// Only a test replaces the roots.
    pub fn set_websocket_tls(&mut self, tls: websocket::Tls) {
        self.tls = tls;
    }

    /// Add an enrolled device's host and return the key to register.
    ///
    /// # Errors
    /// Refuses a host key the registry would refuse.
    pub fn add(&mut self, device: Arc<Device>) -> crate::Result<HostKey> {
        let key = HostKey::new(device.host())
            .map_err(|_| Error::Config("the host key is not a valid registry key".into()))?;
        self.devices.insert(key.clone(), device);
        Ok(key)
    }

    /// The link a registry connection names.
    #[must_use]
    pub fn link(&self, host: &HostKey, connection: ConnectionId) -> Option<Arc<Link>> {
        lock(&self.links)
            .get(&(host.clone(), connection.0))
            .cloned()
    }

    fn spawn(
        &mut self,
        host: &HostKey,
        attempt: AttemptId,
        work: impl Future<Output = ()> + Send + 'static,
    ) {
        let task = self.runtime.spawn(work);
        if let Some(old) = self.attempts.insert((host.clone(), attempt.0), task) {
            old.abort();
        }
    }
}

impl coder_link::Connector for Connector {
    fn open(&mut self, host: &HostKey, attempt: AttemptId, _stage: Stage) {
        let Some(device) = self.devices.get(host).cloned() else {
            let _ = self.reports.send((
                host.clone(),
                Report::Failed(attempt, Failure::Blocked(BlockReason::Configuration)),
            ));
            return;
        };
        let (reports, links, locality, tls, key) = (
            self.reports.clone(),
            self.links.clone(),
            self.locality,
            self.tls.clone(),
            host.clone(),
        );
        self.spawn(host, attempt, async move {
            match establish(device, locality, &tls).await {
                Ok(link) => {
                    let link = Arc::new(link);
                    lock(&links).insert((key.clone(), attempt.0), link.clone());
                    let _ = reports.send((key.clone(), Report::Established(attempt)));
                    // Report the loss of a direct channel with the host's code.
                    let code = link.wait_closed().await;
                    let _ = reports.send((
                        key,
                        Report::ConnectionLost(
                            ConnectionId(attempt.0),
                            closed_failure(code.as_deref()),
                        ),
                    ));
                }
                Err(failure) => {
                    let _ = reports.send((key, Report::Failed(attempt, failure)));
                }
            }
        });
    }

    fn probe(&mut self, host: &HostKey, attempt: AttemptId, connection: ConnectionId) {
        let link = self.link(host, connection);
        let (reports, locality, tls, key) = (
            self.reports.clone(),
            self.locality,
            self.tls.clone(),
            host.clone(),
        );
        self.spawn(host, attempt, async move {
            let Some(link) = link else {
                let _ = reports.send((key, Report::Failed(attempt, Failure::Closed)));
                return;
            };
            let healthy = link.ping().await.is_ok();
            // A relay route is a fallback. Its probe fails while a direct
            // route answers, so the supervisor replaces it with that route.
            let better = matches!(link.route(), Route::Relay(_))
                && healthy
                && direct_answers(link.device().clone(), locality, &tls).await;
            let report = if healthy && !better {
                Report::Established(attempt)
            } else {
                Report::Failed(attempt, Failure::Closed)
            };
            let _ = reports.send((key, report));
        });
    }

    fn cancel(&mut self, host: &HostKey, attempt: AttemptId) {
        if let Some(task) = self.attempts.remove(&(host.clone(), attempt.0)) {
            task.abort();
        }
    }

    fn close(&mut self, host: &HostKey, connection: ConnectionId) {
        // The attempt that opened the connection also watches it.
        if let Some(task) = self.attempts.remove(&(host.clone(), connection.0)) {
            task.abort();
        }
        if let Some(link) = lock(&self.links).remove(&(host.clone(), connection.0)) {
            link.shutdown();
        }
    }
}

/// Prove the best route: selected direct hints in order, then the relay.
async fn establish(
    device: Arc<Device>,
    locality: Locality,
    tls: &websocket::Tls,
) -> Result<Link, Failure> {
    let relay = device.relay().to_owned();
    let reach = fetch_reach(&device, &relay)
        .await
        .map_err(|error| match error {
            Error::Reach(e) if e.code == Refusal::Incompatible => {
                Failure::Blocked(BlockReason::Incompatible)
            }
            _ => Failure::Unreachable,
        })?;
    let client = ClientProfile {
        protocol: PROTOCOL_VERSION,
        accepts: VersionRange {
            min: PROTOCOL_VERSION,
            max: PROTOCOL_VERSION,
        },
    };
    if reach.presence.presence.compatible(&client).is_err() {
        return Err(Failure::Blocked(BlockReason::Incompatible));
    }
    let generation = reach.presence.presence.generation;
    let now = unix_time().map_err(|_| Failure::Unreachable)?;
    let hints =
        select(&reach.hints, locality, generation, now).map_err(|_| Failure::Unreachable)?;
    for hint in hints.iter().filter(|h| h.is_direct()) {
        match try_direct(&device, hint, generation, tls).await {
            Ok(link) => return Ok(link),
            Err(Some(blocked)) => return Err(blocked),
            Err(None) => {}
        }
    }
    // Relay fallback: an authenticated connection to the relay the grant
    // names, which just delivered fresh presence from the host.
    let socket = Connection::connect(&relay, &device.secret, Duration::from_secs(5))
        .await
        .map_err(|_| Failure::Unreachable)?;
    let _ = socket.close().await;
    Ok(Link::relay(device, relay))
}

/// `Err(Some)` blocks the attempt; `Err(None)` tries the next route. A
/// `tcp` hint and a `websocket` hint run the same handshake.
async fn try_direct(
    device: &Arc<Device>,
    hint: &Hint,
    generation: u64,
    tls: &websocket::Tls,
) -> Result<Link, Option<Failure>> {
    let (device, address) = (device.clone(), hint.address.clone());
    let opened = match hint.transport {
        Transport::Tcp => {
            let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&address))
                .await
                .map_err(|_| None)?
                .map_err(|_| None)?;
            Link::direct(device, stream, address, generation, HANDSHAKE_TIMEOUT).await
        }
        Transport::Websocket => {
            let stream = websocket::dial(&address, tls, CONNECT_TIMEOUT)
                .await
                .ok_or(None)?;
            Link::direct(device, stream, address, generation, HANDSHAKE_TIMEOUT).await
        }
        Transport::Nostr => return Err(None),
    };
    match opened {
        Ok(link) => Ok(link),
        Err(Error::Reach(refusal)) if refusal.detail != UNAUTHENTICATED => {
            Err(match refusal.code {
                Refusal::Revoked | Refusal::Stale
                    if refusal.detail == "host refused the channel" =>
                {
                    Some(Failure::Blocked(BlockReason::Revoked))
                }
                Refusal::NotAdmitted => Some(Failure::Blocked(BlockReason::Authentication)),
                Refusal::IdentityMismatch => Some(Failure::Blocked(BlockReason::Authentication)),
                // Another generation means the host restarted; read presence
                // again on the next attempt.
                _ => None,
            })
        }
        Err(_) => Err(None),
    }
}

/// Whether a direct route answers now.
async fn direct_answers(device: Arc<Device>, locality: Locality, tls: &websocket::Tls) -> bool {
    let Ok(reach) = fetch_reach(&device, device.relay()).await else {
        return false;
    };
    let generation = reach.presence.presence.generation;
    let Ok(now) = unix_time() else { return false };
    let Ok(hints) = select(&reach.hints, locality, generation, now) else {
        return false;
    };
    for hint in hints.iter().filter(|h| h.is_direct()) {
        if let Ok(link) = try_direct(&device, hint, generation, tls).await {
            drop(link);
            return true;
        }
    }
    false
}

/// The supervisor failure for a closed direct channel.
fn closed_failure(code: Option<&str>) -> Failure {
    match code {
        Some("revoked" | "stale") => Failure::Blocked(BlockReason::Revoked),
        Some("not_admitted") => Failure::Blocked(BlockReason::Authentication),
        _ => Failure::Closed,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
