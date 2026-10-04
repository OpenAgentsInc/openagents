//! TLS-only framed chamber IO with one host-owned world loop.
use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use rustls::ServerConfig;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Semaphore, mpsc, oneshot},
    task::JoinSet,
    time::{MissedTickBehavior, timeout},
};
use tokio_rustls::TlsAcceptor;

use super::{
    auth::{ConnectionId, Gateway},
    persistence::Store,
    wire::{Body, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request},
};

const CONNECTIONS: usize = 128;
const QUEUE: usize = 128;
const HANDSHAKE: Duration = Duration::from_secs(5);
const WRITE: Duration = Duration::from_secs(10);
const AUTH: Duration = Duration::from_secs(30);
const IDLE: Duration = Duration::from_secs(60);
const REQUESTS_PER_SECOND: u32 = 120;

/// Transport metrics; skipped elapsed time is not silently simulated later.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub accepted_connections: u64,
    pub capacity_refusals: u64,
    pub completed_connections: u64,
    pub requests: u64,
    pub ticks: u64,
    pub dropped_seconds: f64,
    pub checkpoint_commits: u64,
    pub checkpoint_bytes: u64,
    pub checkpoint_seconds: f64,
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
    response: PendingResponse,
}
enum PendingResponse {
    Outcome(DispatchReply),
    Read { id: ConnectionId, bytes: Vec<u8> },
}
enum Event {
    Open(oneshot::Sender<OpenReply>),
    Request {
        id: ConnectionId,
        bytes: Vec<u8>,
        reply: oneshot::Sender<DispatchReply>,
    },
    Close(ConnectionId),
}

/// Reads a big-endian u32 byte length before allocating its bounded JSON payload.
pub async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> Result<Vec<u8>, String> {
    let size = reader
        .read_u32()
        .await
        .map_err(|_| "Chamber frame header unavailable")? as usize;
    if size == 0 || size > max {
        return Err("Chamber frame exceeds byte budget".into());
    }
    let mut bytes = vec![0; size];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|_| "Chamber frame payload incomplete")?;
    Ok(bytes)
}
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    bytes: &[u8],
    max: usize,
) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > max {
        return Err("Chamber frame exceeds byte budget".into());
    }
    let length = u32::try_from(bytes.len()).map_err(|_| "Chamber frame length overflow")?;
    let mut frame = Vec::with_capacity(bytes.len() + 4);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(bytes);
    writer
        .write_all(&frame)
        .await
        .map_err(|_| "Cannot write chamber frame payload")?;
    writer
        .flush()
        .await
        .map_err(|_| "Cannot flush chamber frame")?;
    Ok(())
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
    serve_with_store(listener, tls, gateway, None, shutdown).await
}
/// Commits world mutations before replies and stops on any durability failure.
pub async fn serve_durable<F: Future<Output = ()>>(
    listener: TcpListener,
    tls: Arc<ServerConfig>,
    gateway: Gateway,
    store: Store,
    shutdown: F,
) -> Exit {
    serve_with_store(listener, tls, gateway, Some(store), shutdown).await
}
fn persist(gateway: &Gateway, store: &mut Option<Store>, stats: &mut Stats) -> Result<(), String> {
    if let Some(store) = store {
        let start = Instant::now();
        let committed = store.commit(gateway);
        stats.checkpoint_seconds += start.elapsed().as_secs_f64();
        let committed = committed?;
        if committed.written {
            stats.checkpoint_commits += 1;
            stats.checkpoint_bytes += committed.bytes as u64;
        }
    }
    Ok(())
}
async fn serve_with_store<F: Future<Output = ()>>(
    listener: TcpListener,
    tls: Arc<ServerConfig>,
    mut gateway: Gateway,
    mut store: Option<Store>,
    shutdown: F,
) -> Exit {
    let acceptor = TlsAcceptor::from(tls);
    let capacity = Arc::new(Semaphore::new(CONNECTIONS));
    let (send, mut receive) = mpsc::channel(QUEUE);
    let mut workers = JoinSet::new();
    let start = Instant::now();
    let mut last_tick = start;
    let period = Duration::from_secs_f64(1. / 30.);
    let mut ticker = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut stats = Stats::default();
    let mut failure = None;
    let mut pending: Vec<PendingReply> = Vec::with_capacity(QUEUE);
    let mut dirty = false;
    if let Err(error) = persist(&gateway, &mut store, &mut stats) {
        return Exit {
            gateway,
            stats,
            failure: Some(error),
        };
    }
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accepted = listener.accept() => {
                match accepted {
                    Ok((socket, _)) => {
                        let Ok(permit) = capacity.clone().try_acquire_owned() else {
                            stats.capacity_refusals += 1;
                            continue;
                        };
                        stats.accepted_connections += 1;
                        let acceptor = acceptor.clone(); let send = send.clone();
                        workers.spawn(async move {
                            let _permit = permit;
                            let _ = connection(socket, acceptor, send).await;
                        });
                    }
                    Err(_) => { failure = Some("Chamber listener failed".into()); break; }
                }
            }
            _ = ticker.tick() => {
                let now = Instant::now(); let elapsed = now.duration_since(last_tick).as_secs_f64(); last_tick = now;
                let dt = elapsed.min(0.1);
                stats.dropped_seconds += elapsed - dt;
                if let Err(error) = gateway.tick(dt as f32) { failure = Some(error); break; }
                if let Err(error) = persist(&gateway, &mut store, &mut stats) { failure = Some(error); break; }
                let now = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                for response in pending.drain(..) {
                    let result = match response.response {
                        PendingResponse::Outcome(result) => result,
                        PendingResponse::Read { id, bytes } => gateway.dispatch_json(id, now, &bytes)
                            .map(|bytes| (bytes, gateway.authenticated(id))),
                    };
                    let _ = response.reply.send(result);
                }
                dirty = false;
                stats.ticks += 1;
            }
            event = receive.recv() => {
                let now = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                match event {
                    Some(Event::Open(reply)) => {
                        let result = gateway.open_json(now);
                        if let Err(Ok((id, _))) = reply.send(result) { let _ = gateway.close(id); }
                    }
                    Some(Event::Request { id, bytes, reply }) => {
                        stats.requests += 1;
                        if store.is_some() && pending.len() >= QUEUE {
                            let error = "Chamber durable reply budget exceeded".to_string();
                            let _ = reply.send(Err(error.clone()));
                            failure = Some(error);
                            break;
                        }
                        let mutating = store.is_some() && Request::decode(&bytes).is_ok_and(|request| matches!(request.body,
                            Body::Authenticate { .. } | Body::Command { .. } | Body::Respawn { .. } | Body::ClaimQuest {..} | Body::UseItem {..} | Body::EquipOutfit {..} | Body::EquipGear {..}));
                        if mutating {
                            let result = gateway.dispatch_json(id, now, &bytes).map(|bytes| (bytes, gateway.authenticated(id)));
                            dirty = true;
                            pending.push(PendingReply { reply, response: PendingResponse::Outcome(result) });
                        } else if store.is_some() && dirty {
                            pending.push(PendingReply { reply, response: PendingResponse::Read { id, bytes } });
                        } else {
                            let result = gateway.dispatch_json(id, now, &bytes).map(|bytes| (bytes, gateway.authenticated(id)));
                            let _ = reply.send(result);
                        }
                    }
                    Some(Event::Close(id)) => {
                        let _ = gateway.close(id);
                        dirty |= store.is_some();
                    }
                    None => { failure = Some("Chamber dispatch queue closed".into()); break; }
                }
            }
            _ = workers.join_next(), if !workers.is_empty() => { stats.completed_connections += 1; }
        }
    }
    workers.abort_all();
    while workers.join_next().await.is_some() {
        stats.completed_connections += 1;
    }
    if let Err(error) = gateway.close_all() {
        failure.get_or_insert(error);
    }
    if failure.is_none() {
        if let Err(error) = persist(&gateway, &mut store, &mut stats) {
            failure = Some(error);
        }
    }
    Exit {
        gateway,
        stats,
        failure,
    }
}

async fn connection(
    socket: TcpStream,
    acceptor: TlsAcceptor,
    send: mpsc::Sender<Event>,
) -> Result<(), String> {
    socket
        .set_nodelay(true)
        .map_err(|_| "Cannot configure chamber socket")?;
    let mut stream = timeout(HANDSHAKE, acceptor.accept(socket))
        .await
        .map_err(|_| "Chamber TLS handshake timed out")?
        .map_err(|_| "Chamber TLS handshake refused")?;
    let (reply, receive) = oneshot::channel();
    send.send(Event::Open(reply))
        .await
        .map_err(|_| "Chamber host stopped")?;
    let (id, hello) = receive.await.map_err(|_| "Chamber host stopped")??;
    let outcome = async {
        timeout(WRITE, write_frame(&mut stream, &hello, MAX_RESPONSE_BYTES))
            .await
            .map_err(|_| "Chamber write timed out")??;
        let mut authenticated = false;
        let mut window = Instant::now();
        let mut count = 0;
        loop {
            let deadline = if authenticated { IDLE } else { AUTH };
            let bytes = timeout(deadline, read_frame(&mut stream, MAX_REQUEST_BYTES))
                .await
                .map_err(|_| "Chamber read timed out")??;
            if window.elapsed() >= Duration::from_secs(1) {
                window = Instant::now();
                count = 0;
            }
            count += 1;
            if count > REQUESTS_PER_SECOND {
                return Err("Chamber request rate exceeded".into());
            }
            let (reply, receive) = oneshot::channel();
            send.send(Event::Request { id, bytes, reply })
                .await
                .map_err(|_| "Chamber host stopped")?;
            let (bytes, admitted) = receive.await.map_err(|_| "Chamber host stopped")??;
            timeout(WRITE, write_frame(&mut stream, &bytes, MAX_RESPONSE_BYTES))
                .await
                .map_err(|_| "Chamber write timed out")??;
            authenticated = admitted;
            if !authenticated {
                return Err("Chamber connection is not authenticated".into());
            }
        }
    }
    .await;
    let _ = send.send(Event::Close(id)).await;
    outcome
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
    use tokio_rustls::{TlsConnector, client::TlsStream};
    use verse_engine::director::Scene;
    type Stream = TlsStream<TcpStream>;
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
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
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
                assert!(matches!(reply_a.unwrap().body, Reply::Accepted));
                assert!(matches!(reply_b.unwrap().body, Reply::Accepted));
            }
        })
        .await
        .unwrap();
        let elapsed = start.elapsed().as_secs_f64();
        stop.send(()).unwrap();
        let exit = server.await.unwrap();
        assert!(exit.failure.is_none());
        assert!(exit.stats.checkpoint_commits >= 90);
        eprintln!(
            "{}",
            serde_json::json!({"schema":"verse.durable.fixture.v1","players":2,"spectators":1,
            "accepted_movement_commands":180,"wall_seconds":elapsed,"world_ticks":exit.stats.ticks,
            "checkpoint_commits":exit.stats.checkpoint_commits,"checkpoint_bytes":exit.stats.checkpoint_bytes,
            "checkpoint_seconds":exit.stats.checkpoint_seconds,"dropped_seconds":exit.stats.dropped_seconds})
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
        let committed: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("chamber.json")).unwrap()).unwrap();
        let disk = Gateway::restore(
            committed["checkpoint"].as_str().unwrap().as_bytes(),
            [8; 32],
            120,
        )
        .unwrap();
        assert_eq!(
            disk.game()
                .player_hud(old_a.life.into())
                .unwrap()
                .resources
                .mana,
            19
        );
        assert!(
            disk.game()
                .player_hud(old_b.life.into())
                .unwrap()
                .casting
                .is_some()
        );
        server.abort();
        assert!(matches!(server.await, Err(error) if error.is_cancelled()));
        assert!(a.snapshot().await.is_err());
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let recovered = store.recover().unwrap();
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
        assert_eq!(after_a.hud.as_ref().unwrap().resources.mana, 19);
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
        let server = tokio::spawn(serve_durable(
            listener,
            tls,
            gateway(&keys).with_content([8; 32]).unwrap(),
            store,
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
        assert_eq!(exit.failure.as_deref(), Some("Cannot stage chamber commit"));
        assert_eq!(std::fs::read(root.join("chamber.json")).unwrap(), committed);
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
            send(&mut b, 3, Body::Command { command: command_b })
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
        assert!(matches!(
            send(&mut a, 5, Body::Snapshot {}).await.body,
            Reply::Refused { .. }
        ));
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
        assert_eq!(exit.stats.requests, 15);
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
