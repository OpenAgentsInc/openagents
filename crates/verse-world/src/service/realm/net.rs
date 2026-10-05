//! TLS adapters enqueue bounded work for one realm coordinator thread.
use super::super::net::{self as transport, Event};
use super::*;
use rustls::ServerConfig;
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot, watch},
    task::JoinSet,
};
use tokio_rustls::TlsAcceptor;
const QUEUE: usize = 128;

enum Operation {
    Account {
        id: u64,
        reply: oneshot::Sender<Result<Account, String>>,
    },
    Recover {
        account: u64,
        epoch: u64,
        key: [u8; 32],
        reply: oneshot::Sender<Result<Account, String>>,
    },
    CreateCharacter {
        instance: u64,
        principal: [u8; 32],
        spawn: [f32; 3],
        reply: oneshot::Sender<Result<(u64, LifeId), String>>,
    },
    Studio {
        instance: u64,
        actors: Vec<crate::play::social::SeatActor>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Route {
        character: u64,
        reply: oneshot::Sender<Result<Route, String>>,
    },
    Drain {
        instance: u64,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Admit {
        instance: u64,
        principal: [u8; 32],
        spawn: [f32; 3],
        reply: oneshot::Sender<Result<(u64, LifeId), String>>,
    },
    Transfer {
        source: u64,
        destination: u64,
        character: u64,
        operation: [u8; 16],
        spawn: [f32; 3],
        reply: oneshot::Sender<Result<Transfer, String>>,
    },
}
/// Keep this local operator handle separate from untrusted client payloads.
#[derive(Clone)]
pub struct Control(mpsc::Sender<Operation>);
pub struct Commands(mpsc::Receiver<Operation>);
pub fn channel() -> (Control, Commands) {
    let (send, receive) = mpsc::channel(QUEUE);
    (Control(send), Commands(receive))
}
impl Control {
    pub async fn account(&self, id: u64) -> Result<Account, String> {
        let (reply, receive) = oneshot::channel();
        self.send(Operation::Account { id, reply }).await?;
        receive.await.map_err(|_| "Realm operator stopped")?
    }

    pub async fn recover_account(
        &self,
        account: u64,
        expected_epoch: u64,
        new_key: [u8; 32],
    ) -> Result<Account, String> {
        let (reply, receive) = oneshot::channel();
        self.send(Operation::Recover {
            account,
            epoch: expected_epoch,
            key: new_key,
            reply,
        })
        .await?;
        receive.await.map_err(|_| "Realm operator stopped")?
    }
    pub async fn create_character(
        &self,
        instance: u64,
        principal: [u8; 32],
        spawn: [f32; 3],
    ) -> Result<(u64, LifeId), String> {
        let (reply, receive) = oneshot::channel();
        self.send(Operation::CreateCharacter {
            instance,
            principal,
            spawn,
            reply,
        })
        .await?;
        receive.await.map_err(|_| "Realm operator stopped")?
    }
    async fn send(&self, operation: Operation) -> Result<(), String> {
        self.0
            .send(operation)
            .await
            .map_err(|_| "Realm operator stopped".into())
    }
    pub async fn publish_social_studio(
        &self,
        instance: u64,
        actors: Vec<crate::play::social::SeatActor>,
    ) -> Result<(), String> {
        let (reply, receive) = oneshot::channel();
        self.send(Operation::Studio {
            instance,
            actors,
            reply,
        })
        .await?;
        receive.await.map_err(|_| "Realm operator stopped")?
    }
    pub async fn route(&self, character: u64) -> Result<Route, String> {
        let (reply, receive) = oneshot::channel();
        self.send(Operation::Route { character, reply }).await?;
        receive.await.map_err(|_| "Realm operator stopped")?
    }
    pub async fn drain(&self, instance: u64) -> Result<(), String> {
        let (reply, receive) = oneshot::channel();
        self.send(Operation::Drain { instance, reply }).await?;
        receive.await.map_err(|_| "Realm operator stopped")?
    }
    pub async fn admit(
        &self,
        instance: u64,
        principal: [u8; 32],
        spawn: [f32; 3],
    ) -> Result<(u64, LifeId), String> {
        let (reply, receive) = oneshot::channel();
        self.send(Operation::Admit {
            instance,
            principal,
            spawn,
            reply,
        })
        .await?;
        receive.await.map_err(|_| "Realm operator stopped")?
    }
    pub async fn transfer(
        &self,
        source: u64,
        destination: u64,
        character: u64,
        operation: [u8; 16],
        spawn: [f32; 3],
    ) -> Result<Transfer, String> {
        let (reply, receive) = oneshot::channel();
        self.send(Operation::Transfer {
            source,
            destination,
            character,
            operation,
            spawn,
            reply,
        })
        .await?;
        receive.await.map_err(|_| "Realm operator stopped")?
    }
}
enum Work {
    Client(u64, Event),
    Operator(Operation),
    Tick,
    Stop,
}
#[derive(Default, Debug, Serialize)]
pub struct Stats {
    pub admission: transport::AdmissionStats,
    pub ticks: u64,
    pub requests: u64,
    pub queue_peak: usize,
    pub skipped_seconds: f64,
    pub work_peak_seconds: f64,
}
pub struct Exit {
    pub realm: Realm,
    pub stats: Stats,
    pub failure: Option<String>,
}
fn now() -> Result<u64, String> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "Realm host clock precedes Unix epoch")?;
    u64::try_from(duration.as_millis()).map_err(|_| "Realm host clock exhausted".into())
}
fn coordinator(
    mut realm: Realm,
    leases: BTreeMap<u64, Lease>,
    mut receive: mpsc::Receiver<Work>,
    pending_tick: Arc<AtomicBool>,
) -> Exit {
    let mut stats = Stats::default();
    let mut failure = None;
    let mut last_tick = Instant::now();
    let mut last_renew = Instant::now();
    while let Some(work) = receive.blocking_recv() {
        stats.queue_peak = stats.queue_peak.max(receive.len() + 1);
        let started = Instant::now();
        let clock = match now() {
            Ok(now) => now,
            Err(e) => {
                failure = Some(e);
                break;
            }
        };
        // Explicit renewal still passes the deadline check after any storage stall.
        if last_renew.elapsed() >= Duration::from_secs(10) {
            for lease in leases.values() {
                if let Err(e) = realm.renew(lease, clock) {
                    failure = Some(e);
                    break;
                }
            }
            if failure.is_some() {
                break;
            }
            last_renew = Instant::now();
        }
        match work {
            Work::Stop => break,
            Work::Tick => {
                pending_tick.store(false, Ordering::Release);
                let elapsed = last_tick.elapsed().as_secs_f64();
                last_tick = Instant::now();
                let dt = elapsed.min(0.1);
                stats.skipped_seconds += elapsed - dt;
                for lease in leases.values() {
                    if let Err(e) = realm.tick(lease, clock, dt as f32) {
                        failure = Some(e);
                        break;
                    }
                    stats.ticks += 1;
                }
            }
            Work::Client(instance, event) => {
                let Some(lease) = leases.get(&instance) else {
                    failure = Some("Realm listener has no lease".into());
                    break;
                };
                match event {
                    Event::Open { spectate, reply } => {
                        let result = if spectate.is_some() {
                            Err("Realm TLS listeners require explicit enrollment".into())
                        } else {
                            realm.open_connection(lease, clock)
                        };
                        let _ = reply.send(result);
                    }
                    Event::Request {
                        id,
                        bytes,
                        reply,
                        progress,
                    } => {
                        stats.requests += 1;
                        let result = realm.dispatch(lease, id, clock, &bytes).map(|bytes| {
                            let admitted = realm.games[&instance].authenticated(id);
                            (bytes, admitted)
                        });
                        crate::service::net::dispatch_progress(progress, &result);
                        let _ = reply.send(result);
                    }
                    Event::Close(id) => {
                        let _ = realm.close_connection(lease, id, clock);
                    }
                }
            }
            Work::Operator(operation) => match operation {
                Operation::Account { id, reply } => {
                    let _ = reply.send(realm.account(id));
                }
                Operation::Recover {
                    account,
                    epoch,
                    key,
                    reply,
                } => {
                    let result = realm.recover_account(
                        &leases.values().cloned().collect::<Vec<_>>(),
                        account,
                        epoch,
                        key,
                        clock,
                    );
                    let _ = reply.send(result);
                }
                Operation::CreateCharacter {
                    instance,
                    principal,
                    spawn,
                    reply,
                } => {
                    let result = leases
                        .get(&instance)
                        .ok_or_else(|| "Realm listener has no lease".to_string())
                        .and_then(|lease| realm.create_character(lease, principal, spawn, clock));
                    let _ = reply.send(result);
                }
                Operation::Studio {
                    instance,
                    actors,
                    reply,
                } => {
                    let _ = reply.send(
                        leases
                            .get(&instance)
                            .ok_or_else(|| "Realm instance has no listener".into())
                            .and_then(|lease| realm.publish_social_studio(lease, actors, clock)),
                    );
                }
                Operation::Route { character, reply } => {
                    let _ = reply.send(realm.route(character, clock));
                }
                Operation::Drain { instance, reply } => {
                    let _ = reply.send(
                        leases
                            .get(&instance)
                            .ok_or_else(|| "Realm instance has no listener".into())
                            .and_then(|lease| realm.drain(lease, clock)),
                    );
                }
                Operation::Admit {
                    instance,
                    principal,
                    spawn,
                    reply,
                } => {
                    let _ = reply.send(
                        leases
                            .get(&instance)
                            .ok_or_else(|| "Realm instance has no listener".into())
                            .and_then(|lease| realm.admit(lease, principal, spawn, clock)),
                    );
                }
                Operation::Transfer {
                    source,
                    destination,
                    character,
                    operation,
                    spawn,
                    reply,
                } => {
                    let result = match (leases.get(&source), leases.get(&destination)) {
                        (Some(a), Some(b)) => {
                            realm.transfer(a, b, character, operation, spawn, clock)
                        }
                        _ => Err("Realm transfer instances have no listeners".into()),
                    };
                    let _ = reply.send(result);
                }
            },
        }
        stats.work_peak_seconds = stats.work_peak_seconds.max(started.elapsed().as_secs_f64());
        if realm.poisoned || failure.is_some() {
            failure.get_or_insert("Realm commit requires recovery".into());
            break;
        }
    }
    if failure.is_none() {
        let clock = now();
        for lease in leases.values() {
            let result = clock
                .as_ref()
                .map_err(Clone::clone)
                .and_then(|now| realm.release(lease, *now));
            if let Err(e) = result {
                failure = Some(e);
                break;
            }
        }
    }
    if failure.is_some() {
        realm.poisoned = true;
    }
    Exit {
        realm,
        stats,
        failure,
    }
}
/// Uses prebound listeners and leases; storage work never runs on the TLS executor.
pub async fn serve<F: Future<Output = ()>>(
    mut realm: Realm,
    listeners: Vec<(Lease, TcpListener)>,
    tls: Arc<ServerConfig>,
    mut commands: Commands,
    shutdown: F,
) -> Result<Exit, String> {
    let invalid = listeners.is_empty()
        || listeners.len() > INSTANCES
        || listeners.iter().any(|(lease, listener)| {
            realm
                .manifest
                .instances
                .get(&lease.instance)
                .is_none_or(|slot| {
                    listener
                        .local_addr()
                        .map_or(true, |a| a.port() != slot.endpoint.port())
                })
        });
    let leases: BTreeMap<_, _> = listeners
        .iter()
        .map(|(lease, _)| (lease.instance, lease.clone()))
        .collect();
    if invalid || leases.len() != listeners.len() {
        return Ok(Exit {
            realm,
            stats: Stats::default(),
            failure: Some("Realm listener configuration is incompatible".into()),
        });
    }
    let clock = now()?;
    for lease in leases.values() {
        if let Err(error) = realm.check(lease, clock) {
            return Ok(Exit {
                realm,
                stats: Stats::default(),
                failure: Some(error),
            });
        }
    }
    let (send, receive) = mpsc::channel(QUEUE);
    let pending_tick = Arc::new(AtomicBool::new(false));
    let pending_worker = pending_tick.clone();
    let worker =
        tokio::task::spawn_blocking(move || coordinator(realm, leases, receive, pending_worker));
    let limits = transport::admission::Limits::new();
    let mut adapters = JoinSet::new();
    let (stop_adapters, adapter_stop) = watch::channel(false);
    for (lease, listener) in listeners {
        let send = send.clone();
        let limits = limits.clone();
        let acceptor = TlsAcceptor::from(tls.clone());
        let mut stopped = adapter_stop.clone();
        adapters.spawn(async move {
            let (clients, mut events) = mpsc::channel(1);
            let mut connections = JoinSet::new();
            let outcome = async {
                loop {
                    tokio::select! {
                    _ = stopped.changed() => break,
                    accepted = listener.accept() => {
                        let (socket, address) = accepted.map_err(|_| "Realm listener failed")?;
                        if let Ok(slot) = limits.open(address.ip()) {
                            let acceptor = acceptor.clone(); let clients = clients.clone();
                            let metrics = limits.clone();
                            connections.spawn(async move { let result = transport::connection(socket, acceptor, clients, slot).await; metrics.finish(&result); });
                        }
                    }
                    event = events.recv() => {
                        if let Some(event) = event { send.send(Work::Client(lease.instance, event)).await.map_err(|_| "Realm coordinator stopped")?; }
                    }
                    result = connections.join_next(), if !connections.is_empty() => { if result.is_some_and(|r| r.is_err()) { limits.cancelled(); } }
                }
            }
                Ok::<(), String>(())
            }.await;
            connections.abort_all();
            while let Some(result) = connections.join_next().await { if result.is_err() { limits.cancelled(); } }
            outcome
        });
    }
    let mut ticker = tokio::time::interval(Duration::from_secs_f64(1. / 30.));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    tokio::pin!(shutdown);
    let mut adapter_failure = None;
    let mut commands_open = true;
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            _ = ticker.tick() => {
                if !pending_tick.swap(true, Ordering::AcqRel) && send.try_send(Work::Tick).is_err() { pending_tick.store(false, Ordering::Release); }
                if worker.is_finished() { break; }
            }
            command = commands.0.recv(), if commands_open => {
                if let Some(command) = command { if send.send(Work::Operator(command)).await.is_err() { break; } } else { commands_open = false; }
            }
            result = adapters.join_next(), if !adapters.is_empty() => { adapter_failure = Some(format!("Realm listener stopped: {result:?}")); break; }
        }
    }
    let _ = stop_adapters.send(true);
    while adapters.join_next().await.is_some() {}
    let _ = send.send(Work::Stop).await;
    drop(send);
    let mut exit = worker
        .await
        .map_err(|_| "Realm coordinator failed; reopen its durable state to recover")?;
    exit.stats.admission = limits.stats();
    if adapter_failure.is_some() {
        exit.failure = adapter_failure;
    }
    Ok(exit)
}
