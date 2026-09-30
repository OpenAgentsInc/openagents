//! `wss` direct channels that the host terminates itself, with no TLS
//! forwarder: enroll, connect, run a terminal command, and revoke over TLS;
//! refuse an untrusted issuer, a wrong name, and plain `ws`; and refuse to
//! start with a missing, exposed, or mismatched key, an unusable
//! certificate, or a name the certificate does not cover.
//!
//! The certificates and keys in `tests/fixtures/tls` are test-only.

use std::collections::BTreeMap;
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_host::NoTasks;
use coder_host::access::{RelayPolicy, Rights};
use coder_host::client::{
    Connector, Device, Link, Ordered, Reports, Route, WebSocketTls, connect_websocket, fetch_reach,
};
use coder_host::config::{Advertised, Config, WebsocketTls};
use coder_host::link::SystemClock;
use coder_host::link::{BlockReason, Failure, HostKey, Phase, Policy, Registry, Signal, Status};
use coder_host::mailbox::workspace_id;
use coder_host::message::TermRequest;
use coder_host::pty::client::TerminalState;
use coder_host::pty::wire::{
    Attach, Input, Launch, Mode, Open, Size, Status as TermStatus, TerminalResult, Value,
};
use coder_host::reach::hints::{Class, Hint, Locality, Transport, select};
use coder_host::reach::{Refusal, new_id, pubkey};
use secp256k1::SecretKey;
use tokio::net::{TcpListener, TcpStream};

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
const GENERATION: u64 = 7;
const WAIT: Duration = Duration::from_secs(30);
const DIAL: Duration = Duration::from_secs(5);

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn now() -> u64 {
    coder_host::unix_time().unwrap()
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tls")
        .join(name)
}

/// Copy the test-only chain and a key into `dir`, the key with `mode`,
/// since a checkout does not keep a file's owner-only mode.
fn tls_files(dir: &Path, key: &str, mode: u32) -> WebsocketTls {
    std::fs::create_dir_all(dir).unwrap();
    let cert = dir.join("chain.pem");
    let key_path = dir.join("key.pem");
    std::fs::copy(fixture("localhost.pem"), &cert).unwrap();
    std::fs::copy(fixture(key), &key_path).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(mode)).unwrap();
    // Windows has no modes: an owner-only mode is an owner-only DACL, and
    // any other keeps the entries the copy inherited.
    #[cfg(windows)]
    if mode & 0o077 == 0 {
        private_fs::restrict(&key_path).unwrap();
    }
    WebsocketTls {
        cert,
        key: key_path,
        name: "localhost".into(),
    }
}

fn test_roots(name: &str) -> WebSocketTls {
    WebSocketTls::test_roots(&std::fs::read(fixture(name)).unwrap()).unwrap()
}

/// A plain TCP relay of bytes in front of the host's TLS listener. It does
/// not terminate TLS; it only gives the connector an advertised endpoint
/// that the host's TCP listener does not outrank.
async fn passthrough(target: Arc<Mutex<Option<SocketAddr>>>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut inbound, _)) = listener.accept().await {
            let Some(target) = *target.lock().unwrap() else {
                continue;
            };
            tokio::spawn(async move {
                if let Ok(mut outbound) = TcpStream::connect(target).await {
                    let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
                }
            });
        }
    });
    address
}

/// Drive the registry the way an application does until `done` holds.
async fn until(
    registry: &mut Registry<SystemClock, Connector>,
    reports: &mut Reports,
    host: &HostKey,
    what: &str,
    done: impl Fn(&Status) -> bool,
) -> Status {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let status = registry.status(host).unwrap();
        if done(&status) {
            return status;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}; last status {status:?}"
        );
        tokio::select! {
            Some((host, report)) = reports.recv() => {
                let _ = registry.report(&host, report);
            }
            () = tokio::time::sleep(Duration::from_millis(50)) => registry.tick(),
        }
    }
}

/// Enroll a new device with standard rights by invitation.
async fn enroll(store: &coder_host::access::host::Host, relay: &str) -> Arc<Device> {
    let issued = store
        .invite(relay, Rights::standard(), now(), now() + 3600)
        .unwrap();
    let secret = key();
    let access = coder_host::access::client::redeem(&issued.code, &secret, POLICY)
        .await
        .unwrap();
    Arc::new(Device::new(access, secret, POLICY).unwrap())
}

/// The host's current hints for `device`, as a same-machine client
/// selects them.
async fn hints(device: &Device, relay: &str) -> Vec<Hint> {
    let deadline = tokio::time::Instant::now() + WAIT;
    let reach = loop {
        if let Ok(reach) = fetch_reach(device, relay).await {
            break reach;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out: presence"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    select(&reach.hints, Locality::SameMachine, GENERATION, now())
        .unwrap()
        .into_iter()
        .cloned()
        .collect()
}

async fn terminal(link: &Link, request: TermRequest) -> TerminalResult {
    let result = link.terminal(request).await.unwrap();
    assert_eq!(result.status, TermStatus::Accepted, "{result:?}");
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn enroll_connect_over_wss_run_a_command_and_revoke() {
    let temp = tempfile::tempdir().unwrap();
    let (relay, _relay_task, _events) = relay::start().await;
    let owner = key();
    let access_dir = temp.path().join("access");
    let store = coder_host::access::host::Host::new(&access_dir, POLICY);
    let host_key = store.init(&pubkey(&owner)).unwrap();
    let checkout = temp.path().join("checkout");
    std::fs::create_dir_all(&checkout).unwrap();

    let target = Arc::new(Mutex::new(None));
    let front = passthrough(target.clone()).await;
    let advertised = format!("wss://localhost:{}/", front.port());
    let mut config = Config::new(access_dir, vec![relay.clone()], GENERATION);
    config.policy = POLICY;
    config.listen_websocket = Some(SocketAddr::from(([127, 0, 0, 1], 0)));
    config.websocket_tls = Some(tls_files(&temp.path().join("tls"), "localhost.key", 0o600));
    // Only the advertised `wss` endpoint, so the connector must choose it
    // over the TCP listener. The next test checks the listener's own hints.
    config.advertise_listener = false;
    config.advertise = vec![Advertised {
        class: Class::Loopback,
        address: advertised.clone(),
    }];
    config.workspaces = BTreeMap::from([(
        "checkout".to_owned(),
        std::fs::canonicalize(&checkout).unwrap(),
    )]);
    config.recheck_every = Duration::from_millis(100);
    let running = coder_host::start(config, Arc::new(NoTasks)).await.unwrap();
    let listener = running.websocket_addr().unwrap();
    *target.lock().unwrap() = Some(listener);
    let own = format!("wss://localhost:{}/", listener.port());
    assert_eq!(running.websocket_url().as_deref(), Some(own.as_str()));

    // Enroll by invitation.
    let device = enroll(&store, &relay).await;
    assert_eq!(device.host(), host_key);

    // The first direct hint is the advertised `wss` endpoint.
    let chosen = hints(&device, &relay).await;
    assert_eq!(chosen[0].transport, Transport::Websocket);
    assert_eq!(chosen[0].address, advertised);

    // Dial the host's own listener URL directly and prove the channel.
    let tls = test_roots("test-ca.pem");
    let stream = connect_websocket(&own, &tls, DIAL).await.unwrap();
    let own_link = Link::direct(device.clone(), stream, own.clone(), GENERATION, DIAL)
        .await
        .unwrap();
    own_link.ping().await.unwrap();

    // The connector, trusting only the test root, proves the advertised
    // `wss` route.
    let (mut connector, mut reports) =
        Connector::new(tokio::runtime::Handle::current(), Locality::SameMachine);
    connector.set_websocket_tls(tls.clone());
    let host = connector.add(device.clone()).unwrap();
    let policy = Policy {
        establish_timeout: Duration::from_secs(20),
        probe_timeout: Duration::from_secs(20),
        ladder: vec![Duration::from_millis(200), Duration::from_millis(400)],
        ..Policy::default()
    };
    let mut registry = Registry::new(SystemClock::new(), connector, policy).unwrap();
    registry.register(host.clone(), Default::default()).unwrap();
    registry.signal(&host, Signal::Connect).unwrap();
    let status = until(&mut registry, &mut reports, &host, "a connection", |s| {
        s.phase == Phase::Connected
    })
    .await;
    let link = registry
        .connector()
        .link(&host, status.connection.unwrap())
        .unwrap();
    assert_eq!(link.route(), &Route::Direct(advertised.clone()));

    // Run a command in a terminal over the TLS channel.
    let opened = terminal(
        &link,
        TermRequest::Open(Open::new(
            new_id(),
            workspace_id("checkout"),
            "",
            Launch::Shell,
            Size::new(24, 80),
        )),
    )
    .await;
    let Some(Value::Opened {
        terminal: reference,
        ..
    }) = opened.value
    else {
        panic!("open answered {opened:?}")
    };
    terminal(
        &link,
        TermRequest::Attach(Attach::new(
            new_id(),
            reference.clone(),
            Mode::Interact,
            0,
            64 * 1024,
        )),
    )
    .await;
    let command = "printf 'wss-%s\\n' ok\n";
    terminal(
        &link,
        TermRequest::Input(Input::new(new_id(), reference.clone(), command)),
    )
    .await;
    let mut screen = Ordered::new(TerminalState::new(reference, 200, 120));
    let deadline = tokio::time::Instant::now() + WAIT;
    while !screen.state().screen().text().contains("wss-ok") {
        assert!(tokio::time::Instant::now() < deadline, "no command output");
        if let Some(frame) = link.next_frame(Duration::from_millis(200)).await {
            screen.push(frame);
        }
    }

    // Revoking the device closes both open channels with the host's code.
    let revoked = loop {
        match store.revoke(&device.key(), now()) {
            Ok(revoked) => break revoked,
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    };
    assert_eq!(revoked.0, 1);
    let blocked = until(&mut registry, &mut reports, &host, "the close", |s| {
        s.phase == Phase::Blocked(BlockReason::Revoked)
    })
    .await;
    assert_eq!(
        blocked.last_failure,
        Some(Failure::Blocked(BlockReason::Revoked))
    );
    assert_eq!(link.closed(), Some(Some("revoked".into())));
    let deadline = tokio::time::Instant::now() + WAIT;
    while own_link.closed().is_none() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "own link stayed open"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(own_link.closed(), Some(Some("revoked".into())));

    // A new handshake over the host's own hint: TLS succeeds, and the host's
    // signed verdict refuses the grant.
    let stream = connect_websocket(&own, &tls, DIAL).await.unwrap();
    let refused = Link::direct(device.clone(), stream, own.clone(), GENERATION, DIAL)
        .await
        .unwrap_err();
    match refused {
        coder_host::Error::Reach(refusal) => {
            assert_eq!(refusal.code, Refusal::Revoked);
            assert_eq!(refusal.detail, "host refused the channel");
        }
        other => panic!("expected a handshake refusal, got {other:?}"),
    }

    running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_listener_hint_is_wss_and_the_client_refuses_bad_certificates() {
    let temp = tempfile::tempdir().unwrap();
    let (relay, _relay_task, _events) = relay::start().await;
    let access_dir = temp.path().join("access");
    let store = coder_host::access::host::Host::new(&access_dir, POLICY);
    store.init(&pubkey(&key())).unwrap();
    let mut config = Config::new(access_dir, vec![relay.clone()], GENERATION);
    config.policy = POLICY;
    config.listen_websocket = Some(SocketAddr::from(([127, 0, 0, 1], 0)));
    config.websocket_tls = Some(tls_files(&temp.path().join("tls"), "localhost.key", 0o600));
    let running = coder_host::start(config, Arc::new(NoTasks)).await.unwrap();
    let port = running.websocket_addr().unwrap().port();
    let good = format!("wss://localhost:{port}/");

    // The listener's own hint is `wss://NAME:PORT/`, and the host
    // advertises no plain `ws` endpoint beside it.
    let device = enroll(&store, &relay).await;
    let chosen = hints(&device, &relay).await;
    let websocket: Vec<&Hint> = chosen
        .iter()
        .filter(|hint| hint.transport == Transport::Websocket)
        .collect();
    assert_eq!(websocket.len(), 1, "{chosen:?}");
    assert_eq!(websocket[0].address, good);
    assert_eq!(websocket[0].class, Class::Loopback);
    let refusal = |result: coder_host::Result<_>| match result {
        Ok(_) => panic!("the dial succeeded"),
        Err(coder_host::Error::Transport(message)) => message,
        Err(other) => panic!("expected a transport error, got {other:?}"),
    };

    // The trusted root and the certificate's name: the upgrade completes.
    connect_websocket(&good, &test_roots("test-ca.pem"), DIAL)
        .await
        .unwrap();

    // Another root: the issuer is untrusted.
    let message = refusal(connect_websocket(&good, &test_roots("other-ca.pem"), DIAL).await);
    assert!(message.contains("UnknownIssuer"), "{message}");

    // The default WebPKI roots do not include the test root either.
    let message = refusal(connect_websocket(&good, &WebSocketTls::webpki(), DIAL).await);
    assert!(message.contains("UnknownIssuer"), "{message}");

    // The right root, but a name the certificate does not cover.
    let wrong = format!("wss://127.0.0.1:{port}/");
    let message = refusal(connect_websocket(&wrong, &test_roots("test-ca.pem"), DIAL).await);
    assert!(message.contains("not valid for name"), "{message}");

    // Plain `ws` to the TLS listener: no upgrade.
    let plain = format!("ws://localhost:{port}/");
    refusal(connect_websocket(&plain, &test_roots("test-ca.pem"), DIAL).await);

    running.shutdown().await;
}

#[tokio::test]
async fn start_refuses_unusable_tls_files() {
    let temp = tempfile::tempdir().unwrap();
    let start = |tls: WebsocketTls| {
        let mut config = Config::new(
            temp.path().join("no-access-store"),
            vec!["ws://127.0.0.1:9/".into()],
            GENERATION,
        );
        config.policy = POLICY;
        config.listen_websocket = Some(SocketAddr::from(([127, 0, 0, 1], 0)));
        config.websocket_tls = Some(tls);
        async move {
            match coder_host::start(config, Arc::new(NoTasks)).await {
                Ok(_) => panic!("the host started"),
                Err(error) => error.to_string(),
            }
        }
    };
    let expect = |message: String, part: &str| {
        assert!(message.contains(part), "{part:?} not in {message:?}");
    };

    // A missing key.
    let tls = tls_files(&temp.path().join("missing"), "localhost.key", 0o600);
    std::fs::remove_file(&tls.key).unwrap();
    expect(start(tls.clone()).await, "cannot be read");
    // A key that is a directory.
    std::fs::create_dir(&tls.key).unwrap();
    expect(start(tls).await, "not a regular file");

    // A key open to group or others. Wine keeps no DACL on a file, so
    // there only the owner is checked.
    #[cfg(unix)]
    let widened = "open to group or others";
    #[cfg(windows)]
    let widened = "open to no other";
    for mode in [0o640, 0o604, 0o644] {
        if std::env::var_os("OPENAGENTS_TEST_UNDER_WINE").is_some() {
            break;
        }
        let tls = tls_files(
            &temp.path().join(format!("mode-{mode:o}")),
            "localhost.key",
            mode,
        );
        expect(start(tls).await, widened);
    }

    // A key that is not the certificate's.
    let tls = tls_files(&temp.path().join("mismatch"), "mismatched.key", 0o600);
    expect(start(tls).await, "does not match");

    // A certificate file with no certificate, and one that is not a file.
    let mut tls = tls_files(&temp.path().join("bad-cert"), "localhost.key", 0o600);
    std::fs::write(&tls.cert, "not a certificate\n").unwrap();
    expect(start(tls.clone()).await, "holds no certificate");
    tls.cert = temp.path().join("no-such-chain.pem");
    expect(start(tls).await, "cannot be read");

    // A name the certificate does not cover.
    let mut tls = tls_files(&temp.path().join("name"), "localhost.key", 0o600);
    tls.name = "box.example.net".into();
    expect(start(tls).await, "not valid for box.example.net");

    // The same files with the right name and mode pass the TLS checks; the
    // start then fails later, on the missing access store.
    let tls = tls_files(&temp.path().join("good"), "localhost.key", 0o600);
    let message = start(tls).await;
    assert!(!message.contains("TLS"), "{message}");
}
