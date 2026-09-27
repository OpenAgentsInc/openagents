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
    store: coder_host::access::host::Host,
    running: coder_host::Running,
}

async fn fixture(generation: u64) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let (relay, _task, _events) = relay::start().await;
    let access = temp.path().join("access");
    let store = coder_host::access::host::Host::new(&access, POLICY);
    store.init(&pubkey(&key())).unwrap();
    let root = temp.path().join("checkout");
    std::fs::create_dir_all(&root).unwrap();
    let mut config = Config::new(access, vec![relay.clone()], generation);
    config.policy = POLICY;
    config.workspaces =
        BTreeMap::from([("checkout".to_owned(), std::fs::canonicalize(&root).unwrap())]);
    config.ready = Some(Ready {
        file: temp.path().join("run/ready-9.json"),
        version: "e".repeat(64),
    });
    let running = coder_host::start(config, Arc::new(NoTasks)).await.unwrap();
    Fixture {
        temp,
        relay,
        store,
        running,
    }
}

impl Fixture {
    async fn enroll(&self, rights: Rights) -> Arc<Device> {
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
        Arc::new(Device::new(access, secret, POLICY).unwrap())
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
