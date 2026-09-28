//! Host records other programs read, and per-right admission on every path.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use coder_host::access::protocol::{Operation, Outcome};
use coder_host::access::{Code, RelayPolicy, Right, Rights};
use coder_host::client::{Device, Link, fetch_reach};
use coder_host::config::{Config, Ready};
use coder_host::mailbox::{terminal_generation, workspace_id};
use coder_host::message::TermRequest;
use coder_host::pty::wire::{
    Attach, Input, Launch, Mode, Open, Reason, Size, Status, TerminalRef, Value,
};
use coder_host::reach::pubkey;
use coder_host::{Error, NoTasks};
use secp256k1::SecretKey;
use tokio::net::TcpStream;

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

struct Fixture {
    temp: tempfile::TempDir,
    relay: String,
    events: relay::Events,
    store: coder_host::access::host::Host,
    running: coder_host::Running,
}

/// Which workspace the served host is given.
#[derive(Clone, Copy)]
enum Workspace {
    /// A `checkout` label whose root exists.
    Present,
    /// A `checkout` label whose root does not exist on disk.
    Missing,
    /// No workspace at all.
    None,
}

async fn fixture(generation: u64) -> Fixture {
    fixture_with(generation, Workspace::Present).await
}

async fn fixture_with(generation: u64, workspace: Workspace) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let (relay, _task, events) = relay::start().await;
    let access = temp.path().join("access");
    let store = coder_host::access::host::Host::new(&access, POLICY);
    store.init(&pubkey(&key())).unwrap();
    let root = temp.path().join("checkout");
    let mut config = Config::new(access, vec![relay.clone()], generation);
    config.policy = POLICY;
    config.workspaces = match workspace {
        Workspace::Present => {
            std::fs::create_dir_all(&root).unwrap();
            BTreeMap::from([("checkout".to_owned(), std::fs::canonicalize(&root).unwrap())])
        }
        Workspace::Missing => BTreeMap::from([("checkout".to_owned(), root)]),
        Workspace::None => BTreeMap::new(),
    };
    config.ready = Some(Ready {
        file: temp.path().join("run/ready-9.json"),
        version: "e".repeat(64),
    });
    let running = coder_host::start(config, Arc::new(NoTasks)).await.unwrap();
    Fixture {
        temp,
        relay,
        events,
        store,
        running,
    }
}

impl Fixture {
    async fn enroll(&self, rights: Rights) -> Arc<Device> {
        self.enroll_keyed(rights).await.0
    }

    async fn enroll_keyed(&self, rights: Rights) -> (Arc<Device>, SecretKey) {
        let now = coder_host::unix_time().unwrap();
        let code = self
            .store
            .invite(&self.relay, rights, now, now + 3600)
            .unwrap()
            .code;
        let secret = key();
        let access = coder_host::access::client::redeem(&code, &secret, POLICY)
            .await
            .unwrap();
        (
            Arc::new(Device::new(access, secret, POLICY).unwrap()),
            secret,
        )
    }

    async fn direct(&self, device: &Arc<Device>) -> Link {
        let address = self.running.local_addr();
        let stream = TcpStream::connect(address).await.unwrap();
        Link::direct(
            device.clone(),
            stream,
            address.to_string(),
            self.running.generation(),
            Duration::from_secs(5),
        )
        .await
        .unwrap()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ready_record_is_the_one_the_host_service_reads() {
    let fixture = fixture(9).await;
    let bytes = std::fs::read(fixture.temp.path().join("run/ready-9.json")).unwrap();
    let ready: coder_service::launcher::ReadyRecord = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(ready.schema, coder_service::launcher::READY_SCHEMA);
    assert_eq!(ready.generation, 9);
    assert_eq!(ready.version, "e".repeat(64));
    assert_eq!(ready.protocol_version, coder_host::PROTOCOL_VERSION);
    assert!(ready.capabilities.windows(2).all(|pair| pair[0] < pair[1]));
    for flag in &ready.capabilities {
        coder_service::descriptor::validate_capability(flag).unwrap();
    }
    fixture.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn terminal_open_tells_no_workspace_from_a_missing_root() {
    // A host with no workspace cannot serve `terminal.open` at all.
    let fixture = fixture_with(3, Workspace::None).await;
    let operator = fixture.enroll(Rights::standard()).await;
    let error = fixture
        .direct(&operator)
        .await
        .call(Operation::OpenTerminal { cols: 80, rows: 24 })
        .await
        .unwrap_err();
    assert!(
        matches!(&error, Error::Access(e) if e.code == Code::Unsupported),
        "{error:?}"
    );
    fixture.running.shutdown().await;

    // A configured root that is gone is a passing condition.
    let fixture = fixture_with(3, Workspace::Missing).await;
    let operator = fixture.enroll(Rights::standard()).await;
    let direct = fixture.direct(&operator).await;
    let error = direct
        .call(Operation::OpenTerminal { cols: 80, rows: 24 })
        .await
        .unwrap_err();
    assert!(
        matches!(&error, Error::Access(e) if e.code == Code::Unavailable),
        "{error:?}"
    );
    // Creating the directory repairs it without a restart.
    std::fs::create_dir_all(fixture.temp.path().join("checkout")).unwrap();
    let outcome = direct
        .call(Operation::OpenTerminal { cols: 80, rows: 24 })
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Dispatched { .. }), "{outcome:?}");
    fixture.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn terminal_open_dispatches_and_rights_are_checked_per_operation() {
    let fixture = fixture(3).await;
    let operator = fixture.enroll(Rights::standard()).await;
    let observer = fixture.enroll(Rights::new([Right::Observe]).unwrap()).await;
    let host = fixture.running.host_key().to_owned();

    // NIP-HOST `terminal.open` over the relay opens a shell in the first
    // workspace and returns the terminal ID the device attaches to.
    let relayed = Link::relay(operator.clone(), fixture.relay.clone());
    let Outcome::Dispatched { receipt } = relayed
        .call(Operation::OpenTerminal { cols: 80, rows: 24 })
        .await
        .unwrap()
    else {
        panic!("terminal.open answered another outcome")
    };
    assert_eq!(receipt.operation, "terminal.open");
    let reference = TerminalRef {
        generation: terminal_generation(&host, 3),
        terminal: receipt.reference,
    };
    let direct = fixture.direct(&operator).await;
    let attached = direct
        .terminal(TermRequest::Attach(Attach::new(
            coder_host::reach::new_id(),
            reference.clone(),
            Mode::Interact,
            0,
            64 * 1024,
        )))
        .await
        .unwrap();
    assert_eq!(attached.status, Status::Accepted, "{attached:?}");
    // Tasks are not connected on this host, and the device is told so.
    let error = direct
        .call(Operation::CancelTask {
            task: "a".repeat(64),
            revision: 1,
            reason: "Stop".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Access(e) if e.code == Code::Unavailable));

    // An observe-only device reaches the host but holds neither right.
    let watcher = fixture.direct(&observer).await;
    let error = watcher
        .call(Operation::OpenTerminal { cols: 80, rows: 24 })
        .await
        .unwrap_err();
    assert!(
        matches!(&error, Error::Access(e) if e.code == Code::MissingRight && e.missing == Some(Right::Terminal)),
        "{error:?}"
    );
    let error = watcher
        .call(Operation::SteerTask {
            task: "a".repeat(64),
            revision: 1,
            prompt: "Go".into(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(&error, Error::Access(e) if e.missing == Some(Right::Operate)),
        "{error:?}"
    );
    for request in [
        TermRequest::Input(Input::new(
            coder_host::reach::new_id(),
            reference.clone(),
            "true\n",
        )),
        TermRequest::Open(Open::new(
            coder_host::reach::new_id(),
            workspace_id("checkout"),
            "",
            Launch::Shell,
            Size::new(24, 80),
        )),
        TermRequest::Attach(Attach::new(
            coder_host::reach::new_id(),
            reference.clone(),
            Mode::Observe,
            0,
            1024,
        )),
    ] {
        let result = watcher.terminal(request).await.unwrap();
        assert_eq!(result.status, Status::Refused);
        assert_eq!(result.reason, Some(Reason::NotAdmitted));
    }
    // Presence reaches every enrolled device, whatever its rights.
    let mut reach = fetch_reach(&observer, &fixture.relay).await;
    for _ in 0..100 {
        if reach.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        reach = fetch_reach(&observer, &fixture.relay).await;
    }
    assert_eq!(reach.unwrap().presence.presence.generation, 3);
    assert!(matches!(attached.value, Some(Value::Attached { .. })));
    fixture.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_channel_for_another_generation_or_an_unknown_grant_is_refused() {
    let fixture = fixture(5).await;
    let device = fixture.enroll(Rights::standard()).await;
    let address = fixture.running.local_addr();
    let stream = TcpStream::connect(address).await.unwrap();
    let stale = Link::direct(
        device.clone(),
        stream,
        address.to_string(),
        4,
        Duration::from_secs(5),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&stale, Error::Reach(r) if r.code == coder_host::reach::Refusal::Stale),
        "{stale:?}"
    );
    // A key the host never enrolled is refused after it proves itself.
    let stranger = key();
    let config = coder_host::reach::channel::ClientConfig {
        device: stranger,
        host: fixture.running.host_key().to_owned(),
        grant: device.grant().to_owned(),
        epoch: 0,
        generation: 5,
        timeout: Duration::from_secs(5),
    };
    let stream = TcpStream::connect(address).await.unwrap();
    let refused =
        coder_host::reach::channel::connect(stream, &config, coder_host::unix_time().unwrap())
            .await
            .unwrap_err();
    assert_eq!(refused.code, coder_host::reach::Refusal::NotAdmitted);
    fixture.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_nudge_from_an_enrolled_device_brings_fresh_presence_at_once() {
    let fixture = fixture(3).await;
    let device = fixture.enroll(Rights::standard()).await;
    // The host publishes presence when the device set changes.
    let mut first = None;
    for _ in 0..100 {
        if let Ok(reach) = fetch_reach(&device, &fixture.relay).await {
            first = Some(reach.presence.presence.observed_at);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let first = first.expect("the host published presence");
    // Presence repeats only every minute; a nudge brings it now.
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    coder_host::client::nudge(&device, &fixture.relay)
        .await
        .unwrap();
    let mut fresh = false;
    for _ in 0..50 {
        let reach = fetch_reach(&device, &fixture.relay).await.unwrap();
        if reach.presence.presence.observed_at > first {
            fresh = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(fresh, "the nudge brought no fresh presence");
    fixture.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_new_spend_request_wakes_the_phone_whose_grant_it_draws_on() {
    use coder_host::access::spend::{Context, Grant, Purpose, hex};
    use coder_host::spend::wake::Wake;
    use coder_host::spend::{Ask, Book};
    use nostr::x402::test_invoice::{described, signed_at};

    let fixture = fixture(4).await;
    let (device, secret) = fixture.enroll_keyed(Rights::standard()).await;
    let device_key = pubkey(&secret);
    let host = fixture.running.host_key().to_owned();
    let now = coder_host::unix_time().unwrap();
    // The phone hands the host its grant with `spend.list`.
    let link = fixture.direct(&device).await;
    let grant = Grant::request_mode(hex(&[7; 32]), &device_key, &host, 0, now);
    let listed = link
        .call(Operation::ListSpends {
            grant: Box::new(grant),
        })
        .await
        .unwrap();
    assert!(matches!(listed, Outcome::Spends { spends } if spends.is_empty()));
    let wakes = || async {
        fixture
            .events
            .lock()
            .await
            .values()
            .filter(|event| Wake::open(event, &secret, now).is_ok())
            .count()
    };
    assert_eq!(wakes().await, 0, "nothing waits yet");

    // Another process records a request; the host wakes that phone once.
    let book = Book::open(&fixture.temp.path().join("access"));
    let ask = Ask {
        payment: signed_at(
            "lnbc250n",
            described([9; 32], "Search API call", 600),
            false,
            false,
            now,
        ),
        fee_max_msat: None,
        purpose: Purpose::X402Purchase,
        context: Context::default(),
        ttl: 300,
        id: None,
    };
    book.request(&host, &ask, now).unwrap();
    let mut woken = 0;
    for _ in 0..50 {
        woken = wakes().await;
        if woken > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(woken, 1, "the host woke the phone");
    // The wake names only the pair; asking again for the same invoice is the
    // same request and wakes nobody twice.
    book.request(&host, &ask, now).unwrap();
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert_eq!(wakes().await, 1);
    fixture.running.shutdown().await;
}
