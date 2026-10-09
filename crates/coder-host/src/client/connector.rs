//! A `coder-link` connector that proves routes to Coder hosts.
//!
//! Each attempt reads the host's presence and hints from the device's relay,
//! tries the host's iroh endpoint when this device saved one and has an
//! iroh key, then the selected direct routes in order, over TCP or
//! WebSocket as each hint names, and falls back to the relay.
//! Selection never offers a loopback route to a device on another machine.
//! A local route, such as the loopback port of an SSH tunnel this process
//! owns, is tried before the hints: it is same-machine evidence for that one
//! address, whatever the connector's locality, and it is never published.
//! A handshake refusal the host signed after proving its key blocks the
//! attempt instead of falling back: a revoked grant is revoked on every
//! route. Outcomes return through a channel the application drains into
//! `Registry::report`, because a connector must not call the registry.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_link::{AttemptId, BlockReason, ConnectionId, Failure, HostKey, Report, Stage};
use coder_reach::channel::{GENERATION_DIFFERS, UNAUTHENTICATED};
use coder_reach::hints::{Hint, Locality, Transport, select};
use coder_reach::presence::{ClientProfile, VersionRange};
use coder_reach::{PROTOCOL_VERSION, Refusal};
use nostr_transport::Connection;
use tokio::net::TcpStream;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::iroh::{Dialer, IrohRoute, open_link};
use super::{Device, Link, Reach, Route, fetch_reach, websocket};
use crate::{Error, unix_time};

/// Outcomes for the application to pass to `Registry::report`.
pub type Reports = mpsc::UnboundedReceiver<(HostKey, Report)>;

/// How long one TCP connect, or one WebSocket connect and upgrade, may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// How long one handshake may take.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

type Links = Arc<Mutex<HashMap<(HostKey, u64), Arc<Link>>>>;
/// Local routes by host: loopback addresses only this process can use.
type LocalRoutes = Arc<Mutex<HashMap<HostKey, SocketAddr>>>;
/// Saved iroh endpoints by host, from pairing.
type IrohRoutes = Arc<Mutex<HashMap<HostKey, IrohRoute>>>;

/// This device's iroh endpoint and the hosts' saved iroh routes.
#[derive(Clone, Default)]
struct Iroh {
    dialer: Option<Arc<Dialer>>,
    routes: IrohRoutes,
}

impl Iroh {
    /// The route to try for `host`, when this device can dial one.
    fn route(&self, host: &HostKey) -> Option<(Arc<Dialer>, IrohRoute)> {
        let dialer = self.dialer.clone()?;
        let route = lock(&self.routes).get(host).cloned()?;
        Some((dialer, route))
    }
}

/// Proves routes for the hosts a registry supervises.
pub struct Connector {
    runtime: Handle,
    locality: Locality,
    tls: websocket::Tls,
    devices: HashMap<HostKey, Arc<Device>>,
    reports: mpsc::UnboundedSender<(HostKey, Report)>,
    links: Links,
    local: LocalRoutes,
    iroh: Iroh,
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
                local: Arc::default(),
                iroh: Iroh::default(),
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

    /// Set or clear the local route for `host`: a loopback address on this
    /// machine that reaches the host, such as the forwarded port of an SSH
    /// tunnel this process runs. Each attempt tries it first, over TCP,
    /// before the host's hints and the relay. It is same-machine evidence for
    /// this one address only, so the connector's locality still governs the
    /// hints. Clear it when the tunnel ends; an attempt that finds it closed
    /// moves on to the hints and the relay.
    ///
    /// # Errors
    /// Refuses an address that is not loopback: a local route never leaves
    /// this machine.
    pub fn set_local_route(
        &mut self,
        host: &HostKey,
        address: Option<SocketAddr>,
    ) -> crate::Result<()> {
        let mut local = lock(&self.local);
        match address {
            Some(address) if !address.ip().is_loopback() => Err(Error::Config(
                "a local route must be a loopback address".into(),
            )),
            Some(address) => {
                local.insert(host.clone(), address);
                Ok(())
            }
            None => {
                local.remove(host);
                Ok(())
            }
        }
    }

    /// The local route set for `host`, if any.
    #[must_use]
    pub fn local_route(&self, host: &HostKey) -> Option<SocketAddr> {
        lock(&self.local).get(host).copied()
    }

    /// Dial saved iroh routes with `dialer`, this device's iroh endpoint.
    pub fn set_iroh(&mut self, dialer: Arc<Dialer>) {
        self.iroh.dialer = Some(dialer);
    }

    /// Set or clear the saved iroh route for `host`, from pairing. Each
    /// attempt tries it first when this device has an iroh endpoint; a
    /// route that does not answer moves on to the hints and the relay.
    pub fn set_iroh_route(&mut self, host: &HostKey, route: Option<IrohRoute>) {
        let mut routes = lock(&self.iroh.routes);
        match route {
            Some(route) => routes.insert(host.clone(), route),
            None => routes.remove(host),
        };
    }

    /// The saved iroh route for `host`, if any.
    #[must_use]
    pub fn iroh_route(&self, host: &HostKey) -> Option<IrohRoute> {
        lock(&self.iroh.routes).get(host).cloned()
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
        let local = self.local_route(host);
        let iroh = self.iroh.route(host);
        self.spawn(host, attempt, async move {
            match establish(device, locality, local, iroh, &tls).await {
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
        let local = self.local_route(host);
        let iroh = self.iroh.route(host);
        self.spawn(host, attempt, async move {
            let Some(link) = link else {
                let _ = reports.send((key, Report::Failed(attempt, Failure::Closed)));
                return;
            };
            let healthy = link.ping().await.is_ok();
            // A relay route is a fallback. Its probe fails while a direct
            // route answers, so the supervisor replaces it with that route.
            // It also follows the host's generation from the presence it
            // reads: a relay route outlives a host restart.
            let better = matches!(link.route(), Route::Relay(_))
                && healthy
                && match fetch_reach(link.device(), link.device().relay()).await {
                    Ok(reach) => {
                        link.note_generation(reach.presence.presence.generation);
                        direct_answers(link.device().clone(), &reach, locality, local, iroh, &tls)
                            .await
                    }
                    Err(_) => false,
                };
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

/// Prove the best route: the saved iroh route, the local route, selected
/// direct hints in order, then the relay.
async fn establish(
    device: Arc<Device>,
    locality: Locality,
    local: Option<SocketAddr>,
    iroh: Option<(Arc<Dialer>, IrohRoute)>,
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
    if let Some((dialer, route)) = &iroh {
        match try_iroh(&device, dialer, route, generation).await {
            Ok(link) => return Ok(link),
            Err(Some(blocked)) => return Err(blocked),
            Err(None) => {}
        }
    }
    for (transport, address) in direct_routes(local, &hints) {
        match try_direct(&device, transport, address, generation, tls).await {
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
    Ok(Link::relay_at(device, relay, generation))
}

/// The saved iroh route. `Err(Some)` blocks the attempt; `Err(None)` tries
/// the next route. The handshake is the one TCP runs.
async fn try_iroh(
    device: &Arc<Device>,
    dialer: &Dialer,
    route: &IrohRoute,
    generation: u64,
) -> Result<Link, Option<Failure>> {
    let opened = open_link(dialer, device.clone(), route, generation, HANDSHAKE_TIMEOUT).await;
    handshake_outcome(opened)
}

/// `Err(Some)` blocks the attempt; `Err(None)` tries the next route. A
/// `tcp` route and a `websocket` route run the same handshake.
async fn try_direct(
    device: &Arc<Device>,
    transport: Transport,
    address: String,
    generation: u64,
    tls: &websocket::Tls,
) -> Result<Link, Option<Failure>> {
    let device = device.clone();
    let opened = match transport {
        Transport::Tcp => {
            let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&address))
                .await
                .map_err(|_| None)?
                .map_err(|_| None)?;
            // Calls wait on their answers; see `serve::direct`.
            let _ = stream.set_nodelay(true);
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
    handshake_outcome(opened)
}

/// Whether a handshake's outcome proves the route, blocks the attempt, or
/// moves on to the next route.
fn handshake_outcome(opened: crate::Result<Link>) -> Result<Link, Option<Failure>> {
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
                // The host proved it restarted, and the presence read is
                // its old one. Try again soon with fresh presence rather
                // than settle on a relay route to the old generation, which
                // every terminal opened through it would find lost.
                Refusal::Stale if refusal.detail == GENERATION_DIFFERS => {
                    Some(Failure::Unreachable)
                }
                _ => None,
            })
        }
        Err(_) => Err(None),
    }
}

/// Whether a direct route, the iroh and local routes included, answers
/// now.
async fn direct_answers(
    device: Arc<Device>,
    reach: &Reach,
    locality: Locality,
    local: Option<SocketAddr>,
    iroh: Option<(Arc<Dialer>, IrohRoute)>,
    tls: &websocket::Tls,
) -> bool {
    let generation = reach.presence.presence.generation;
    if let Some((dialer, route)) = &iroh
        && try_iroh(&device, dialer, route, generation).await.is_ok()
    {
        return true;
    }
    let Ok(now) = unix_time() else { return false };
    let Ok(hints) = select(&reach.hints, locality, generation, now) else {
        return false;
    };
    for (transport, address) in direct_routes(local, &hints) {
        if let Ok(link) = try_direct(&device, transport, address, generation, tls).await {
            drop(link);
            return true;
        }
    }
    false
}

/// The direct routes to try, in order: the local route over TCP, then the
/// selected direct hints.
fn direct_routes(local: Option<SocketAddr>, hints: &[&Hint]) -> Vec<(Transport, String)> {
    local
        .map(|address| (Transport::Tcp, address.to_string()))
        .into_iter()
        .chain(
            hints
                .iter()
                .filter(|h| h.is_direct())
                .map(|h| (h.transport, h.address.clone())),
        )
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use coder_reach::hints::{Class, Status};

    #[test]
    fn a_local_route_is_loopback_only_and_tried_first() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let (mut connector, _reports) =
            Connector::new(runtime.handle().clone(), Locality::OtherMachine);
        let host = HostKey::new("ab".repeat(32)).unwrap();
        let tunnel: SocketAddr = "127.0.0.1:40123".parse().unwrap();
        connector.set_local_route(&host, Some(tunnel)).unwrap();
        assert_eq!(connector.local_route(&host), Some(tunnel));
        let lan: SocketAddr = "192.168.1.20:40123".parse().unwrap();
        assert!(connector.set_local_route(&host, Some(lan)).is_err());
        assert_eq!(connector.local_route(&host), Some(tunnel));

        let hint = |class, transport, address: &str| Hint {
            class,
            transport,
            address: address.into(),
            status: Status::Reachable,
            observed_at: 1,
        };
        let hints = [
            hint(Class::Lan, Transport::Tcp, "192.168.1.20:4000"),
            hint(Class::Relay, Transport::Nostr, "wss://relay.example/"),
        ];
        assert_eq!(
            direct_routes(Some(tunnel), &hints.iter().collect::<Vec<_>>()),
            vec![
                (Transport::Tcp, "127.0.0.1:40123".to_owned()),
                (Transport::Tcp, "192.168.1.20:4000".to_owned()),
            ]
        );
        connector.set_local_route(&host, None).unwrap();
        assert_eq!(connector.local_route(&host), None);
        assert_eq!(
            direct_routes(None, &hints.iter().collect::<Vec<_>>()).len(),
            1
        );
    }

    #[test]
    fn a_host_at_another_generation_retries_instead_of_falling_back() {
        let refused = |code, detail| {
            handshake_outcome(Err(Error::Reach(coder_reach::Error::new(code, detail))))
        };
        // A restarted host proved its key at a generation the presence read
        // does not name: retry with fresh presence, not the relay.
        assert!(matches!(
            refused(Refusal::Stale, GENERATION_DIFFERS),
            Err(Some(Failure::Unreachable))
        ));
        // Other refusals before the key is proven move on to the next route.
        assert!(matches!(
            refused(Refusal::Stale, UNAUTHENTICATED),
            Err(None)
        ));
    }
}
