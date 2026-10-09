//! Framed chamber IO with one host-owned world loop, over TLS or, with the
//! `service-reach` feature, a NIP-REACH direct channel. Plaintext is refused.
use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use rustls::ServerConfig;
use tokio::{
    io::AsyncRead,
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    task::JoinSet,
    time::{MissedTickBehavior, timeout},
};
use tokio_rustls::TlsAcceptor;

use super::persistence::writer::{Done, Work, Writer};
use super::{
    auth::{ConnectionId, Gateway},
    persistence::Store,
    wire::{
        Body, Control, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Reply, Request, Response, VERSION,
    },
};
use std::collections::BTreeMap;
#[cfg(test)]
mod operator_tests;
#[cfg(test)]
mod resume_tests;
mod session_pipeline;
mod timing;
pub use timing::{Phases, Timing};

pub(crate) mod admission;
pub use admission::Stats as AdmissionStats;
pub(super) const QUEUE: usize = 128;
pub(super) const REPLY_BYTES: usize = 16 * 1024 * 1024;
const CLOCK_BYTES: usize = 20;
const CONFIRMATION_BYTES: usize = 4096;
/// Loose props a movement confirmation carries, and how near the confirmed
/// character they must be, m.
const DYNAMIC_POSES: usize = 8;
const DYNAMIC_RADIUS: f64 = 16.;
const MAX_HELD_REPLY_BYTES: usize = MAX_RESPONSE_BYTES + CLOCK_BYTES + CONFIRMATION_BYTES;
const HANDSHAKE: Duration = Duration::from_secs(5);
const WRITE: Duration = Duration::from_secs(10);
const AUTH: Duration = Duration::from_secs(30);
const IDLE: Duration = Duration::from_secs(60);
const REQUESTS_PER_SECOND: u32 = 120;

/// Work the trusted host does on the authority's tick, right after the
/// simulation steps, with the step's seconds: a hosted Everglade instance
/// publishes its studio seat poses through it
/// (`crate::social::hosted::StudioFeed`). No wire request reaches it.
pub type Tick = Box<dyn FnMut(&mut Gateway, f32) + Send>;

/// Transport metrics; skipped elapsed time is not silently simulated later.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub admission: AdmissionStats,
    pub accepted_connections: u64,
    pub capacity_refusals: u64,
    pub completed_connections: u64,
    pub requests: u64,
    pub replication: super::replication::Stats,
    pub ticks: u64,
    pub dropped_seconds: f64,
    pub checkpoint_commits: u64,
    pub durable_revision: u64,
    pub checkpoint_bytes: u64,
    pub checkpoint_seconds: f64,
    pub simulation: Timing,
    pub capture: Timing,
    pub read_projection: Timing,
    /// Time from transport submission to authority-loop dispatch, including queue pressure.
    pub movement_queue_wait: Timing,
    pub read_queue_wait: Timing,
    pub checkpoint_copy: Timing,
    pub commits: Timing,
    pub commit_preparation: Timing,
    pub history_sync: Timing,
    pub journal_encoding: Timing,
    pub journal_write: Timing,
    pub journal_sync: Timing,
    pub snapshot_compaction: Timing,
    pub simulation_phases: Phases,
    pub capture_phases: Phases,
    pub commit_phases: Phases,
    pub writer_queue_peak: usize,
    pub request_queue_peak: usize,
    pub held_reply_bytes_peak: usize,
    pub storage_refusals: u64,
    pub storage_paused_ticks: u64,
    pub storage_paused_seconds: f64,
}
/// Retains the authority after shutdown, including runtime failure diagnostics.
pub struct Exit {
    pub gateway: Gateway,
    pub stats: Stats,
    pub failure: Option<String>,
}

type OpenReply = Result<(ConnectionId, Vec<u8>), String>;
type DispatchReply = Result<(Vec<u8>, bool), String>;
struct PendingReply {
    reply: oneshot::Sender<DispatchReply>,
    response: DispatchReply,
}
fn reply_bytes(reply: &DispatchReply) -> usize {
    match reply {
        // Reserve clock growth and a bounded completed-movement confirmation.
        Ok((bytes, _)) => bytes.len() + CLOCK_BYTES + CONFIRMATION_BYTES,
        Err(error) => error.len(),
    }
}
struct CommitView {
    tick: u64,
    instance: u64,
    controls: BTreeMap<ConnectionId, Control>,
    confirmations: BTreeMap<verse_engine::core::LifeId, Vec<crate::movement::Baseline>>,
    dynamic: Vec<super::wire::ColliderPose>,
    authenticated: std::collections::BTreeSet<ConnectionId>,
}
impl CommitView {
    fn capture(gateway: &Gateway) -> Self {
        Self {
            tick: gateway.game().authority_tick,
            instance: gateway.game().player_life().instance,
            confirmations: gateway
                .committed_controls()
                .values()
                .map(|c| {
                    let life = c.life.into();
                    (life, gateway.game().movement_confirmations(life))
                })
                .collect(),
            controls: gateway.committed_controls(),
            dynamic: gateway.game().dynamic_poses(),
            authenticated: gateway.committed_connections(),
        }
    }
    /// Reads only a durable owned clock that cannot regress the delivered prefix.
    fn credit(
        &self,
        id: ConnectionId,
        request_id: u64,
        life: super::wire::Life,
        prefix: &Response,
    ) -> Option<DispatchReply> {
        let control = self.controls.get(&id)?;
        let previous = prefix.control.as_ref()?;
        if !self.authenticated.contains(&id)
            || self.instance != prefix.instance
            || self.tick < prefix.tick
            || control.life != life
            || control.life != previous.life
            || control.epoch != previous.epoch
            || control.accepted_sequence < previous.accepted_sequence
            || control.world_step < previous.world_step
            || control.credit_step < previous.credit_step
        {
            return None;
        }
        Some(
            Response {
                version: VERSION,
                request_id,
                instance: self.instance,
                tick: self.tick,
                control: Some(control.clone()),
                body: Reply::Accepted,
            }
            .encode()
            .map(|bytes| (bytes, true)),
        )
    }
    fn busy(&self, id: ConnectionId, bytes: &[u8]) -> DispatchReply {
        let request = Request::decode(bytes)?;
        let control = self.controls.get(&id).cloned();
        let admitted = self.authenticated.contains(&id);
        Response {
            version: VERSION,
            request_id: request.request_id,
            instance: self.instance,
            tick: self.tick,
            control,
            body: Reply::Refused {
                code: "storage_busy".into(),
                message: "Chamber storage is busy; retry without changing operation identity"
                    .into(),
            },
        }
        .encode()
        .map(|bytes| (bytes, admitted))
    }
}
struct Fence {
    created: Instant,
    view: CommitView,
    replies: Vec<(oneshot::Sender<DispatchReply>, DispatchReply)>,
    reply_bytes: usize,
}
fn finish(
    done: Done,
    fences: &mut BTreeMap<u64, Fence>,
    view: &mut CommitView,
    stats: &mut Stats,
) -> Result<(), String> {
    stats.checkpoint_seconds += done.seconds;
    stats.commits.record(done.seconds);
    for (timing, seconds) in [
        (&mut stats.commit_preparation, done.timings.preparation),
        (&mut stats.history_sync, done.timings.history_sync),
        (&mut stats.journal_encoding, done.timings.journal_encoding),
        (&mut stats.journal_write, done.timings.journal_write),
        (&mut stats.journal_sync, done.timings.journal_sync),
        (
            &mut stats.snapshot_compaction,
            done.timings.snapshot_compaction,
        ),
    ] {
        if let Some(seconds) = seconds {
            timing.record(seconds);
        }
    }
    stats.commit_phases.record(done.seconds);
    let committed = done.result?;
    stats.durable_revision = committed.revision;
    if committed.written {
        stats.checkpoint_commits += 1;
        stats.checkpoint_bytes += committed.bytes as u64;
    }
    if fences.keys().next().copied() != Some(done.token) {
        return Err("Chamber storage completion order is incompatible".into());
    }
    let fence = fences
        .remove(&done.token)
        .ok_or("Chamber storage completion has no fence")?;
    *view = fence.view;
    for (reply, result) in fence.replies {
        let _ = reply.send(committed_movement(
            committed_control_credit(result, &view.controls),
            &view.confirmations,
            &view.dynamic,
        ));
    }
    Ok(())
}
/// A later checkpoint grants credit only to its matching admitted control context.
fn committed_control_credit(
    result: DispatchReply,
    controls: &BTreeMap<ConnectionId, Control>,
) -> DispatchReply {
    let Ok((bytes, _)) = &result else {
        return result;
    };
    let body = bytes
        .windows(7)
        .position(|part| part == b"\"body\":")
        .ok_or("Committed response has no body")?;
    #[derive(serde::Deserialize)]
    struct Header {
        control: Option<Control>,
    }
    let mut header = bytes[..body].to_vec();
    header.extend_from_slice(b"\"body\":null}");
    let Header { control } =
        serde_json::from_slice(&header).map_err(|_| "Committed response header is malformed")?;
    let Some(old) = control else {
        return result;
    };
    let Some(current) = controls
        .values()
        .find(|c| c.life == old.life && c.epoch == old.epoch)
    else {
        return result;
    };
    committed_world_credit(result, current.credit_step)
}
/// Renews only the clock from the completed durable fence. Admission identity,
/// sequence, tick, and projected state retain their original ordered prefix.
fn committed_world_credit(mut result: DispatchReply, world_step: u64) -> DispatchReply {
    let Ok((bytes, _)) = &mut result else {
        return result;
    };
    // These bytes come from Response::encode. Its typed numeric header precedes
    // the body; never search arbitrary body strings or decode the projected scene.
    let body = bytes
        .windows(7)
        .position(|part| part == b"\"body\":")
        .ok_or("Committed response has no body")?;
    let marker = b"\"credit_step\":";
    let Some(start) = bytes[..body]
        .windows(marker.len())
        .position(|part| part == marker)
        .map(|offset| offset + marker.len())
    else {
        return result;
    };
    let end = start
        + bytes[start..body]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
    let previous: u64 = std::str::from_utf8(&bytes[start..end])
        .map_err(|_| "Committed response clock is malformed")?
        .parse()
        .map_err(|_| "Committed response clock is malformed")?;
    if previous > world_step {
        return Err("Committed response clock exceeds its durable fence".into());
    }
    let clock = world_step.to_string();
    let length = bytes.len() - (end - start) + clock.len();
    if length > MAX_RESPONSE_BYTES {
        return Err("Committed response exceeds byte budget".into());
    }
    bytes.splice(start..end, clock.bytes());
    result
}
/// Adds only confirmed travel within the response's original admission prefix.
/// Decoding the small header avoids copying or rebuilding the projected scene.
fn committed_movement(
    mut result: DispatchReply,
    histories: &BTreeMap<verse_engine::core::LifeId, Vec<crate::movement::Baseline>>,
    dynamic: &[super::wire::ColliderPose],
) -> DispatchReply {
    let Ok((bytes, _)) = &mut result else {
        return result;
    };
    let body = bytes
        .windows(7)
        .position(|part| part == b"\"body\":")
        .ok_or("Committed response has no body")?;
    #[derive(serde::Deserialize)]
    struct Header {
        control: Option<Control>,
    }
    let mut header = bytes[..body].to_vec();
    header.extend_from_slice(b"\"body\":null}");
    let Header { control } =
        serde_json::from_slice(&header).map_err(|_| "Committed response header is malformed")?;
    let Some(mut control) = control else {
        return result;
    };
    let Some(baseline) = histories.get(&control.life.into()).and_then(|history| {
        history.iter().rev().find(|b| {
            b.life == control.life.into()
                && b.epoch == control.epoch
                && b.applied_sequence <= control.accepted_sequence
                && b.world_step <= control.credit_step
        })
    }) else {
        return result;
    };
    baseline.validate()?;
    control.applied_movement = Some(*baseline);
    // The nearest loose props, within reach of the replayed travel.
    let feet = baseline.character.feet;
    let mut near: Vec<_> = dynamic
        .iter()
        .filter(|p| p.pose.position.distance(feet) <= DYNAMIC_RADIUS)
        .copied()
        .collect();
    near.sort_by(|a, b| {
        a.pose
            .position
            .distance_squared(feet)
            .total_cmp(&b.pose.position.distance_squared(feet))
            .then(a.key.cmp(&b.key))
    });
    near.truncate(DYNAMIC_POSES);
    control.dynamic = near;
    let encoded =
        serde_json::to_vec(&control).map_err(|_| "Cannot encode movement confirmation")?;
    if encoded.len() > CONFIRMATION_BYTES {
        return Err("Movement confirmation exceeds byte budget".into());
    }
    let marker = b"\"control\":";
    let start = bytes[..body]
        .windows(marker.len())
        .position(|part| part == marker)
        .ok_or("Committed response has no control header")?
        + marker.len();
    let end = body
        .checked_sub(1)
        .ok_or("Committed response header is malformed")?;
    if bytes.get(end) != Some(&b',')
        || bytes.len() - (end - start) + encoded.len() > MAX_RESPONSE_BYTES
    {
        return Err("Committed response exceeds byte budget".into());
    }
    bytes.splice(start..end, encoded);
    result
}

/// An ordered, authenticated byte stream that carries chamber frames.
pub use super::transport::{Transport, read_frame, write_frame};

/// How accepted sockets become authenticated chamber transports.
pub(super) enum Listen {
    Tls(TlsAcceptor),
    #[cfg(feature = "service-reach")]
    Reach(Arc<dyn super::reach::Admit>),
}

/// Checks one connection's standing before each request it sends.
pub(super) trait Guard: Send + Sync {
    /// Refuses a request the connection's admission no longer allows.
    fn admit(&self, request: &[u8]) -> Result<(), String>;
    /// The identity key the transport authenticated, when it binds one.
    fn device(&self) -> Option<[u8; 32]>;
}

pub(super) enum RequestProgress {
    Queued { authenticated: bool },
    Busy,
    Failed(String),
}
/// Internal admission progress never acknowledges durability to a peer.
pub(super) fn dispatch_progress(
    progress: Option<oneshot::Sender<RequestProgress>>,
    result: &DispatchReply,
) {
    if let Some(progress) = progress {
        let state = match result {
            Ok((_, authenticated)) => RequestProgress::Queued {
                authenticated: *authenticated,
            },
            Err(error) => RequestProgress::Failed(error.clone()),
        };
        let _ = progress.send(state);
    }
}
fn busy_progress(progress: Option<oneshot::Sender<RequestProgress>>) {
    if let Some(progress) = progress {
        let _ = progress.send(RequestProgress::Busy);
    }
}
pub(super) enum Event {
    Open {
        /// A transport-admitted key the chamber does not yet know joins as a
        /// spectator before its challenge is issued.
        spectate: Option<[u8; 32]>,
        reply: oneshot::Sender<OpenReply>,
    },
    Request {
        id: ConnectionId,
        bytes: Vec<u8>,
        reply: oneshot::Sender<DispatchReply>,
        progress: Option<oneshot::Sender<RequestProgress>>,
        delivered_prefix: Option<Response>,
        queued_at: Instant,
    },
    Close(ConnectionId),
}

/// Runs an already-bound listener with a configured certificate and private key.
///
/// The caller supplies TLS identity and enrolled world rights. This function
/// never accepts plaintext, loads credentials, or installs a host service.
pub async fn serve<F: Future<Output = ()>>(
    listener: TcpListener,
    tls: Arc<ServerConfig>,
    gateway: Gateway,
    shutdown: F,
) -> Exit {
    let listen = Listen::Tls(TlsAcceptor::from(tls));
    serve_with_store(listener, listen, gateway, None, None, shutdown).await
}
/// Commits world mutations before replies and stops on any durability failure.
pub async fn serve_durable<F: Future<Output = ()>>(
    listener: TcpListener,
    tls: Arc<ServerConfig>,
    gateway: Gateway,
    store: Store,
    shutdown: F,
) -> Exit {
    let listen = Listen::Tls(TlsAcceptor::from(tls));
    serve_with_store(listener, listen, gateway, Some(store), None, shutdown).await
}
/// [`serve`] or, with `store`, [`serve_durable`], running `tick` on every
/// authority tick after the simulation steps.
pub async fn serve_ticked<F: Future<Output = ()>>(
    listener: TcpListener,
    tls: Arc<ServerConfig>,
    gateway: Gateway,
    store: Option<Store>,
    tick: Tick,
    shutdown: F,
) -> Exit {
    let listen = Listen::Tls(TlsAcceptor::from(tls));
    serve_with_store(listener, listen, gateway, store, Some(tick), shutdown).await
}
/// Runs TLS with bounded live diagnostics and an operator-requested ordered drain.
pub async fn serve_monitored<F: Future<Output = ()>>(
    listener: TcpListener,
    tls: Arc<ServerConfig>,
    gateway: Gateway,
    store: Option<Store>,
    tick: Option<Tick>,
    monitor: super::operator::Monitor,
    shutdown: F,
) -> Exit {
    serve_observed(
        listener,
        Listen::Tls(TlsAcceptor::from(tls)),
        gateway,
        store,
        tick,
        Some(monitor),
        shutdown,
    )
    .await
}
pub(super) async fn serve_with_store<F: Future<Output = ()>>(
    listener: TcpListener,
    listen: Listen,
    gateway: Gateway,
    store: Option<Store>,
    hook: Option<Tick>,
    shutdown: F,
) -> Exit {
    serve_observed(listener, listen, gateway, store, hook, None, shutdown).await
}
async fn serve_observed<F: Future<Output = ()>>(
    listener: TcpListener,
    listen: Listen,
    gateway: Gateway,
    store: Option<Store>,
    hook: Option<Tick>,
    monitor: Option<super::operator::Monitor>,
    shutdown: F,
) -> Exit {
    let start = Instant::now();
    if let Some(m) = &monitor {
        let snapshot = m.snapshot();
        if snapshot.instance != gateway.game().player_life().instance
            || snapshot.content != gateway.content()
        {
            let exit = Exit {
                gateway,
                stats: Stats::default(),
                failure: Some("Operator context is incompatible".into()),
            };
            m.terminal(&exit, start);
            return exit;
        }
    }
    let exit = serve_loop(
        listener,
        listen,
        gateway,
        store,
        hook,
        monitor.as_ref(),
        shutdown,
    )
    .await;
    if let Some(m) = monitor {
        m.terminal(&exit, start);
    }
    exit
}
/// Whether the loop can admit another request into the next durable fence.
fn admission_room(
    writer: Option<&Writer>,
    pending: usize,
    fences: usize,
    pending_bytes: usize,
) -> bool {
    writer.is_none_or(|writer| {
        pending < QUEUE
            && fences < 2
            && pending_bytes <= REPLY_BYTES - MAX_HELD_REPLY_BYTES
            && writer.send.as_ref().unwrap().capacity() > 0
    })
}
async fn serve_loop<F: Future<Output = ()>>(
    listener: TcpListener,
    listen: Listen,
    mut gateway: Gateway,
    store: Option<Store>,
    mut hook: Option<Tick>,
    monitor: Option<&super::operator::Monitor>,
    shutdown: F,
) -> Exit {
    let mut stats = Stats::default();
    let mut failure = None;
    let mut writer = if let Some(mut store) = store {
        let start = Instant::now();
        let prepared = match store.prepare(&mut gateway) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Exit {
                    gateway,
                    stats,
                    failure: Some(error),
                };
            }
        };
        let seconds = start.elapsed().as_secs_f64();
        stats.capture.record(seconds);
        stats.capture_phases.record(seconds);
        let start = Instant::now();
        let initial = tokio::task::spawn_blocking(move || {
            let result = store.commit_prepared(prepared);
            (store, result)
        })
        .await;
        let (store, result) = match initial {
            Ok(initial) => initial,
            Err(_) => {
                return Exit {
                    gateway,
                    stats,
                    failure: Some("Chamber storage initialization failed".into()),
                };
            }
        };
        let seconds = start.elapsed().as_secs_f64();
        stats.checkpoint_seconds += seconds;
        stats.commits.record(seconds);
        stats.commit_phases.record(seconds);
        match result {
            Ok(commit) => {
                stats.durable_revision = commit.revision;
                if commit.written {
                    stats.checkpoint_commits += 1;
                    stats.checkpoint_bytes += commit.bytes as u64;
                }
            }
            Err(error) => {
                return Exit {
                    gateway,
                    stats,
                    failure: Some(error),
                };
            }
        }
        match Writer::start(store) {
            Ok(writer) => Some(writer),
            Err(error) => {
                return Exit {
                    gateway,
                    stats,
                    failure: Some(error),
                };
            }
        }
    } else {
        if let Err(error) = super::rewards::history::History::temporary()
            .and_then(|archive| gateway.chamber.rewards.attach(archive))
        {
            return Exit {
                gateway,
                stats,
                failure: Some(error),
            };
        }
        None
    };
    let limits = admission::Limits::new();
    let (send, mut receive) = mpsc::channel(QUEUE);
    let mut workers = JoinSet::new();
    let start = Instant::now();
    let mut last_tick = start;
    let mut schedule = verse_engine::core::FixedSchedule::new(30, 3).expect("fixed host schedule");
    let period = Duration::from_secs_f64(1. / 30.);
    let mut ticker = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut pending: Vec<PendingReply> = Vec::with_capacity(QUEUE);
    let mut pending_bytes = 0usize;
    let mut fences = BTreeMap::new();
    let mut committed = CommitView::capture(&gateway);
    let mut token = 0u64;
    let mut dirty = false;
    let mut checkpoint_ticks = 0u64;
    let mut diagnostics = tokio::time::interval(Duration::from_secs(1));
    diagnostics.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut storage_paused = false;
    let mut resume_requests = 0usize;
    let mut deferred_tick = None;
    tokio::pin!(shutdown);
    loop {
        stats.request_queue_peak = stats.request_queue_peak.max(receive.len());
        stats.held_reply_bytes_peak = stats.held_reply_bytes_peak.max(
            pending_bytes
                + fences
                    .values()
                    .map(|fence: &Fence| fence.reply_bytes)
                    .sum::<usize>(),
        );
        let request_room =
            admission_room(writer.as_ref(), pending.len(), fences.len(), pending_bytes);
        // A checkpoint handed to the writer fills its one-slot channel until the
        // writer thread dequeues it. Nothing else wakes the loop then, so watch
        // the slot: queued requests must not wait for the next deadline.
        let handoff = writer
            .as_ref()
            .and_then(|writer| writer.send.as_ref())
            .filter(|send| fences.len() < 2 && send.capacity() == 0)
            .cloned();
        tokio::select! {
            _ = &mut shutdown => break,
            _ = async { monitor.unwrap().draining().await }, if monitor.is_some() => break,
            _ = diagnostics.tick(), if monitor.is_some() => {
                stats.admission = limits.stats();
                let held = pending_bytes + fences.values().map(|f: &Fence| f.reply_bytes).sum::<usize>();
                monitor.unwrap().running(&gateway, &stats, start, receive.len(), fences.len(), held,
                    fences.first_key_value().map(|(_, f)| f.created.elapsed()),
                    gateway.chamber.rewards.history_capacity().unwrap_or(false), limits.clients(gateway.game().authority_tick));
            },
            completed = async { writer.as_mut().unwrap().done.recv().await }, if writer.is_some() && !fences.is_empty() => {
                let result = completed.ok_or_else(|| "Chamber storage writer stopped".to_string())
                    .and_then(|done| finish(done, &mut fences, &mut committed, &mut stats));
                if let Err(error) = result {failure = Some(error); break;}
                if storage_paused {
                    // Admit the bounded FIFO cohort that arrived during the pause
                    // before resumed time can retire its movement clocks.
                    resume_requests = resume_requests.max(receive.len());
                    storage_paused = false;
                }
            }
            freed = async { handoff.as_ref().unwrap().reserve().await.map(drop) }, if handoff.is_some() => {
                if freed.is_err() {failure = Some("Chamber storage writer stopped".into()); break;}
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((socket, address)) => {
                        let Ok(slot) = limits.open(address.ip()) else {
                            stats.capacity_refusals += 1;
                            continue;
                        };
                        stats.accepted_connections += 1;
                        let send = send.clone();
                        let metrics = limits.clone();
                        match &listen {
                            Listen::Tls(acceptor) => {
                                let acceptor = acceptor.clone();
                                workers.spawn(async move {
                                    let result = connection(socket, acceptor, send, slot).await;
                                    metrics.finish(&result);
                                });
                            }
                            #[cfg(feature = "service-reach")]
                            Listen::Reach(admit) => {
                                let admit = admit.clone();
                                workers.spawn(async move {
                                    let result = super::reach::connection(socket, admit, send, slot).await;
                                    metrics.finish(&result);
                                });
                            }
                        }
                    }
                    Err(_) => {failure = Some("Chamber listener failed".into()); break;}
                }
            }
            // If reply or writer bounds block admission, a tick must still flush
            // admitted state. New arrivals cannot extend the captured FIFO cohort.
            _ = async {
                if deferred_tick.is_none() || resume_requests > 0 {
                    ticker.tick().await;
                }
            }, if resume_requests == 0 || !request_room => {
                let now = Instant::now();
                let wall_elapsed = now.duration_since(last_tick).as_secs_f64();
                last_tick = now;
                let elapsed = deferred_tick.unwrap_or(0.) + wall_elapsed;
                let room = writer.as_ref().is_none_or(|writer| fences.len() < 2 && writer.send.as_ref().unwrap().capacity() > 0);
                // Re-read admission room: the writer may have dequeued a
                // checkpoint since the loop began waiting. A stale refusal here
                // would let this deadline overtake already queued movement.
                let request_room = admission_room(writer.as_ref(), pending.len(), fences.len(), pending_bytes);
                let history = match gateway.chamber.rewards.history_capacity() {
                    Ok(available) => available,
                    Err(error) => {failure = Some(error); break;}
                };
                if !room {
                    storage_paused = true;
                    stats.storage_paused_ticks += 1;
                    stats.storage_paused_seconds += wall_elapsed;
                    continue;
                }
                if deferred_tick.is_none() && history && request_room
                    && resume_requests == 0
                    && !receive.is_empty()
                {
                    // A simulation deadline can become ready alongside valid
                    // movement already waiting. Admit that fixed FIFO
                    // cohort first, then resume this same elapsed-time batch.
                    // Later arrivals cannot extend the cohort or starve time.
                    resume_requests = receive.len();
                    deferred_tick = Some(elapsed);
                    continue;
                }
                if history && resume_requests == 0 {
                    deferred_tick = None;
                    let batch = match schedule.advance(elapsed) {
                        Ok(batch) => batch,
                        Err(error) => {failure = Some(error); break;}
                    };
                    stats.dropped_seconds += batch.dropped_seconds;
                    for _ in 0..batch.steps {
                        let tick = Instant::now();
                        if let Err(error) = gateway.tick(batch.seconds) {failure = Some(error); break;}
                        if let Some(hook) = hook.as_mut() { hook(&mut gateway, batch.seconds); }
                        let seconds = tick.elapsed().as_secs_f64();
                        stats.simulation.record(seconds);
                        stats.simulation_phases.record(seconds);
                        stats.ticks += 1;
                        if writer.is_some() {
                            checkpoint_ticks = checkpoint_ticks.saturating_add(1);
                            dirty = true;
                        }
                    }
                    if failure.is_some() { break; }
                } else {
                    // Flush already admitted state without adding more simulation mutations.
                    stats.storage_paused_ticks += 1;
                    stats.storage_paused_seconds += wall_elapsed;
                    if !history { deferred_tick = None; }
                }
                if let Some(writer) = &mut writer {
                    // Adjacent ticks share one immutable checkpoint. Pressure still
                    // flushes immediately, and replies wait for the durable prefix.
                    if checkpoint_ticks < 2 && history && request_room {
                        continue;
                    }
                    let permit = match writer.send.as_ref().unwrap().try_reserve() {
                        Ok(permit) => permit,
                        Err(_) => {
                            failure = Some(match writer.done.recv().await {
                                Some(done) => finish(done, &mut fences, &mut committed, &mut stats).err()
                                    .unwrap_or_else(|| "Chamber storage writer stopped".into()),
                                None => "Chamber storage writer stopped".into(),
                            });
                            break;
                        }
                    };
                    let capture = Instant::now();
                    let replies = pending.drain(..).map(|pending|
                        (pending.reply, pending.response)).collect();
                    let reply_bytes = std::mem::take(&mut pending_bytes);
                    let copy = Instant::now();
                    let prepared = match super::save::Prepared::capture(&gateway) {
                        Ok(prepared) => prepared,
                        Err(error) => {failure = Some(error); break;}
                    };
                    stats.checkpoint_copy.record(copy.elapsed().as_secs_f64());
                    token = match token.checked_add(1) {
                        Some(token) => token,
                        None => {failure = Some("Chamber storage tokens exhausted".into()); break;}
                    };
                    fences.insert(token, Fence {created: Instant::now(), view:CommitView::capture(&gateway), replies, reply_bytes});
                    let seconds = capture.elapsed().as_secs_f64();
                    stats.capture.record(seconds);
                    stats.capture_phases.record(seconds);
                    stats.writer_queue_peak = stats.writer_queue_peak.max(fences.len());
                    permit.send(Work {token, prepared});
                    checkpoint_ticks = 0;
                    dirty = false;
                }
            }
            // Leave requests in the bounded transport queue while both persistence
            // slots are occupied. Completion wakes this loop without a retry tick.
            event = receive.recv(), if request_room => {
                resume_requests = resume_requests.saturating_sub(1);
                let now = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                match event {
                    Some(Event::Open {spectate, reply}) => {
                        let joined = match spectate {
                            Some(key) => gateway.admit_spectator(key),
                            None => Ok(false),
                        };
                        dirty |= joined.as_ref().is_ok_and(|joined| *joined);
                        let result = joined.and_then(|_| gateway.open_json(now));
                        if let Err(Ok((id, _))) = reply.send(result) {let _ = gateway.close(id);}
                    }
                    Some(Event::Request {id, bytes, reply, progress, delivered_prefix, queued_at}) => {
                        stats.requests += 1;
                        if let Ok(request) = Request::decode(&bytes) {
                            let timing = match request.body {
                                Body::BeginMovementFrames {..} | Body::MovementFrame {..} => Some(&mut stats.movement_queue_wait),
                                Body::Snapshot {} | Body::Replicate {..} | Body::MovementCredit {} => Some(&mut stats.read_queue_wait),
                                _ => None,
                            };
                            if let Some(timing) = timing { timing.record(queued_at.elapsed().as_secs_f64()); }
                        }
                        if let Some(writer) = &writer {
                            if let Some(prefix) = delivered_prefix.as_ref() {
                                if let Ok(request) = Request::decode(&bytes) {
                                    if matches!(request.body, Body::MovementCredit {}) {
                                        if let Ok(admission) = gateway.admission(id) {
                                            if let Some(result) = committed.credit(id, request.request_id, admission.actor().into(), prefix) {
                                                dispatch_progress(progress, &result);
                                                let _ = reply.send(result);
                                                continue;
                                            }
                                        }
                                    }
                                }
                            }
                            let room = pending.len() < QUEUE && fences.len() < 2 && writer.send.as_ref().unwrap().capacity() > 0;
                            let history = gateway.chamber.rewards.history_capacity().unwrap_or(false);
                            let mutating = Request::decode(&bytes).is_ok_and(|request| matches!(request.body,
                                Body::Authenticate {..} | Body::Social {..} | Body::BeginMovementFrames {..} | Body::MovementFrame {..} | Body::Command {..} | Body::Respawn {..} | Body::ClaimQuest {..} | Body::QuestCycle {..}
                                | Body::AcceptQuest {..} | Body::UseItem {..} | Body::EquipOutfit {..} | Body::EquipGear {..}));
                            if !mutating && !dirty {
                                if let Some(mut entry) = fences.last_entry() {
                                    let fence = entry.get_mut();
                                    if fence.replies.len() >= QUEUE
                                        || fence.reply_bytes > REPLY_BYTES - MAX_HELD_REPLY_BYTES {
                                        stats.storage_refusals += 1;
                                        busy_progress(progress);
                                        let _ = reply.send(committed.busy(id, &bytes));
                                    } else {
                                        let result = gateway.dispatch_json(id, now, &bytes)
                                            .map(|bytes| (bytes, gateway.authenticated(id)));
                                        dispatch_progress(progress, &result);
                                        fence.reply_bytes += reply_bytes(&result);
                                        fence.replies.push((reply, result));
                                    }
                                } else {
                                    // The current authority state is already committed.
                                    let result = gateway.dispatch_json(id, now, &bytes)
                                        .map(|bytes| (bytes, gateway.authenticated(id)));
                                    dispatch_progress(progress, &result);
                                    let _ = reply.send(committed_movement(result, &committed.confirmations, &committed.dynamic));
                                }
                                continue;
                            }
                            if !room || !history {
                                stats.storage_refusals += 1;
                                busy_progress(progress);
                                let _ = reply.send(committed.busy(id, &bytes));
                                continue;
                            }
                            dirty |= mutating;
                            // Capture this ordered prefix before admitting later input.
                            // Admission progress is immediate; delivery still waits for storage.
                            let projection = Instant::now();
                            let response = gateway.dispatch_json(id, now, &bytes)
                                .map(|bytes| (bytes, gateway.authenticated(id)));
                            if !mutating { stats.read_projection.record(projection.elapsed().as_secs_f64()); }
                            dispatch_progress(progress, &response);
                            pending_bytes += reply_bytes(&response);
                            pending.push(PendingReply {reply, response});
                        } else {
                            let result = gateway.dispatch_json(id, now, &bytes).map(|bytes| (bytes, gateway.authenticated(id)));
                            dispatch_progress(progress, &result);
                            let histories = gateway.admission(id).ok().map(|a| {
                                let life = a.actor();
                                BTreeMap::from([(life, gateway.game().movement_confirmations(life))])
                            }).unwrap_or_default();
                            let dynamic = gateway.game().dynamic_poses();
                            let _ = reply.send(committed_movement(result, &histories, &dynamic));
                        }
                    }
                    Some(Event::Close(id)) => {let _ = gateway.close(id); dirty = true;}
                    None => {failure = Some("Chamber dispatch queue closed".into()); break;}
                }
            }
            result = workers.join_next(), if !workers.is_empty() => {
                stats.completed_connections += 1;
                if result.is_some_and(|r| r.is_err()) { limits.cancelled(); }
            }
        }
    }
    if let Some(m) = monitor {
        m.phase(
            super::operator::Phase::Draining,
            super::operator::Reason::Draining,
            start,
        );
    }
    workers.abort_all();
    while let Some(result) = workers.join_next().await {
        stats.completed_connections += 1;
        if result.is_err() {
            limits.cancelled();
        }
    }
    if let Some(writer) = &mut writer {
        while failure.is_none() && !fences.is_empty() {
            let result = writer
                .done
                .recv()
                .await
                .ok_or_else(|| "Chamber storage writer stopped".to_string())
                .and_then(|done| finish(done, &mut fences, &mut committed, &mut stats));
            if let Err(error) = result {
                failure = Some(error);
            }
        }
    }
    if let Err(error) = gateway.close_all() {
        failure.get_or_insert(error);
    }
    if failure.is_none() {
        if let Some(writer) = &mut writer {
            let capture = Instant::now();
            match super::save::Prepared::capture(&gateway) {
                Ok(prepared) => {
                    token = match token.checked_add(1) {
                        Some(token) => token,
                        None => {
                            return Exit {
                                gateway,
                                stats,
                                failure: Some("Chamber storage tokens exhausted".into()),
                            };
                        }
                    };
                    fences.insert(
                        token,
                        Fence {
                            created: Instant::now(),
                            view: CommitView::capture(&gateway),
                            replies: vec![],
                            reply_bytes: 0,
                        },
                    );
                    let seconds = capture.elapsed().as_secs_f64();
                    stats.capture.record(seconds);
                    stats.capture_phases.record(seconds);
                    if writer
                        .send
                        .as_ref()
                        .unwrap()
                        .send(Work { token, prepared })
                        .await
                        .is_err()
                    {
                        failure = Some("Chamber storage writer stopped during drain".into());
                    } else {
                        let result = writer
                            .done
                            .recv()
                            .await
                            .ok_or_else(|| {
                                "Chamber storage writer stopped during drain".to_string()
                            })
                            .and_then(|done| finish(done, &mut fences, &mut committed, &mut stats));
                        if let Err(error) = result {
                            failure = Some(error);
                        }
                    }
                }
                Err(error) => {
                    failure = Some(error);
                }
            }
        }
    }
    drop(writer);
    stats.replication = gateway.replication_stats();
    stats.admission = limits.stats();
    Exit {
        gateway,
        stats,
        failure,
    }
}

pub(in crate::service) async fn connection(
    socket: TcpStream,
    acceptor: TlsAcceptor,
    send: mpsc::Sender<Event>,
    slot: admission::Slot,
) -> Result<(), String> {
    socket
        .set_nodelay(true)
        .map_err(|_| "Cannot configure chamber socket")?;
    let stream = timeout(HANDSHAKE, acceptor.accept(socket))
        .await
        .map_err(|_| "Chamber TLS handshake timed out")?
        .map_err(|_| "Chamber TLS handshake refused")?;
    session(stream, None, send, slot).await
}

// Retain only authority headers for local refusals; projected state can be large.
#[derive(serde::Deserialize)]
struct ResponseHeader {
    version: u16,
    request_id: u64,
    instance: u64,
    tick: u64,
    control: Option<Control>,
    body: ReplyHeader,
}
#[derive(serde::Deserialize)]
struct ReplyHeader {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    code: Option<String>,
}
impl ResponseHeader {
    fn refusal_template(self) -> Response {
        Response {
            version: self.version,
            request_id: self.request_id,
            instance: self.instance,
            tick: self.tick,
            control: self.control,
            body: Reply::Accepted,
        }
    }
}

/// Serves chamber frames on one authenticated transport until it closes.
pub(super) async fn session<S: Transport + 'static>(
    stream: S,
    guard: Option<Box<dyn Guard>>,
    send: mpsc::Sender<Event>,
    slot: admission::Slot,
) -> Result<(), String> {
    session_until(
        stream,
        guard,
        send,
        slot,
        tokio::time::Instant::now() + AUTH,
    )
    .await
}
async fn session_until<S: Transport + 'static>(
    mut stream: S,
    guard: Option<Box<dyn Guard>>,
    send: mpsc::Sender<Event>,
    mut slot: admission::Slot,
    authentication_deadline: tokio::time::Instant,
) -> Result<(), String> {
    let (reply, receive) = oneshot::channel();
    let spectate = guard.as_ref().and_then(|guard| guard.device());
    send.send(Event::Open { spectate, reply })
        .await
        .map_err(|_| "Chamber host stopped")?;
    let (id, hello) = receive.await.map_err(|_| "Chamber host stopped")??;
    let outcome = async {
        timeout(WRITE, write_frame(&mut stream, &hello, MAX_RESPONSE_BYTES))
            .await
            .map_err(|_| "Chamber write timed out")??;
        slot.delivered(hello.len(), None);
        let mut authenticated = false;
        let mut last_response: Option<Response> = None;
        let mut window = Instant::now();
        let mut count = 0;
        loop {
            if slot.retired() { return Err("Chamber connection was superseded".into()); }
            let deadline = if authenticated { tokio::time::Instant::now() + IDLE } else { authentication_deadline };
            let bytes = tokio::select! {
                _ = slot.cancelled() => return Err("Chamber connection was superseded".into()),
                read = tokio::time::timeout_at(deadline, read_frame(&mut stream, MAX_REQUEST_BYTES)) =>
                    read.map_err(|_| if authenticated { "Chamber read timed out" } else { "Chamber authentication timed out" })??,
            };
            slot.received(bytes.len());
            if window.elapsed() >= Duration::from_secs(1) {
                window = Instant::now();
                count = 0;
            }
            count += 1;
            if count > REQUESTS_PER_SECOND {
                return Err("Chamber request rate exceeded".into());
            }
            if let Some(guard) = &guard {
                guard.admit(&bytes)?;
            }
            let request = Request::decode(&bytes)?;
            if !authenticated && !matches!(request.body, Body::Authenticate {..}) {
                return Err("Chamber connection is not authenticated".into());
            }
            if authenticated && !slot.request(&request.body) {
                let mut response = last_response.clone().ok_or("Chamber admission has no response")?;
                response.request_id = request.request_id;
                response.body = Reply::Refused {
                    code: "rate_limited".into(),
                    message: "Chamber request work budget exceeded; no operation was admitted".into(),
                };
                let refused = response.encode()?;
                timeout(WRITE, write_frame(&mut stream, &refused, MAX_RESPONSE_BYTES)).await
                    .map_err(|_| "Chamber write timed out")??;
                slot.delivered(refused.len(), Some(response.tick));
                continue;
            }
            let wait_for_storage = movement_storage_wait(authenticated, &request.body);
            let authenticate_key = match request.body { Body::Authenticate { public_key, .. } => Some(public_key), _ => None };
            let (bytes, admitted) = request_with_storage_backpressure(&send, id, bytes, wait_for_storage).await?;
            let response: ResponseHeader = serde_json::from_slice(&bytes).map_err(|_| "Invalid chamber response")?;
            let retry_admission = response.body.kind == "refused" && response.body.code.as_deref() == Some("storage_busy");
            if admitted && !authenticated && response.body.kind == "accepted" {
                slot.authenticate(authenticate_key.ok_or("Chamber authentication key unavailable")?)?;
            }
            let delivered_tick = response.tick;
            last_response = Some(response.refusal_template());
            timeout(WRITE, write_frame(&mut stream, &bytes, MAX_RESPONSE_BYTES))
                .await
                .map_err(|_| "Chamber write timed out")??;
            slot.delivered(bytes.len(), Some(delivered_tick));
            authenticated = admitted;
            if authenticated {
                return session_pipeline::run(stream, &guard, &send, &mut slot, id,
                    last_response.take().unwrap(), window, count).await;
            }
            if !authenticated && !retry_admission {
                return Err("Chamber connection is not authenticated".into());
            }
        }
    }
    .await;
    let _ = send.send(Event::Close(id)).await;
    outcome
}

// Hold only explicitly unadmitted intervals, preserving their bytes and connection order.
// An IO failure or any other refusal is never retried here.
fn movement_storage_wait(authenticated: bool, body: &Body) -> bool {
    authenticated
        && matches!(
            body,
            Body::BeginMovementFrames { .. } | Body::MovementFrame { .. }
        )
}

async fn request_with_storage_backpressure(
    send: &mpsc::Sender<Event>,
    id: ConnectionId,
    bytes: Vec<u8>,
    wait_for_storage: bool,
) -> DispatchReply {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let (reply, receive) = oneshot::channel();
        send.send(Event::Request {
            id,
            bytes: bytes.clone(),
            reply,
            progress: None,
            delivered_prefix: None,
            queued_at: Instant::now(),
        })
        .await
        .map_err(|_| "Chamber host stopped")?;
        let result = receive.await.map_err(|_| "Chamber host stopped")??;
        let response: ResponseHeader =
            serde_json::from_slice(&result.0).map_err(|_| "Invalid chamber response")?;
        if !wait_for_storage
            || !result.1
            || response.body.kind != "refused"
            || response.body.code.as_deref() != Some("storage_busy")
            || tokio::time::Instant::now() >= deadline
        {
            return Ok(result);
        }
        tokio::time::sleep(Duration::from_millis(33)).await;
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::{
        play::Game,
        service::{
            Chamber,
            wire::{Action, Body, Hello, Input, Reply, Request, Response, State, VERSION},
        },
    };
    use glam::Vec3;
    use rustls::{
        ClientConfig, RootCertStore,
        pki_types::{PrivatePkcs8KeyDer, ServerName},
    };
    use secp256k1::{Keypair, Secp256k1, SecretKey};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_rustls::{TlsConnector, client::TlsStream};
    use verse_engine::director::Scene;
    type Stream = TlsStream<TcpStream>;
    #[test]
    fn committed_credit_reads_cannot_regress_or_cross_owned_control() {
        let keys = [key(171), key(172), key(173)];
        let (id, _) = gateway(&keys).open_json(0).unwrap();
        let life = super::super::wire::Life {
            instance: 120,
            actor: 14,
            generation: 2,
        };
        let control = Control {
            life,
            epoch: 3,
            accepted_sequence: 7,
            world_step: 12,
            credit_step: 16,
            applied_movement: None,
            dynamic: Vec::new(),
        };
        let prefix = Response {
            version: VERSION,
            request_id: 23,
            instance: 120,
            tick: 4,
            control: Some(control.clone()),
            body: Reply::Accepted,
        };
        let mut view = CommitView {
            tick: 5,
            instance: 120,
            controls: BTreeMap::from([(id, control)]),
            confirmations: BTreeMap::new(),
            dynamic: Vec::new(),
            authenticated: std::collections::BTreeSet::from([id]),
        };
        view.controls.get_mut(&id).unwrap().credit_step = 20;
        let (bytes, authenticated) = view.credit(id, 24, life, &prefix).unwrap().unwrap();
        let reply: Response = serde_json::from_slice(&bytes).unwrap();
        assert!(authenticated);
        assert_eq!(reply.request_id, 24);
        assert_eq!(reply.control.unwrap().credit_step, 20);
        for field in 0..6 {
            let mut stale = prefix.clone();
            match field {
                0 => stale.tick = 6,
                1 => stale.control.as_mut().unwrap().epoch += 1,
                2 => stale.control.as_mut().unwrap().accepted_sequence += 1,
                3 => stale.control.as_mut().unwrap().world_step += 1,
                4 => stale.control.as_mut().unwrap().credit_step = 21,
                _ => stale.control.as_mut().unwrap().life.actor += 1,
            }
            assert!(view.credit(id, 24, life, &stale).is_none());
        }
        let mut foreign = life;
        foreign.actor += 1;
        assert!(view.credit(id, 24, foreign, &prefix).is_none());
        view.authenticated.clear();
        assert!(view.credit(id, 24, life, &prefix).is_none());
    }

    #[test]
    fn durable_travel_confirmation_preserves_body_and_cannot_cross_admission_prefix() {
        let life = verse_engine::core::LifeId {
            instance: 120,
            actor: 14,
            generation: 2,
        };
        let baseline = crate::movement::Baseline {
            profile: crate::movement::Profile::Frames,
            life,
            epoch: 3,
            applied_sequence: 7,
            physics_step: 8,
            world_step: 12,
            held: Default::default(),
            policy: Default::default(),
            character: physics::character::Character::new(glam::DVec3::ZERO),
            yaw: 0.,
        };
        let response = Response {
            version: VERSION,
            request_id: 23,
            instance: 120,
            tick: 4,
            control: Some(Control {
                life: life.into(),
                epoch: 3,
                accepted_sequence: 7,
                world_step: 12,
                credit_step: 12,
                applied_movement: None,
                dynamic: Vec::new(),
            }),
            body: Reply::Refused {
                code: "gameplay".into(),
                message: "Body text contains \"control\": and \"body\":".into(),
            },
        };
        let original = response.encode().unwrap();
        let mut ahead = baseline;
        ahead.applied_sequence = 8;
        let histories = BTreeMap::from([(life, vec![baseline, ahead])]);
        let (encoded, authenticated) =
            committed_movement(Ok((original.clone(), true)), &histories, &[]).unwrap();
        assert!(authenticated);
        let actual: Response = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(
            actual
                .control
                .as_ref()
                .unwrap()
                .applied_movement
                .unwrap()
                .applied_sequence,
            7
        );
        let mut expected = serde_json::to_value(response.clone()).unwrap();
        expected["control"]["applied_movement"] = serde_json::to_value(baseline).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&encoded).unwrap(),
            expected
        );
        for mut invalid in [baseline; 3].into_iter().enumerate() {
            match invalid.0 {
                0 => invalid.1.epoch += 1,
                1 => invalid.1.applied_sequence += 1,
                _ => invalid.1.world_step += 1,
            }
            let histories = BTreeMap::from([(life, vec![invalid.1])]);
            assert_eq!(
                committed_movement(Ok((original.clone(), true)), &histories, &[])
                    .unwrap()
                    .0,
                original
            );
        }
        let mut bounded = response;
        if let Reply::Refused { message, .. } = &mut bounded.body {
            message.clear();
        }
        let overhead = bounded.encode().unwrap().len();
        if let Reply::Refused { message, .. } = &mut bounded.body {
            *message = "a".repeat(MAX_RESPONSE_BYTES - overhead);
        }
        assert!(
            committed_movement(Ok((bounded.encode().unwrap(), true)), &histories, &[]).is_err()
        );
    }

    #[test]
    fn durable_clock_credit_preserves_the_admission_prefix_and_byte_bounds() {
        let mut response = Response {
            version: VERSION,
            request_id: 23,
            instance: 120,
            tick: 4,
            control: Some(Control {
                life: super::super::wire::Life { instance: 120, actor: 14, generation: 2 },
                epoch: 3,
                accepted_sequence: 7,
                world_step: 9,
                credit_step: 9,
                applied_movement: None,
                dynamic: Vec::new(),
            }),
            body: Reply::Refused {
                code: "gameplay".into(),
                message: "Body strings can contain \"world_step\":999 and \"body\": without changing header credit".into(),
            },
        };
        let original = response.encode().unwrap();
        let (renewed, admitted) =
            committed_world_credit(Ok((original.clone(), true)), 100).unwrap();
        assert!(admitted);
        let mut actual: serde_json::Value = serde_json::from_slice(&renewed).unwrap();
        assert_eq!(actual["control"]["credit_step"], 100);
        actual["control"]["credit_step"] = serde_json::json!(9);
        assert_eq!(
            actual,
            serde_json::from_slice::<serde_json::Value>(&original).unwrap()
        );
        assert!(committed_world_credit(Ok((original, true)), 8).is_err());
        response.control = None;
        let original = response.encode().unwrap();
        assert_eq!(
            committed_world_credit(Ok((original.clone(), false)), 100).unwrap(),
            (original, false)
        );
        response.control = Some(Control {
            life: super::super::wire::Life {
                instance: 120,
                actor: 14,
                generation: 2,
            },
            epoch: 3,
            accepted_sequence: 7,
            world_step: 9,
            credit_step: 9,
            applied_movement: None,
            dynamic: Vec::new(),
        });
        if let Reply::Refused { message, .. } = &mut response.body {
            message.clear();
        }
        let overhead = response.encode().unwrap().len();
        if let Reply::Refused { message, .. } = &mut response.body {
            *message = "a".repeat(MAX_RESPONSE_BYTES - overhead);
        }
        let bounded = response.encode().unwrap();
        assert_eq!(bounded.len(), MAX_RESPONSE_BYTES);
        assert!(committed_world_credit(Ok((bounded, true)), 100).is_err());
    }
    pub(in crate::service) fn key(n: u8) -> Keypair {
        Keypair::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_byte_array([n; 32]).unwrap(),
        )
    }
    fn public(k: &Keypair) -> [u8; 32] {
        k.x_only_public_key().0.serialize()
    }
    pub(in crate::service) fn gateway(keys: &[Keypair; 3]) -> Gateway {
        gateway_at(keys, None)
    }
    pub(in crate::service) fn gateway_at(keys: &[Keypair; 3], giver: Option<Vec3>) -> Gateway {
        let mut scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        if let Some(position) = giver {
            scene
                .actors
                .iter_mut()
                .find(|a| a.id == 2)
                .unwrap()
                .position = position;
        }
        let mut game = Game::combat_in(scene, false, 120).unwrap();
        game.time = game.scene.cut_at;
        game.tick(0., [0.; 2]).unwrap();
        game.encounter
            .as_mut()
            .unwrap()
            .postpone_casts_until(600.)
            .unwrap();
        let mut g = Gateway::new(Chamber::new(game).unwrap()).unwrap();
        g.enroll_primary(public(&keys[0])).unwrap();
        g.enroll_player(public(&keys[1]), Vec3::new(3., 0., -22.))
            .unwrap();
        g.enroll_spectator(public(&keys[2])).unwrap();
        g
    }
    pub(in crate::service) fn tls() -> (Arc<ServerConfig>, TlsConnector) {
        let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let der = certificate.cert.der().clone();
        let private = PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der());
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let server = ServerConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![der.clone()], private.into())
            .unwrap();
        let mut roots = RootCertStore::empty();
        roots.add(der).unwrap();
        let client = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
        (Arc::new(server), TlsConnector::from(Arc::new(client)))
    }
    pub(in crate::service) async fn start(
        keys: &[Keypair; 3],
    ) -> (
        std::net::SocketAddr,
        TlsConnector,
        oneshot::Sender<()>,
        tokio::task::JoinHandle<Exit>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tls, connector) = tls();
        let g = gateway(keys);
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(serve(listener, tls, g, async {
            let _ = stopped.await;
        }));
        (address, connector, stop, task)
    }
    #[tokio::test]
    async fn catch_up_ticks_each_leave_their_own_simulation_observation() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (tls, _) = tls();
        let keys = [key(121), key(122), key(123)];
        let gateway = gateway(&keys);
        let (stop, stopped) = oneshot::channel();
        let mut stop = Some(stop);
        let mut hooks = 0;
        let hook: Tick = Box::new(move |_, _| {
            hooks += 1;
            if hooks == 1 {
                // The next wake has more than one fixed step to account for.
                std::thread::sleep(Duration::from_millis(80));
            }
            if hooks >= 8 {
                if let Some(stop) = stop.take() {
                    let _ = stop.send(());
                }
            }
        });
        let exit = timeout(
            Duration::from_secs(3),
            serve_ticked(listener, tls, gateway, None, hook, async {
                let _ = stopped.await;
            }),
        )
        .await
        .unwrap();
        assert!(exit.failure.is_none(), "{:?}", exit.failure);
        assert!(exit.stats.ticks >= 8);
        assert_eq!(exit.stats.simulation.count, exit.stats.ticks);
        assert_eq!(exit.stats.simulation_phases.startup.count, exit.stats.ticks);
    }
    async fn open(address: std::net::SocketAddr, connector: &TlsConnector) -> (Stream, Hello) {
        let socket = TcpStream::connect(address).await.unwrap();
        socket.set_nodelay(true).unwrap();
        let mut stream = timeout(
            Duration::from_secs(3),
            connector.connect(ServerName::try_from("localhost").unwrap(), socket),
        )
        .await
        .unwrap()
        .unwrap();
        let bytes = timeout(
            Duration::from_secs(3),
            read_frame(&mut stream, MAX_RESPONSE_BYTES),
        )
        .await
        .unwrap()
        .unwrap();
        (stream, serde_json::from_slice(&bytes).unwrap())
    }
    async fn send(stream: &mut Stream, number: u64, body: Body) -> Response {
        let bytes = serde_json::to_vec(&Request {
            version: VERSION,
            request_id: number,
            body,
        })
        .unwrap();
        timeout(
            Duration::from_secs(3),
            write_frame(stream, &bytes, MAX_REQUEST_BYTES),
        )
        .await
        .unwrap()
        .unwrap();
        let bytes = timeout(
            Duration::from_secs(3),
            read_frame(stream, MAX_RESPONSE_BYTES),
        )
        .await
        .unwrap()
        .unwrap();
        let response: Response = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(response.request_id, number);
        assert_eq!(response.instance, 120);
        response
    }
    async fn join(
        address: std::net::SocketAddr,
        connector: &TlsConnector,
        key: &Keypair,
    ) -> (Stream, Response) {
        let (mut stream, hello) = open(address, connector).await;
        let signature = Secp256k1::new()
            .sign_schnorr_no_aux_rand(&hello.challenge.signing_digest(public(key)), key)
            .to_byte_array()
            .to_vec();
        let response = send(
            &mut stream,
            1,
            Body::Authenticate {
                public_key: public(key),
                signature,
            },
        )
        .await;
        assert!(matches!(response.body, Reply::Accepted));
        (stream, response)
    }
    fn state(r: Response) -> State {
        let Reply::Snapshot { state } = r.body else {
            panic!()
        };
        state
    }

    #[tokio::test]
    async fn slow_tls_handshake_expires_and_releases_its_pending_slot() {
        use crate::service::client::Client;
        let keys = [key(177), key(178), key(179)];
        let (address, connector, stop, task) = start(&keys).await;
        let mut player = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        let mut stalled = TcpStream::connect(address).await.unwrap();
        let mut byte = [0];
        assert_eq!(
            timeout(HANDSHAKE + Duration::from_secs(2), stalled.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        player.snapshot().await.unwrap();
        assert!(matches!(
            player
                .command(crate::Intent::Move {
                    axes: [0., 0.],
                    yaw: 0.
                })
                .await
                .unwrap()
                .body,
            Reply::Accepted
        ));
        player.close().await.unwrap();
        stop.send(()).unwrap();
        let exit = task.await.unwrap();
        assert!(exit.failure.is_none());
        assert_eq!(exit.stats.admission.handshake_timeouts, 1);
        assert_eq!(
            (exit.stats.admission.pending, exit.stats.admission.active),
            (0, 0)
        );
    }

    #[tokio::test]
    async fn movement_entry_storage_backpressure_preserves_exact_request_and_refusal_boundaries() {
        for (wait, admitted, code, count) in [
            (true, true, "storage_busy", 3),
            (false, true, "storage_busy", 1),
            (true, false, "storage_busy", 1),
            (true, true, "command", 1),
        ] {
            let keys = [key(177), key(178), key(179)];
            let (id, _) = gateway(&keys).open_json(0).unwrap();
            let request = Request {
                version: VERSION,
                request_id: 9,
                body: Body::BeginMovementFrames {
                    life: super::super::wire::Life {
                        instance: 120,
                        actor: 1,
                        generation: 0,
                    },
                    epoch: 4,
                },
            };
            let wait = movement_storage_wait(wait, &request.body);
            assert!(!movement_storage_wait(true, &Body::Snapshot {}));
            let request = serde_json::to_vec(&request).unwrap();
            let expected = request.clone();
            let (send, mut receive) = mpsc::channel(4);
            let peer = tokio::spawn(async move {
                for attempt in 0..count {
                    let Event::Request {
                        id: actual,
                        bytes,
                        reply,
                        ..
                    } = receive.recv().await.unwrap()
                    else {
                        panic!()
                    };
                    assert_eq!(actual, id);
                    assert_eq!(bytes, expected);
                    let response = Response {
                        version: VERSION,
                        request_id: 9,
                        instance: 120,
                        tick: attempt,
                        control: None,
                        body: if attempt == 2 {
                            Reply::Accepted
                        } else {
                            Reply::Refused {
                                code: code.into(),
                                message: "Not admitted".into(),
                            }
                        },
                    };
                    reply
                        .send(Ok((response.encode().unwrap(), admitted)))
                        .unwrap();
                }
                assert!(
                    timeout(Duration::from_millis(80), receive.recv())
                        .await
                        .is_err()
                );
            });
            let result = request_with_storage_backpressure(&send, id, request, wait)
                .await
                .unwrap();
            let response: Response = serde_json::from_slice(&result.0).unwrap();
            assert_eq!(matches!(response.body, Reply::Accepted), count == 3);
            peer.await.unwrap();
        }
        let keys = [key(177), key(178), key(179)];
        let (id, _) = gateway(&keys).open_json(0).unwrap();
        let (send, mut receive) = mpsc::channel(4);
        let peer = tokio::spawn(async move {
            let Event::Request { reply, .. } = receive.recv().await.unwrap() else {
                panic!()
            };
            reply.send(Err("Uncertain storage result".into())).unwrap();
            assert!(
                timeout(Duration::from_millis(80), receive.recv())
                    .await
                    .is_err()
            );
        });
        assert_eq!(
            request_with_storage_backpressure(&send, id, vec![1], true)
                .await
                .unwrap_err(),
            "Uncertain storage result"
        );
        peer.await.unwrap();
    }

    #[tokio::test]
    async fn storage_busy_does_not_extend_the_authentication_deadline() {
        let keys = [key(174), key(175), key(176)];
        let mut gateway = gateway(&keys);
        let (near, mut far) = tokio::io::duplex(16 * 1024);
        let limits = admission::Limits::new();
        let slot = limits.open([127, 0, 0, 1].into()).unwrap();
        let (send, mut receive) = mpsc::channel(4);
        let deadline = tokio::time::Instant::now() + Duration::from_millis(250);
        let session = tokio::spawn(session_until(near, None, send, slot, deadline));
        let Event::Open { reply, .. } = receive.recv().await.unwrap() else {
            panic!()
        };
        reply.send(gateway.open_json(0)).unwrap();
        read_frame(&mut far, MAX_RESPONSE_BYTES).await.unwrap();
        let request = Request {
            version: VERSION,
            request_id: 1,
            body: Body::Authenticate {
                public_key: public(&keys[0]),
                signature: vec![0; 64],
            },
        };
        write_frame(
            &mut far,
            &serde_json::to_vec(&request).unwrap(),
            MAX_REQUEST_BYTES,
        )
        .await
        .unwrap();
        let Event::Request {
            id, bytes, reply, ..
        } = receive.recv().await.unwrap()
        else {
            panic!()
        };
        tokio::time::sleep_until(deadline + Duration::from_millis(10)).await;
        reply
            .send(CommitView::capture(&gateway).busy(id, &bytes))
            .unwrap();
        read_frame(&mut far, MAX_RESPONSE_BYTES).await.unwrap();
        assert!(
            timeout(
                Duration::from_millis(100),
                read_frame(&mut far, MAX_RESPONSE_BYTES)
            )
            .await
            .unwrap()
            .is_err()
        );
        assert!(matches!(receive.recv().await.unwrap(), Event::Close(_)));
        let result = session.await.unwrap();
        assert_eq!(
            result.as_ref().unwrap_err(),
            "Chamber authentication timed out"
        );
        limits.finish(&result);
        assert_eq!(limits.stats().authentication_timeouts, 1);
        assert_eq!(limits.stats().pending, 0);
    }

    #[tokio::test]
    async fn overload_keeps_admitted_commands_live_and_reconnects_share_budgets() {
        let keys = [key(171), key(172), key(173)];
        let (address, connector, stop, task) = start(&keys).await;
        let (mut player, mut owned) = join(address, &connector, &keys[0]).await;
        let (mut flood, _) = join(address, &connector, &keys[2]).await;
        // Sockets send no TLS bytes. They cannot occupy admitted-player slots.
        let mut sockets = Vec::new();
        for _ in 0..48 {
            sockets.push(TcpStream::connect(address).await.unwrap());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut refused = 0;
        let mut admitted = 0;
        for sequence in 1..=40 {
            let response = send(&mut flood, sequence + 1, Body::Snapshot {}).await;
            if matches!(response.body, Reply::Refused { ref code, .. } if code == "rate_limited") {
                refused += 1;
            } else {
                assert!(matches!(response.body, Reply::Snapshot { .. }));
            }
            let control = owned.control.clone().unwrap();
            owned = send(
                &mut player,
                sequence + 1,
                Body::Command {
                    command: Input {
                        actor: control.life,
                        epoch: control.epoch,
                        sequence,
                        tick: owned.tick,
                        intent: Action::Move {
                            axes: [0.5, 0.],
                            yaw: 0.,
                        },
                    },
                },
            )
            .await;
            assert!(matches!(owned.body, Reply::Accepted), "{:?}", owned.body);
            assert_eq!(owned.control.as_ref().unwrap().accepted_sequence, sequence);
            admitted += 1;
        }
        assert!(refused > 0);
        // Free pending IP slots before the legitimate reconnect.
        drop(sockets);
        tokio::time::sleep(Duration::from_millis(50)).await;
        let (mut reconnected, _) = join(address, &connector, &keys[2]).await;
        let reply = send(&mut reconnected, 2, Body::Snapshot {}).await;
        // Real elapsed time can refill credit; the deterministic budget test
        // separately verifies that reconnection adds no credit.
        assert!(
            matches!(reply.body, Reply::Snapshot { .. })
                || matches!(reply.body, Reply::Refused {code, ..} if code == "rate_limited")
        );
        assert!(
            timeout(
                Duration::from_secs(1),
                read_frame(&mut flood, MAX_RESPONSE_BYTES)
            )
            .await
            .unwrap()
            .is_err()
        );
        drop(player);
        drop(flood);
        drop(reconnected);
        tokio::time::sleep(Duration::from_millis(20)).await;
        stop.send(()).unwrap();
        let exit = task.await.unwrap();
        assert!(exit.failure.is_none());
        assert!(exit.stats.admission.ip_refusals > 0);
        assert!(exit.stats.admission.principal_work_refusals > 0);
        assert!(exit.stats.admission.retired_connections > 0);
        assert_eq!(
            (exit.stats.admission.pending, exit.stats.admission.active),
            (0, 0)
        );
        eprintln!(
            "{}",
            serde_json::json!({"schema":"verse.admission.fixture.v1", "admitted_commands":admitted, "snapshot_refusals":refused, "stats":exit.stats.admission})
        );
    }

    #[tokio::test]
    async fn checkpoint_credit_preserves_sequence_snapshots_and_other_connections() {
        use crate::service::client::Client;
        let keys = [key(221), key(222), key(223)];
        let (address, tls, stop, host) = start(&keys).await;
        let mut client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            tls.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        let snapshot = client.request(Body::Snapshot {}).await.unwrap();
        let old = snapshot.control.clone().unwrap();
        let mut identities = gateway(&keys);
        let (first, _) = identities.open(0).unwrap();
        let (second, _) = identities.open(1).unwrap();

        let mut current = old.clone();
        current.world_step += 4;
        current.credit_step += 4;
        current.accepted_sequence += 10;
        let mut acknowledgment = snapshot.clone();
        acknowledgment.body = Reply::Accepted;
        let (bytes, _) = committed_control_credit(
            Ok((acknowledgment.encode().unwrap(), true)),
            &BTreeMap::from([(first, current.clone())]),
        )
        .unwrap();
        let promoted: Response = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            promoted.control.as_ref().unwrap().credit_step,
            current.credit_step
        );
        assert_eq!(
            promoted.control.as_ref().unwrap().accepted_sequence,
            old.accepted_sequence
        );
        assert_eq!(promoted.tick, acknowledgment.tick);
        current.epoch += 1;
        let original = acknowledgment.encode().unwrap();
        assert_eq!(
            committed_control_credit(
                Ok((original.clone(), true)),
                &BTreeMap::from([(first, current.clone())])
            )
            .unwrap()
            .0,
            original
        );
        current.epoch = old.epoch;
        let credited = committed_control_credit(
            Ok((snapshot.encode().unwrap(), true)),
            &BTreeMap::from([(first, current.clone())]),
        )
        .unwrap()
        .0;
        let decoded: Response = serde_json::from_slice(&credited).unwrap();
        assert_eq!(decoded.control.as_ref().unwrap().world_step, old.world_step);
        assert_eq!(
            decoded.control.as_ref().unwrap().credit_step,
            current.credit_step
        );
        let original_snapshot = snapshot.encode().unwrap();
        let before: serde_json::Value = serde_json::from_slice(&original_snapshot).unwrap();
        let after: serde_json::Value = serde_json::from_slice(&credited).unwrap();
        assert_eq!(before["body"], after["body"]);
        let mut second_credit = current.clone();
        second_credit.life.actor += 1;
        second_credit.credit_step += 8;
        let mut other = acknowledgment.clone();
        other.control.as_mut().unwrap().life = second_credit.life;
        let credits = BTreeMap::from([(first, current.clone()), (second, second_credit.clone())]);
        let controls: Vec<_> = [
            acknowledgment.clone(),
            other,
            snapshot.clone(),
            acknowledgment.clone(),
        ]
        .into_iter()
        .map(|response| {
            let bytes = committed_control_credit(Ok((response.encode().unwrap(), true)), &credits)
                .unwrap()
                .0;
            serde_json::from_slice::<Response>(&bytes)
                .unwrap()
                .control
                .unwrap()
        })
        .collect();
        assert_eq!(controls[0].world_step, old.world_step);
        assert_eq!(controls[1].world_step, old.world_step);
        assert_eq!(controls[2].world_step, old.world_step);
        assert_eq!(controls[3].world_step, old.world_step);
        assert!(
            controls
                .iter()
                .all(|c| c.accepted_sequence == old.accepted_sequence)
        );
        assert_eq!(
            controls.iter().map(|c| c.credit_step).collect::<Vec<_>>(),
            vec![
                current.credit_step,
                second_credit.credit_step,
                current.credit_step,
                current.credit_step
            ]
        );
        let Reply::Snapshot { state } = snapshot.body else {
            panic!("Expected owned snapshot");
        };
        state.validate_control(120, &Some(old)).unwrap();
        let _ = stop.send(());
        assert!(host.await.unwrap().failure.is_none());
    }

    #[tokio::test]
    async fn slow_writer_fences_replies_and_backpressures_the_bounded_request_queue() {
        use crate::{Intent, service::client::Client};
        use std::sync::{
            Condvar, Mutex,
            atomic::{AtomicBool, Ordering},
        };
        struct Release(Arc<(Mutex<bool>, Condvar)>);
        impl Drop for Release {
            fn drop(&mut self) {
                *self.0.0.lock().unwrap() = true;
                self.0.1.notify_all();
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let keys = [key(121), key(122), key(123)];
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let release = Release(gate.clone());
        let armed = Arc::new(AtomicBool::new(false));
        let trigger = armed.clone();
        let (started, mut blocked) = mpsc::unbounded_channel();
        store.inject(Arc::new(move |stage| {
            if stage == "before_encode" && trigger.swap(false, Ordering::AcqRel) {
                started.send(()).unwrap();
                let mut open = gate.0.lock().unwrap();
                while !*open {
                    open = gate.1.wait(open).unwrap();
                }
            }
        }));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tls, connector) = tls();
        let (stop, stopped) = oneshot::channel();
        let server = tokio::spawn(serve_durable(
            listener,
            tls,
            gateway(&keys).with_content([8; 32]).unwrap(),
            store,
            async {
                let _ = stopped.await;
            },
        ));
        let name = || ServerName::try_from("localhost").unwrap();
        let mut a = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([8; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let mut b = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([8; 32]),
            &keys[1],
        )
        .await
        .unwrap();
        b.snapshot().await.unwrap();
        let committed_tick = b.tick();
        let control = b.control().unwrap();
        let committed_control = (control.life, control.epoch, control.accepted_sequence);
        a.snapshot().await.unwrap();
        armed.store(true, Ordering::Release);
        timeout(Duration::from_secs(2), blocked.recv())
            .await
            .unwrap()
            .unwrap();
        let movement = Intent::Move {
            axes: [0., 0.],
            yaw: 0.,
        };
        let command = a.command(movement.clone());
        tokio::pin!(command);
        assert!(
            timeout(Duration::from_millis(120), &mut command)
                .await
                .is_err(),
            "Uncommitted command was acknowledged"
        );
        let mut b = b.pipeline().unwrap();
        let command_b = b.prepare_command(movement).unwrap();
        b.send(Body::Command {
            command: command_b.into(),
        })
        .unwrap();
        b.send_snapshot().unwrap();
        assert!(
            timeout(Duration::from_millis(120), b.receive())
                .await
                .is_err(),
            "A queued command bypassed storage backpressure or its durability fence"
        );
        drop(release);
        let admitted = timeout(Duration::from_secs(2), &mut command)
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(admitted.body, Reply::Accepted),
            "Unexpected admission after storage drain: {:?}",
            admitted.body
        );
        let (body, response) = timeout(Duration::from_secs(2), b.receive())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(body, Body::Command { .. }));
        assert!(matches!(response.body, Reply::Accepted));
        assert!(response.tick >= committed_tick);
        let control = response.control.unwrap();
        assert_eq!(
            (control.life, control.epoch),
            (committed_control.0, committed_control.1)
        );
        assert_eq!(control.accepted_sequence, committed_control.2 + 1);
        let (body, response) = timeout(Duration::from_secs(2), b.receive())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            body,
            Body::Snapshot { .. } | Body::Replicate { .. }
        ));
        assert!(matches!(response.body, Reply::Snapshot { .. }));
        stop.send(()).unwrap();
        let exit = timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
        assert!(exit.failure.is_none(), "{:?}", exit.failure);
        assert_eq!(exit.stats.writer_queue_peak, 2);
        assert!((1..=QUEUE).contains(&exit.stats.request_queue_peak));
        assert!((1..=REPLY_BYTES * 2).contains(&exit.stats.held_reply_bytes_peak));
        assert_eq!(exit.stats.storage_refusals, 0);
        assert!(exit.stats.storage_paused_ticks > 0 && exit.stats.storage_paused_seconds > 0.);
        assert!(exit.stats.commits.maximum_seconds >= 0.12);
        assert_eq!(exit.stats.simulation.count, exit.stats.ticks);
        assert!(exit.stats.capture.count >= 2);
        assert!(exit.stats.checkpoint_commits <= exit.stats.capture.count + 1);
        assert!(exit.stats.commits.percentile(0.99).unwrap().is_finite());
        // Shutdown drains the writer and releases its exclusive storage lock.
        assert!(Store::open(&root, [8; 32], 120).is_ok());
    }

    #[tokio::test]
    async fn durable_ticks_share_checkpoints_and_shutdown_keeps_the_latest_state() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("state");
        let keys = [key(121), key(122), key(123)];
        let store = Store::open(&root, [8; 32], 120).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (tls, _) = tls();
        let (stop, stopped) = oneshot::channel();
        let mut stop = Some(stop);
        let mut ticks = 0;
        let hook: Tick = Box::new(move |_, _| {
            ticks += 1;
            if ticks == 24 {
                let _ = stop.take().unwrap().send(());
            }
        });
        let exit = timeout(
            Duration::from_secs(10),
            serve_ticked(
                listener,
                tls,
                gateway(&keys).with_content([8; 32]).unwrap(),
                Some(store),
                hook,
                async {
                    let _ = stopped.await;
                },
            ),
        )
        .await
        .unwrap();
        assert!(exit.failure.is_none(), "{:?}", exit.failure);
        assert!(exit.stats.ticks >= 24);
        assert!(
            exit.stats.checkpoint_commits <= exit.stats.ticks.div_ceil(2) + 2,
            "Each tick still forces a separate durable checkpoint: {} commits for {} ticks",
            exit.stats.checkpoint_commits,
            exit.stats.ticks
        );
        let expected_tick = exit.gateway.game().authority_tick;
        let mut reopened = Store::open(&root, [8; 32], 120).unwrap();
        let restored = reopened.recover().unwrap();
        assert_eq!(restored.game().authority_tick, expected_tick);
    }

    #[tokio::test]
    async fn durable_two_player_input_cadence_records_checkpoint_cost() {
        use crate::{Intent, service::client::Client};
        let dir = tempfile::tempdir().unwrap();
        let keys = [key(117), key(118), key(119)];
        let store = Store::open(&dir.path().join("state"), [8; 32], 120).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tls, connector) = tls();
        let (stop, stopped) = oneshot::channel();
        let server = tokio::spawn(serve_durable(
            listener,
            tls,
            gateway(&keys).with_content([8; 32]).unwrap(),
            store,
            async {
                let _ = stopped.await;
            },
        ));
        let name = || ServerName::try_from("localhost").unwrap();
        let mut a = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([8; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let mut b = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([8; 32]),
            &keys[1],
        )
        .await
        .unwrap();
        let mut spectator = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([8; 32]),
            &keys[2],
        )
        .await
        .unwrap();
        let start = Instant::now();
        timeout(Duration::from_secs(12), async {
            let mut clock = tokio::time::interval(Duration::from_millis(33));
            clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
            for _ in 0..90 {
                clock.tick().await;
                let (state_a, state_b, state_s) =
                    tokio::join!(a.snapshot(), b.snapshot(), spectator.snapshot());
                state_a.unwrap();
                state_b.unwrap();
                assert!(state_s.unwrap().hud.is_none());
                let movement = Intent::Move {
                    axes: [0., 0.],
                    yaw: 0.,
                };
                let (reply_a, reply_b) =
                    tokio::join!(a.command(movement.clone()), b.command(movement));
                let reply_a = reply_a.unwrap();
                let reply_b = reply_b.unwrap();
                assert!(
                    matches!(reply_a.body, Reply::Accepted),
                    "Primary input: {:?}",
                    reply_a.body
                );
                assert!(
                    matches!(reply_b.body, Reply::Accepted),
                    "Secondary input: {:?}",
                    reply_b.body
                );
            }
        })
        .await
        .unwrap();
        let elapsed = start.elapsed().as_secs_f64();
        stop.send(()).unwrap();
        let exit = server.await.unwrap();
        assert!(exit.failure.is_none());
        assert!(exit.stats.checkpoint_commits >= 90);
        let timings = |timing: &Timing| {
            serde_json::json!({
            "observations":timing.count,"total_seconds":timing.total_seconds,
            "maximum_ms":timing.maximum_seconds*1000.,
            "p50_upper_ms":timing.percentile(0.5).map(|s|s*1000.),
            "p95_upper_ms":timing.percentile(0.95).map(|s|s*1000.),
            "p99_upper_ms":timing.percentile(0.99).map(|s|s*1000.)})
        };
        eprintln!(
            "{}",
            serde_json::json!({"schema":"verse.durable.fixture.v2","players":2,"spectators":1,
            "accepted_movement_commands":180,"wall_seconds":elapsed,"world_ticks":exit.stats.ticks,
            "checkpoint_commits":exit.stats.checkpoint_commits,"checkpoint_bytes":exit.stats.checkpoint_bytes,
            "checkpoint_seconds":exit.stats.checkpoint_seconds,"dropped_seconds":exit.stats.dropped_seconds,
            "simulation":timings(&exit.stats.simulation),"capture":timings(&exit.stats.capture),
            "commits":timings(&exit.stats.commits),"writer_queue_peak":exit.stats.writer_queue_peak,
            "storage_refusals":exit.stats.storage_refusals,"storage_paused_ticks":exit.stats.storage_paused_ticks,
            "storage_paused_seconds":exit.stats.storage_paused_seconds})
        );
    }
    #[tokio::test]
    async fn durable_tls_host_recovers_acknowledged_combat_after_abrupt_restart() {
        use crate::{Intent, play::Ability, service::client::Client};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let keys = [key(111), key(112), key(113)];
        let gateway = gateway(&keys).with_content([8; 32]).unwrap();
        let target = gateway.game().actor_life(1).unwrap();
        let store = Store::open(&root, [8; 32], 120).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let (_stop, stopped) = oneshot::channel::<()>();
        let server = tokio::spawn(serve_durable(
            listener,
            server_tls.clone(),
            gateway,
            store,
            async {
                let _ = stopped.await;
            },
        ));
        let name = || ServerName::try_from("localhost").unwrap();
        let mut a = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([8; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let mut b = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([8; 32]),
            &keys[1],
        )
        .await
        .unwrap();
        let old_a = a.control().unwrap().clone();
        let old_b = b.control().unwrap().clone();
        a.snapshot().await.unwrap();
        assert!(matches!(
            a.command(Intent::Cast {
                ability: Ability::Shield,
                target: None,
                aim: [0., 0., 1.]
            })
            .await
            .unwrap()
            .body,
            Reply::Accepted
        ));
        b.snapshot().await.unwrap();
        assert!(matches!(
            b.command(Intent::Cast {
                ability: Ability::Fireball,
                target: Some(target),
                aim: [0., 0., 1.]
            })
            .await
            .unwrap()
            .body,
            Reply::Accepted
        ));
        let before_a = a.snapshot().await.unwrap();
        let before_b = b.snapshot().await.unwrap();
        assert_eq!(before_a.hud.as_ref().unwrap().resources.mana, 19);
        assert!(before_b.hud.as_ref().unwrap().casting.is_some());
        server.abort();
        assert!(matches!(server.await, Err(error) if error.is_cancelled()));
        assert!(a.snapshot().await.is_err());
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let recovered = store.recover().unwrap();
        assert_eq!(
            recovered
                .game()
                .player_hud(old_a.life.into())
                .unwrap()
                .resources
                .mana,
            19
        );
        assert!(
            recovered
                .game()
                .player_hud(old_b.life.into())
                .unwrap()
                .casting
                .is_some()
        );
        let mut expected = Game::restore(&recovered.game().checkpoint().unwrap()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel();
        let server = tokio::spawn(serve_durable(
            listener,
            server_tls,
            recovered,
            store,
            async {
                let _ = stopped.await;
            },
        ));
        let mut a = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([8; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let mut b = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([8; 32]),
            &keys[1],
        )
        .await
        .unwrap();
        let mut spectator = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([8; 32]),
            &keys[2],
        )
        .await
        .unwrap();
        assert_eq!(a.control().unwrap().life, old_a.life);
        assert_eq!(b.control().unwrap().life, old_b.life);
        assert!(a.control().unwrap().epoch > old_a.epoch);
        assert!(b.control().unwrap().epoch > old_b.epoch);
        let after_a = a.snapshot().await.unwrap();
        let after_b = b.snapshot().await.unwrap();
        assert_eq!(
            after_a.hud.as_ref().unwrap().resources.hp,
            before_a.hud.as_ref().unwrap().resources.hp
        );
        // Reconnection advances world time; compare regeneration against the recovered simulation.
        while expected.time < after_a.presentation.time {
            expected
                .tick(expected.physics_clock.dt as f32, [0.; 2])
                .unwrap();
        }
        assert_eq!(expected.time, after_a.presentation.time);
        assert_eq!(
            after_a.hud.as_ref().unwrap().resources.mana,
            expected
                .player_hud(old_a.life.into())
                .unwrap()
                .resources
                .mana
        );
        assert!(after_b.hud.as_ref().unwrap().casting.is_some());
        assert!(spectator.snapshot().await.unwrap().hud.is_none());
        assert_eq!(
            after_a
                .presentation
                .actors
                .iter()
                .filter(|p| p.actor.model == "adventurer")
                .count(),
            2
        );
        let stale = Input {
            actor: old_a.life,
            epoch: old_a.epoch,
            sequence: 1,
            tick: a.tick(),
            intent: Action::Move {
                axes: [1., 0.],
                yaw: 0.,
            },
        };
        assert!(matches!(
            a.request(Body::Command { command: stale })
                .await
                .unwrap()
                .body,
            Reply::Refused { .. }
        ));
        stop.send(()).unwrap();
        let exit = server.await.unwrap();
        assert!(exit.failure.is_none());
        assert!(exit.stats.checkpoint_commits > 0 && exit.stats.checkpoint_bytes > 0);
        assert!(exit.stats.checkpoint_seconds.is_finite());
    }
    #[tokio::test]
    async fn durable_host_stops_without_acknowledgment_on_storage_failure() {
        use crate::{Intent, play::Ability, service::client::Client};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let keys = [key(114), key(115), key(116)];
        let store = Store::open(&root, [8; 32], 120).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tls, connector) = tls();
        let (_stop, stopped) = oneshot::channel::<()>();
        let monitor = super::operator_tests::monitor();
        let server = tokio::spawn(serve_monitored(
            listener,
            tls,
            gateway(&keys).with_content([8; 32]).unwrap(),
            Some(store),
            None,
            monitor.clone(),
            async {
                let _ = stopped.await;
            },
        ));
        let mut client = Client::connect_with_content(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            Some([8; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let committed = std::fs::read(root.join("chamber.json")).unwrap();
        std::fs::create_dir(root.join("next.json")).unwrap();
        assert!(
            client
                .command(Intent::Cast {
                    ability: Ability::Shield,
                    target: None,
                    aim: [0., 0., 1.]
                })
                .await
                .is_err()
        );
        let exit = timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            exit.failure.as_deref(),
            Some("Chamber storage entry must be a regular file")
        );
        assert_eq!(std::fs::read(root.join("chamber.json")).unwrap(), committed);
        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.phase, super::super::operator::Phase::Failed);
        assert!(!snapshot.live && !snapshot.ready);
        assert_eq!(
            snapshot.reasons,
            vec![super::super::operator::Reason::StorageFailure]
        );
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join("writer.lock"))
            .unwrap();
        lock.try_lock().unwrap();
    }
    #[tokio::test]
    async fn tls_players_and_spectator_share_one_world_and_shutdown_parks_controls() {
        let keys = [key(21), key(22), key(23)];
        let (address, connector, stop, task) = start(&keys).await;
        let (mut a, auth_a) = join(address, &connector, &keys[0]).await;
        let (mut b, auth_b) = join(address, &connector, &keys[1]).await;
        let (mut spectator, auth_s) = join(address, &connector, &keys[2]).await;
        assert!(auth_s.control.is_none());
        let before_a = state(send(&mut a, 2, Body::Snapshot {}).await);
        let before_b = state(send(&mut b, 2, Body::Snapshot {}).await);
        let ca = auth_a.control.unwrap();
        let cb = auth_b.control.unwrap();
        let command = Input {
            actor: ca.life,
            epoch: ca.epoch,
            sequence: ca.accepted_sequence + 1,
            tick: auth_a.tick,
            intent: Action::Move {
                axes: [1., 0.],
                yaw: 0.,
            },
        };
        assert!(matches!(
            send(
                &mut spectator,
                3,
                Body::Command {
                    command: command.clone()
                }
            )
            .await
            .body,
            Reply::Refused { .. }
        ));
        assert!(matches!(
            send(
                &mut a,
                3,
                Body::Command {
                    command: command.clone()
                }
            )
            .await
            .body,
            Reply::Accepted
        ));
        let command_b = Input {
            actor: cb.life,
            epoch: cb.epoch,
            sequence: cb.accepted_sequence + 1,
            tick: auth_b.tick,
            intent: Action::Move {
                axes: [-1., 0.],
                yaw: 0.,
            },
        };
        assert!(matches!(
            send(
                &mut b,
                3,
                Body::Command {
                    command: command_b.into()
                }
            )
            .await
            .body,
            Reply::Accepted
        ));
        tokio::time::sleep(Duration::from_millis(100)).await;
        let after_a = state(send(&mut a, 4, Body::Snapshot {}).await);
        let after_b = state(send(&mut b, 4, Body::Snapshot {}).await);
        let shared = state(send(&mut spectator, 4, Body::Snapshot {}).await);
        assert_eq!(
            serde_json::to_vec(&after_a.actors).unwrap(),
            serde_json::to_vec(&shared.actors).unwrap()
        );
        let own_b = after_b
            .actors
            .iter()
            .find(|actor| actor.life == cb.life)
            .unwrap()
            .source;
        assert_ne!(
            before_a
                .snapshot
                .actors
                .iter()
                .find(|a| a.id == 0)
                .unwrap()
                .pos,
            after_a
                .snapshot
                .actors
                .iter()
                .find(|a| a.id == 0)
                .unwrap()
                .pos
        );
        assert_ne!(
            before_b
                .snapshot
                .actors
                .iter()
                .find(|a| a.id == own_b)
                .unwrap()
                .pos,
            after_b
                .snapshot
                .actors
                .iter()
                .find(|a| a.id == own_b)
                .unwrap()
                .pos
        );
        assert_eq!(
            after_a
                .snapshot
                .actors
                .iter()
                .map(|a| (a.id, a.hp))
                .collect::<Vec<_>>(),
            shared
                .snapshot
                .actors
                .iter()
                .map(|a| (a.id, a.hp))
                .collect::<Vec<_>>()
        );
        let (mut replacement, current) = join(address, &connector, &keys[0]).await;
        assert!(current.control.unwrap().epoch > ca.epoch);
        // Supersession now releases transport capacity without another request.
        assert!(
            timeout(
                Duration::from_secs(1),
                read_frame(&mut a, MAX_RESPONSE_BYTES)
            )
            .await
            .unwrap()
            .is_err()
        );
        assert!(matches!(
            send(&mut replacement, 2, Body::Command { command })
                .await
                .body,
            Reply::Refused { .. }
        ));
        drop(a);
        b.shutdown().await.unwrap();
        drop(b);
        tokio::time::sleep(Duration::from_millis(30)).await;
        let (reconnected_b, reconnected) = join(address, &connector, &keys[1]).await;
        assert!(reconnected.control.unwrap().epoch >= cb.epoch + 2);
        drop(reconnected_b);
        drop(spectator);
        drop(replacement);
        stop.send(()).unwrap();
        let exit = timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap();
        assert!(exit.failure.is_none());
        assert_eq!(exit.stats.requests, 14);
        assert!(exit.stats.ticks > 0);
        assert_eq!(
            exit.stats.accepted_connections,
            exit.stats.completed_connections
        );
        assert!(
            exit.gateway
                .game()
                .controlled_effects()
                .all(|(life, _, _)| exit
                    .gateway
                    .game()
                    .player_admission(life.actor)
                    .unwrap()
                    .controller()
                    == crate::Controller(0))
        );
    }

    #[tokio::test]
    async fn tls_and_frame_refusals_leave_the_world_available() {
        let keys = [key(24), key(25), key(26)];
        let (address, connector, stop, task) = start(&keys).await;
        let mut plaintext = TcpStream::connect(address).await.unwrap();
        plaintext
            .write_all(b"plaintext is not a TLS ClientHello")
            .await
            .unwrap();
        let _ = timeout(Duration::from_secs(3), plaintext.read_u8())
            .await
            .unwrap();
        drop(plaintext);
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let untrusted = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(RootCertStore::empty())
            .with_no_client_auth();
        let socket = TcpStream::connect(address).await.unwrap();
        assert!(
            TlsConnector::from(Arc::new(untrusted))
                .connect(ServerName::try_from("localhost").unwrap(), socket)
                .await
                .is_err()
        );
        let (mut oversized, _) = open(address, &connector).await;
        oversized
            .write_u32((MAX_REQUEST_BYTES + 1) as u32)
            .await
            .unwrap();
        oversized.flush().await.unwrap();
        assert!(
            timeout(
                Duration::from_secs(3),
                read_frame(&mut oversized, MAX_RESPONSE_BYTES)
            )
            .await
            .unwrap()
            .is_err()
        );
        drop(oversized);
        let (mut bad, _) = open(address, &connector).await;
        let refusal = send(
            &mut bad,
            1,
            Body::Authenticate {
                public_key: public(&keys[0]),
                signature: vec![0; 64],
            },
        )
        .await;
        assert!(matches!(refusal.body, Reply::Refused { .. }));
        assert!(
            timeout(
                Duration::from_secs(3),
                read_frame(&mut bad, MAX_RESPONSE_BYTES)
            )
            .await
            .unwrap()
            .is_err()
        );
        drop(bad);
        let (mut valid, _) = join(address, &connector, &keys[0]).await;
        assert!(matches!(
            send(&mut valid, 2, Body::Snapshot {}).await.body,
            Reply::Snapshot { .. }
        ));
        drop(valid);
        stop.send(()).unwrap();
        let exit = timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap();
        assert!(exit.failure.is_none());
        assert_eq!(exit.stats.accepted_connections, 5);
        assert_eq!(
            exit.stats.accepted_connections,
            exit.stats.completed_connections
        );
    }

    #[tokio::test]
    async fn frame_budget_rejects_header_before_waiting_for_payload() {
        let (mut writer, mut reader) = tokio::io::duplex(16);
        writer
            .write_u32((MAX_REQUEST_BYTES + 1) as u32)
            .await
            .unwrap();
        assert!(read_frame(&mut reader, MAX_REQUEST_BYTES).await.is_err());
        assert!(
            write_frame(&mut writer, &[], MAX_REQUEST_BYTES)
                .await
                .is_err()
        );
    }
}
