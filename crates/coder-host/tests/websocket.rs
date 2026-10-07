//! WebSocket direct channels end to end: enroll a device by invitation,
//! let the connector choose the host's `websocket` hint, run a command in a
//! terminal over the channel, then revoke the device and see the open
//! channel close with the host's code and a new handshake refused.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_host::NoTasks;
use coder_host::access::{RelayPolicy, Rights};
use coder_host::client::{Connector, Device, Link, Ordered, Reports, Route, fetch_reach};
use coder_host::config::{Advertised, Config};
use coder_host::link::SystemClock;
use coder_host::link::{BlockReason, Failure, HostKey, Phase, Policy, Registry, Signal, Status};
use coder_host::mailbox::workspace_id;
use coder_host::message::TermRequest;
use coder_host::pty::client::TerminalState;
use coder_host::pty::wire::{
    Attach, Input, Launch, Mode, Open, Size, Status as TermStatus, TerminalResult, Value,
};
use coder_host::reach::hints::{Class, Locality, Transport, select};
use coder_host::reach::{Refusal, new_id, pubkey};
use secp256k1::SecretKey;
use tokio::net::{TcpListener, TcpStream};

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
const GENERATION: u64 = 5;
const WAIT: Duration = Duration::from_secs(30);

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn now() -> u64 {
    coder_host::unix_time().unwrap()
}

/// A TCP forwarder in front of the host's WebSocket listener, so the host
/// can advertise an endpoint whose address is known before it binds.
async fn forwarder(target: Arc<Mutex<Option<SocketAddr>>>) -> SocketAddr {
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

async fn terminal(link: &Link, request: TermRequest) -> TerminalResult {
    let result = link.terminal(request).await.unwrap();
    assert_eq!(result.status, TermStatus::Accepted, "{result:?}");
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn enroll_connect_over_websocket_run_a_command_and_revoke() {
    let temp = tempfile::tempdir().unwrap();
    let (relay, _relay_task, _events) = relay::start().await;
    let owner = key();
    let access_dir = temp.path().join("access");
    let store = coder_host::access::host::Host::new(&access_dir, POLICY);
    let host_key = store.init(&pubkey(&owner)).unwrap();
    let checkout = temp.path().join("checkout");
    std::fs::create_dir_all(&checkout).unwrap();

    // The host advertises only a WebSocket endpoint, so the connector must
    // choose the `websocket` hint to go direct.
    let target = Arc::new(Mutex::new(None));
    let front = forwarder(target.clone()).await;
    let url = format!("ws://{front}/");
    let mut config = Config::new(access_dir, vec![relay.clone()], GENERATION);
    config.policy = POLICY;
    config.listen_websocket = Some(SocketAddr::from(([127, 0, 0, 1], 0)));
    config.advertise_listener = false;
    config.advertise = vec![Advertised {
        class: Class::Loopback,
        address: url.clone(),
    }];
    config.workspaces = BTreeMap::from([(
        "checkout".to_owned(),
        std::fs::canonicalize(&checkout).unwrap(),
    )]);
    config.recheck_every = Duration::from_millis(100);
    let running = coder_host::start(config, Arc::new(NoTasks)).await.unwrap();
    let listener = running.websocket_addr().unwrap();
    assert!(listener.ip().is_loopback());
    *target.lock().unwrap() = Some(listener);

    // Enroll by invitation.
    let issued = store
        .invite(&relay, Rights::standard(), now(), now() + 3600)
        .unwrap();
    let secret = key();
    let access = coder_host::access::client::redeem(&issued.code, &secret, POLICY)
        .await
        .unwrap();
    assert_eq!(access.grant.host, host_key);
    let device = Arc::new(Device::new(access, secret, POLICY).unwrap());

    // The host's hints name the WebSocket endpoint.
    let deadline = tokio::time::Instant::now() + WAIT;
    let reach = loop {
        if let Ok(reach) = fetch_reach(&device, &relay).await {
            break reach;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out: presence"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    let chosen = select(&reach.hints, Locality::SameMachine, GENERATION, now()).unwrap();
    assert_eq!(chosen[0].transport, Transport::Websocket);
    assert_eq!(chosen[0].address, url);

    // The connector proves the WebSocket route.
    let (mut connector, mut reports) =
        Connector::new(tokio::runtime::Handle::current(), Locality::SameMachine);
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
    assert_eq!(link.route(), &Route::Direct(url.clone()));
    link.ping().await.unwrap();

    // Run a command in a terminal over the channel.
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
    let command = "printf 'websocket-%s\\n' ok\n";
    terminal(
        &link,
        TermRequest::Input(Input::new(new_id(), reference.clone(), command)),
    )
    .await;
    let mut screen = Ordered::new(TerminalState::new(reference, 200, 120));
    let deadline = tokio::time::Instant::now() + WAIT;
    while !screen.state().screen().text().contains("websocket-ok") {
        assert!(tokio::time::Instant::now() < deadline, "no command output");
        if let Some(frame) = link.next_frame(Duration::from_millis(200)).await {
            screen.push(frame);
        }
    }

    // Revoking the device closes the open channel with the host's code.
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

    // A new WebSocket handshake: the host's signed verdict refuses the grant.
    let stream = TcpStream::connect(front).await.unwrap();
    let socket = coder_host::reach::websocket::client(&url, stream)
        .await
        .unwrap();
    let refused = Link::direct(
        device.clone(),
        socket,
        url.clone(),
        GENERATION,
        Duration::from_secs(5),
    )
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
async fn a_websocket_address_that_cannot_bind_leaves_the_host_serving() {
    // A tailnet address disappears while Tailscale is stopped; the host
    // must still start and serve its other routes.
    let temp = tempfile::tempdir().unwrap();
    let (relay, _relay_task, _events) = relay::start().await;
    let access_dir = temp.path().join("access");
    coder_host::access::host::Host::new(&access_dir, POLICY)
        .init(&pubkey(&key()))
        .unwrap();
    let taken = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut config = Config::new(access_dir, vec![relay], GENERATION);
    config.policy = POLICY;
    config.listen_websocket = Some(taken.local_addr().unwrap());
    let running = coder_host::start(config, Arc::new(NoTasks)).await.unwrap();
    assert_eq!(running.websocket_addr(), None);
    assert_eq!(
        running.websocket_off(),
        Some("the WebSocket listener cannot bind")
    );
    assert!(running.local_addr().ip().is_loopback());
    running.shutdown().await;
}
