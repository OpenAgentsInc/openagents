use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use coder_reach::{Refusal, channel::Acceptor};
use secp256k1::Keypair;
use tokio::sync::oneshot;

use super::*;
use crate::service::{
    net::{
        read_frame,
        tests::{gateway, key},
        write_frame,
    },
    wire::{Body, MAX_RESPONSE_BYTES, Request, VERSION},
};

const GRANT: &str = "0101010101010101010101010101010101010101010101010101010101010101";

#[derive(Clone, Copy)]
struct Grant {
    epoch: u64,
    revoked: bool,
    world: bool,
    expired: bool,
}

/// A host grant store in memory. A grant without `world` refuses the way
/// `coder_host::authority::WorldGrants` does.
#[derive(Clone, Default)]
struct Grants(Arc<Mutex<HashMap<String, Grant>>>);
impl Grants {
    fn set(&self, device: &Keypair, grant: Grant) {
        self.0.lock().unwrap().insert(hex(device), grant);
    }
    fn update(&self, device: &Keypair, change: impl FnOnce(&mut Grant)) {
        change(self.0.lock().unwrap().get_mut(&hex(device)).unwrap());
    }
}
impl GrantCheck for Grants {
    fn check(&self, device: &str, grant: &str, epoch: u64, _: u64) -> Result<(), GrantRefusal> {
        let grants = self.0.lock().unwrap();
        let held = grants
            .get(device)
            .filter(|_| grant == GRANT)
            .ok_or(GrantRefusal::Unknown)?;
        if held.revoked {
            Err(GrantRefusal::Revoked)
        } else if held.expired {
            Err(GrantRefusal::Expired)
        } else if held.epoch != epoch {
            Err(GrantRefusal::EpochMismatch)
        } else if !held.world {
            Err(GrantRefusal::Unknown)
        } else {
            Ok(())
        }
    }
}

fn hex(key: &Keypair) -> String {
    key.x_only_public_key().0.to_string()
}

fn granted() -> Grant {
    Grant {
        epoch: 1,
        revoked: false,
        world: true,
        expired: false,
    }
}

fn config(device: &Keypair, host: &Keypair) -> ClientConfig {
    ClientConfig {
        device: device.secret_key(),
        host: hex(host),
        grant: GRANT.into(),
        epoch: 1,
        generation: 7,
        timeout: Duration::from_secs(3),
    }
}

async fn start(
    grants: Grants,
    host: &Keypair,
    carrier: Carrier,
    keys: &[Keypair; 3],
) -> (
    std::net::SocketAddr,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<Exit>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = Server::new(host.secret_key(), 7, grants, carrier);
    let (stop, stopped) = oneshot::channel();
    let task = tokio::spawn(serve(listener, server, gateway(keys), async {
        let _ = stopped.await;
    }));
    (address, stop, task)
}

async fn join_tcp(
    address: std::net::SocketAddr,
    device: &Keypair,
    host: &Keypair,
) -> Result<Client, String> {
    let socket = TcpStream::connect(address).await.unwrap();
    join(socket, &config(device, host), 120, None).await
}

#[tokio::test]
async fn a_channel_pair_carries_chamber_frames_both_ways_unchanged() {
    let (host, device) = (key(40), key(41));
    let grants = Grants::default();
    grants.set(&device, granted());
    let acceptor = Acceptor::new(host.secret_key(), 7, grants, HANDSHAKE);
    let (client_end, host_end) = tokio::io::duplex(64 * 1024);
    let now = unix_now().unwrap();
    let client = config(&device, &host);
    let (opened, accepted) = tokio::join!(
        connect(client_end, &client, now),
        acceptor.accept(host_end, now)
    );
    let alive = Arc::new(AtomicBool::new(true));
    let watched = alive.clone();
    let watch: Watch = (
        Box::new(move || watched.load(Ordering::SeqCst)),
        Duration::from_millis(20),
    );
    let mut device_side = bridge(opened.unwrap(), None);
    let mut host_side = bridge(accepted.unwrap(), Some(watch));

    // Frames larger than one data frame split and rejoin byte for byte.
    let large: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
    for payload in [b"{}".to_vec(), large.clone()] {
        write_frame(&mut device_side, &payload, MAX_RESPONSE_BYTES)
            .await
            .unwrap();
        let received = read_frame(&mut host_side, MAX_RESPONSE_BYTES)
            .await
            .unwrap();
        assert_eq!(received, payload);
        write_frame(&mut host_side, &payload, MAX_RESPONSE_BYTES)
            .await
            .unwrap();
        let received = read_frame(&mut device_side, MAX_RESPONSE_BYTES)
            .await
            .unwrap();
        assert_eq!(received, payload);
    }

    // When the watch refuses, the host's stream ends and the device sees
    // the channel close.
    alive.store(false, Ordering::SeqCst);
    let mut rest = Vec::new();
    timeout(Duration::from_secs(2), host_side.read_to_end(&mut rest))
        .await
        .unwrap()
        .unwrap();
    assert!(rest.is_empty());
    drop(host_side);
    let closed = timeout(
        Duration::from_secs(2),
        read_frame(&mut device_side, MAX_RESPONSE_BYTES),
    )
    .await
    .unwrap();
    assert!(closed.is_err());
}

#[tokio::test]
async fn a_world_grant_admits_its_device_and_refuses_everyone_else() {
    let keys = [key(42), key(43), key(44)];
    let (host, stranger, narrow) = (key(45), key(46), key(47));
    let grants = Grants::default();
    grants.set(&keys[0], granted());
    grants.set(&keys[2], granted());
    grants.set(
        &narrow,
        Grant {
            world: false,
            ..granted()
        },
    );
    let (address, stop, task) = start(grants.clone(), &host, Carrier::Tcp, &keys).await;

    // The primary adventurer's device joins with control.
    let mut player = join_tcp(address, &keys[0], &host).await.unwrap();
    assert!(player.control().is_some());
    player.snapshot().await.unwrap();
    // An enrolled spectator joins without control.
    let spectator = join_tcp(address, &keys[2], &host).await.unwrap();
    assert!(spectator.control().is_none());

    // No grant, a grant without `world`, and a stale epoch are refused at the
    // handshake; a granted key not in the role table joins as a spectator.
    for device in [&stranger, &narrow] {
        let error = join_tcp(address, device, &host).await.err().unwrap();
        assert!(error.contains(Refusal::NotAdmitted.as_str()), "{error}");
    }
    let unlisted = key(48);
    grants.set(&unlisted, granted());
    let mut watcher = join_tcp(address, &unlisted, &host).await.unwrap();
    assert!(watcher.control().is_none());
    watcher.snapshot().await.unwrap();
    grants.update(&unlisted, |grant| grant.epoch = 2);
    assert!(watcher.snapshot().await.is_err());
    let error = join_tcp(address, &unlisted, &host).await.err().unwrap();
    assert!(error.contains(Refusal::Stale.as_str()), "{error}");

    let expired = key(49);
    grants.set(
        &expired,
        Grant {
            expired: true,
            ..granted()
        },
    );
    let error = join_tcp(address, &expired, &host).await.err().unwrap();
    assert!(error.contains(Refusal::Stale.as_str()), "{error}");

    // Revocation drops the open connection at its next request and refuses
    // the next handshake.
    grants.update(&keys[0], |grant| grant.revoked = true);
    assert!(player.snapshot().await.is_err());
    assert!(!player.connected());
    let error = join_tcp(address, &keys[0], &host).await.err().unwrap();
    assert!(error.contains(Refusal::Revoked.as_str()), "{error}");

    let _ = stop.send(());
    let exit = task.await.unwrap();
    assert!(exit.failure.is_none(), "{:?}", exit.failure);
}

#[tokio::test]
async fn an_idle_revoked_connection_is_closed_by_the_recheck() {
    let keys = [key(50), key(51), key(52)];
    let host = key(53);
    let grants = Grants::default();
    grants.set(&keys[2], granted());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = Server::new(host.secret_key(), 7, grants.clone(), Carrier::Tcp)
        .with_recheck(Duration::from_millis(20));
    let (stop, stopped) = oneshot::channel::<()>();
    let task = tokio::spawn(serve(listener, server, gateway(&keys), async {
        let _ = stopped.await;
    }));
    let socket = TcpStream::connect(address).await.unwrap();
    let channel = connect(socket, &config(&keys[2], &host), unix_now().unwrap())
        .await
        .unwrap();
    let mut stream = bridge(channel, None);
    read_frame(&mut stream, MAX_RESPONSE_BYTES).await.unwrap();
    grants.update(&keys[2], |grant| grant.revoked = true);
    // The host closes without waiting for another request.
    let closed = timeout(
        Duration::from_secs(2),
        read_frame(&mut stream, MAX_RESPONSE_BYTES),
    )
    .await
    .unwrap();
    assert!(closed.is_err());
    let _ = stop.send(());
    assert!(task.await.unwrap().failure.is_none());
}

#[tokio::test]
async fn a_websocket_client_joins_through_the_fallback() {
    let keys = [key(54), key(55), key(56)];
    let host = key(57);
    let grants = Grants::default();
    grants.set(&keys[1], granted());
    let (address, stop, task) = start(grants, &host, Carrier::WebSocket, &keys).await;
    let socket = TcpStream::connect(address).await.unwrap();
    let socket = websocket::client(&format!("ws://{address}/"), socket)
        .await
        .unwrap();
    let mut player = join(socket, &config(&keys[1], &host), 120, None)
        .await
        .unwrap();
    assert!(player.control().is_some());
    player.snapshot().await.unwrap();
    player.close().await.unwrap();
    let _ = stop.send(());
    assert!(task.await.unwrap().failure.is_none());
}

#[test]
fn a_channel_carries_only_its_own_devices_key() {
    let (device, other) = (key(58), key(59));
    let grants = Grants::default();
    grants.set(&device, granted());
    let guard = ChannelGuard {
        grants: Arc::new(grants),
        binding: Binding {
            client: hex(&device),
            host: hex(&key(60)),
            grant: GRANT.into(),
            epoch: 1,
            generation: 7,
            transcript: [0; 32],
        },
        device: device.x_only_public_key().0.serialize(),
    };
    let authenticate = |key: &Keypair| {
        serde_json::to_vec(&Request {
            version: VERSION,
            request_id: 1,
            body: Body::Authenticate {
                public_key: key.x_only_public_key().0.serialize(),
                signature: vec![0; 64],
            },
        })
        .unwrap()
    };
    guard.admit(&authenticate(&device)).unwrap();
    assert!(guard.admit(&authenticate(&other)).is_err());
    assert_eq!(
        guard.device(),
        Some(device.x_only_public_key().0.serialize())
    );
}
