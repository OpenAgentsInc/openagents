//! Connect a computer against a real resident host (`coder_host::start`)
//! with its iroh endpoint on loopback, relays disabled, and presence on the
//! synthetic NIP-42 relay: the phone's live service pairs from a connect
//! code over iroh, reaches the host over its saved iroh route (not TCP, not
//! the relay), and creates a task there, as **Run Coder** does.
//!
//! One machine, loopback only; no production relay and no real device.
#![cfg(all(unix, feature = "live"))]

#[path = "../../coder-control/src/tests/relay.rs"]
#[allow(dead_code)]
mod relay;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use coder_computers::ComputersService;
use coder_computers::connect::PairedOver;
use coder_computers::live::{Live, Locality, MemoryStore, Settings};
use coder_computers::model::Platform;
use coder_host::access::host::Host;
use coder_host::access::{RelayPolicy, Rights};
use coder_host::client::Route;
use coder_host::config::{Config, Iroh};
use coder_host::{Code, TaskCreate, TaskRef, Tasks};
use nostr::activity_summary::Phase;
use openagents_connect::code::{CodeParts, ConnectCode};
use secp256k1::SecretKey;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
const WAIT: Duration = Duration::from_secs(45);

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn now() -> u64 {
    coder_host::unix_time().unwrap()
}

/// The tasks the host was asked to create.
#[derive(Default)]
struct Recorder(Mutex<Vec<TaskCreate>>);

impl Tasks for Recorder {
    fn create(&self, _: &str, _: &str, task: &TaskCreate) -> Result<TaskRef, Code> {
        self.0.lock().unwrap().push(task.clone());
        Ok(TaskRef {
            task: "ab".repeat(32),
            revision: 1,
            phase: Phase::Queued,
        })
    }
    fn steer(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn cancel(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
}

#[test]
fn a_phone_pairs_over_loopback_iroh_then_creates_a_task_over_iroh() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());

    // The computer: a resident host serving iroh on loopback.
    let access = temp.path().join("access");
    let store = Host::new(&access, POLICY);
    let host_key = store.init(&coder_host::reach::pubkey(&key())).unwrap();
    let checkout = temp.path().join("checkout");
    std::fs::create_dir_all(&checkout).unwrap();
    let mut config = Config::new(access, vec![relay.clone()], 3);
    config.policy = POLICY;
    config.iroh = Some(Iroh::loopback());
    config.label = "Studio Mac".into();
    config.workspaces = BTreeMap::from([(
        "checkout".to_owned(),
        std::fs::canonicalize(&checkout).unwrap(),
    )]);
    let tasks = Arc::new(Recorder::default());
    let running = runtime
        .block_on(coder_host::start(config, tasks.clone()))
        .unwrap();
    let addr = running.iroh_addr().expect("the host serves iroh");

    // The code the desktop app would show.
    // A connect code carries the pairing rights, and a phone keeps a grant
    // only when it holds exactly those.
    let rights = Rights::pairing();
    let issued = store.invite(&relay, rights, now(), now() + 86_400).unwrap();
    let code = ConnectCode::from_invitation(
        CodeParts {
            host: host_key.clone(),
            endpoint: addr.id,
            issued_at: issued.issued_at,
            relay: None,
            addrs: addr.ip_addrs().copied().collect(),
            label: "Studio Mac".into(),
        },
        &issued.id,
        &issued.capability,
    )
    .unwrap()
    .encode();

    // The phone: on another machine as far as routes go, so the host's
    // loopback TCP hint is never tried and only iroh or the relay remain.
    let mut settings = Settings::new(Platform::Phone);
    settings.policy = POLICY;
    settings.locality = Locality::OtherMachine;
    settings.refresh_every = Duration::from_secs(1);
    settings.iroh_secret = Some(key().secret_bytes());
    settings.iroh_loopback = true;
    settings.connect_relay = relay.clone();
    let mut phone = Live::open(
        settings,
        key(),
        Box::new(MemoryStore::default()),
        runtime.handle().clone(),
    )
    .unwrap();

    let paired = runtime
        .block_on(phone.pairing().pair(&code))
        .unwrap_or_else(|failure| panic!("{failure:?}"));
    assert_eq!(paired.host, host_key);
    assert_eq!(paired.label, "Studio Mac");
    assert_eq!(paired.over, PairedOver::Iroh);

    // The channel comes up over the saved iroh route.
    let deadline = Instant::now() + WAIT;
    let link = loop {
        if let Ok(link) = phone.host_link(&host_key) {
            break link;
        }
        assert!(Instant::now() < deadline, "the phone never connected");
        std::thread::sleep(Duration::from_millis(100));
    };
    match link.route() {
        Route::Direct(address) => assert!(address.starts_with("iroh:"), "{address}"),
        other => panic!("the channel should run over iroh: {other:?}"),
    }

    // Run Coder: task.create over that channel.
    let task = phone
        .create_task(
            &host_key,
            &TaskCreate {
                title: "Fix the flaky test".into(),
                prompt: "Fix the flaky test in the checkout.".into(),
                workspace: "checkout".into(),
                images: Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(task, "ab".repeat(32));
    let created = tasks.0.lock().unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].workspace, "checkout");
}

/// A connect code redeemed on the relay, because this phone has no iroh
/// path, carries no chat invitation. The phone asks for one on the link
/// the service keeps (`chats.invite`, as the app does for any computer it
/// holds no current chat pairing for), and the invitation reads the
/// computer's Coder chats.
#[test]
fn a_phone_paired_on_the_relay_asks_for_its_chats_on_its_link() {
    use coder_host::access::protocol::{Operation, Outcome};

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());

    // The computer serves iroh on loopback and its Coder chats on the relay.
    let access = temp.path().join("access");
    let store = Host::new(&access, POLICY);
    let host_key = store.init(&coder_host::reach::pubkey(&key())).unwrap();
    let tasks_dir = temp.path().join("tasks");
    std::fs::create_dir_all(&tasks_dir).unwrap();
    let mut config = Config::new(access, vec![relay.clone()], 3);
    config.policy = POLICY;
    config.iroh = Some(Iroh::loopback());
    config.label = "Studio Mac".into();
    config.chats = Some(coder_host::tailnet::Chats {
        observer: temp.path().join("observer"),
        sources: coder_connect::coder_history::Config {
            coder: Some(tasks_dir),
            ..coder_connect::coder_history::Config::default()
        },
    });
    config.serve_chats = true;
    let running = runtime
        .block_on(coder_host::start(config, Arc::new(Recorder::default())))
        .unwrap();
    let addr = running.iroh_addr().expect("the host serves iroh");
    // A connect code carries the pairing rights, and a phone keeps a grant
    // only when it holds exactly those.
    let rights = Rights::pairing();
    let issued = store.invite(&relay, rights, now(), now() + 86_400).unwrap();
    let code = ConnectCode::from_invitation(
        CodeParts {
            host: host_key.clone(),
            endpoint: addr.id,
            issued_at: issued.issued_at,
            relay: None,
            addrs: addr.ip_addrs().copied().collect(),
            label: "Studio Mac".into(),
        },
        &issued.id,
        &issued.capability,
    )
    .unwrap()
    .encode();

    // The phone has no iroh key, so the code goes on the relay.
    let mut settings = Settings::new(Platform::Phone);
    settings.policy = POLICY;
    settings.locality = Locality::OtherMachine;
    settings.refresh_every = Duration::from_secs(1);
    settings.connect_relay = relay.clone();
    let secret = key();
    let phone = Live::open(
        settings,
        secret,
        Box::new(MemoryStore::default()),
        runtime.handle().clone(),
    )
    .unwrap();
    let paired = runtime
        .block_on(phone.pairing().pair(&code))
        .unwrap_or_else(|failure| panic!("{failure:?}"));
    assert_eq!(paired.over, PairedOver::Relay);
    assert!(paired.chats.is_none());

    let links = phone.terminals().links(&host_key);
    let deadline = Instant::now() + WAIT;
    let link = loop {
        if let Ok(link) = links() {
            break link;
        }
        assert!(Instant::now() < deadline, "the phone never connected");
        std::thread::sleep(Duration::from_millis(100));
    };
    let Outcome::Chats { invitation, .. } = runtime
        .block_on(link.call(Operation::InviteChats {}))
        .unwrap()
    else {
        panic!("a chat invitation")
    };
    let chats = runtime
        .block_on(coder_connect::pairing::redeem(&invitation, &secret, POLICY))
        .expect("the chat invitation redeems");
    let client = coder_connect::Client::new_with_policy(chats, secret, POLICY).unwrap();
    let page = runtime
        .block_on(client.observe(coder_connect::Query::Catalog(
            coder_connect::coder_history::CatalogRequest::default(),
        )))
        .expect("the catalog reads");
    assert!(matches!(page, coder_connect::Observation::Catalog(_)));
}
