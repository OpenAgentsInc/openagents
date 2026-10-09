//! The remote-access acceptance run, shared by the `coder-host` fixture and
//! the `coder` binary crate's test with the durable task inbox.
//!
//! One machine, one synthetic NIP-42 relay, one resident host, one device:
//! enroll by invitation, discover the host through the owner directory,
//! connect directly, run a command in a terminal, create a task, lose the
//! direct channel and continue through the relay, reconnect and catch up,
//! then revoke the device and see every later operation refused. Each step
//! asserts the exact result or refusal. The test that includes this module
//! also includes the relay fixture as `crate::relay`.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_host::access::protocol::{Operation, Outcome, TaskCreate};
use coder_host::access::{Code, RelayPolicy, Rights};
use coder_host::client::{
    Connector, Device, Link, Ordered, Reports, Route, fetch_directory, fetch_reach,
    fetch_summaries, publish_directory,
};
use coder_host::config::{Advertised, Config};
use coder_host::link::{
    BlockReason, Failure, HostKey, Phase, Policy, Registry, Signal, Status, SystemClock,
};
use coder_host::mailbox::{terminal_generation, workspace_id};
use coder_host::message::TermRequest;
use coder_host::pty::client::TerminalState;
use coder_host::pty::wire::{
    Attach, Detach, Input, Launch, Mode, Open, Reason, Size, Status as TermStatus, TerminalRef,
    TerminalResult, Value,
};
use coder_host::reach::directory::{Directory, HostEntry};
use coder_host::reach::hints::{Class, Locality, Transport, select};
use coder_host::reach::{Refusal, new_id, pubkey};
use coder_host::{Error, Tasks};
use nostr::activity_summary::Phase as TaskPhase;
use secp256k1::SecretKey;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

pub const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
pub const GENERATION: u64 = 7;
const WAIT: Duration = Duration::from_secs(30);

/// Where the caller builds its task owner.
pub struct Paths {
    #[allow(dead_code, reason = "the durable-inbox test reads it")]
    pub tasks: PathBuf,
    pub workspace: PathBuf,
}

/// What the scenario needs to know about a task in the owner's store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskView {
    pub revision: u64,
    pub title: String,
    pub prompt: String,
    /// `queued`, `cancelled`, and so on, in the owner's own words.
    pub status: String,
    /// Whether the owner started anything for it.
    pub started: bool,
    /// The images the task holds, by digest, with the bytes the owner kept.
    pub images: Vec<(String, Vec<u8>)>,
}

pub type Inspect = Box<dyn Fn(&str) -> Option<TaskView> + Send>;

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn now() -> u64 {
    coder_host::unix_time().unwrap()
}

fn step(name: &str) {
    println!("step: {name}");
}

/// Retry until `probe` yields a value.
async fn eventually<T, F, Fut>(what: &str, mut probe: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        if let Some(value) = probe().await {
            return value;
        }
        assert!(tokio::time::Instant::now() < deadline, "timed out: {what}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// A TCP forwarder the test can cut and restore, standing in for a network
/// path the host cannot observe.
struct Path_ {
    address: SocketAddr,
    open: Arc<AtomicBool>,
    connections: Arc<Mutex<Vec<JoinHandle<()>>>>,
    _accept: JoinHandle<()>,
}

impl Path_ {
    async fn start(target: Arc<Mutex<Option<SocketAddr>>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let open = Arc::new(AtomicBool::new(true));
        let connections: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::default();
        let (gate, held) = (open.clone(), connections.clone());
        let accept = tokio::spawn(async move {
            while let Ok((mut inbound, _)) = listener.accept().await {
                let target = *target.lock().unwrap();
                if !gate.load(Ordering::SeqCst) {
                    continue; // Dropping the stream refuses the connection.
                }
                let Some(target) = target else { continue };
                held.lock().unwrap().push(tokio::spawn(async move {
                    if let Ok(mut outbound) = TcpStream::connect(target).await {
                        let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
                    }
                }));
            }
        });
        Self {
            address,
            open,
            connections,
            _accept: accept,
        }
    }

    /// Drop every forwarded connection and refuse new ones.
    fn cut(&self) {
        self.open.store(false, Ordering::SeqCst);
        for connection in self.connections.lock().unwrap().drain(..) {
            connection.abort();
        }
    }

    fn restore(&self) {
        self.open.store(true, Ordering::SeqCst);
    }
}

/// One host's connection owner, driven the way an application drives it.
struct Supervised {
    registry: Registry<SystemClock, Connector>,
    reports: Reports,
    host: HostKey,
}

impl Supervised {
    async fn until(
        &mut self,
        what: &str,
        done: impl Fn(&Status, Option<&Route>) -> bool,
    ) -> Status {
        let deadline = tokio::time::Instant::now() + WAIT;
        loop {
            let status = self.registry.status(&self.host).unwrap();
            let route = status
                .connection
                .and_then(|c| self.registry.connector().link(&self.host, c))
                .map(|link| link.route().clone());
            if done(&status, route.as_ref()) {
                return status;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for {what}; last status {status:?}"
            );
            tokio::select! {
                Some((host, report)) = self.reports.recv() => {
                    // A report about a superseded attempt changes nothing.
                    let _ = self.registry.report(&host, report);
                }
                () = tokio::time::sleep(Duration::from_millis(50)) => self.registry.tick(),
            }
        }
    }

    fn link(&self) -> Arc<Link> {
        let connection = self
            .registry
            .status(&self.host)
            .unwrap()
            .connection
            .unwrap();
        self.registry
            .connector()
            .link(&self.host, connection)
            .unwrap()
    }
}

async fn terminal(link: &Link, request: TermRequest) -> TerminalResult {
    link.terminal(request).await.unwrap()
}

/// Read frames into `ordered` until the screen shows `needle`.
async fn read_until(link: &Link, ordered: &mut Ordered, needle: &str) {
    let deadline = tokio::time::Instant::now() + WAIT;
    while !ordered.state().screen().text().contains(needle) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {needle:?}; screen {:?}",
            ordered.state().screen().text()
        );
        if let Some(frame) = link.next_frame(Duration::from_millis(200)).await {
            ordered.push(frame);
        }
    }
}

fn attached(result: &TerminalResult) -> String {
    assert_eq!(result.status, TermStatus::Accepted, "{result:?}");
    match &result.value {
        Some(Value::Attached { attachment, .. }) => attachment.clone(),
        other => panic!("attach answered {other:?}"),
    }
}

fn refused(result: &TerminalResult) -> Reason {
    assert_eq!(result.status, TermStatus::Refused, "{result:?}");
    assert!(result.value.is_none());
    result.reason.unwrap()
}

fn access_code(error: Error) -> Code {
    match error {
        Error::Access(error) => error.code,
        other => panic!("expected a host refusal, got {other:?}"),
    }
}

/// Run the whole acceptance path. `build` makes the task owner under test
/// and a way to read a task back from its store.
/// The coding agents the scenario's host lists, as `(engine, state)`
/// (#10119).
pub const ENGINES: [(&str, &str); 4] = [
    ("codex", "ready"),
    ("claude", "limited"),
    ("grok", "ready"),
    ("devin", "not_enabled"),
];

/// [`ENGINES`] as a serving program's `engines_here` lists them. A test
/// that runs the scenario hands it to
/// `coder_host::control::set_local_engines` first.
pub fn engines_here() -> Vec<openagents_chat::router::Engine> {
    use openagents_chat::router::{Engine, EngineState};
    ENGINES
        .iter()
        .map(|(engine, state)| Engine {
            engine: (*engine).into(),
            state: EngineState::of_word(state).unwrap(),
        })
        .collect()
}

pub async fn run(build: impl FnOnce(&Paths) -> (Arc<dyn Tasks>, Inspect)) {
    let temp = tempfile::tempdir().unwrap();
    let paths = Paths {
        tasks: temp.path().join("tasks"),
        workspace: temp.path().join("checkout"),
    };
    std::fs::create_dir_all(&paths.workspace).unwrap();
    let workspace = std::fs::canonicalize(&paths.workspace).unwrap();
    let (tasks, inspect) = build(&paths);

    let (relay, _relay_task, _events) = crate::relay::start().await;
    let owner = key();
    let access_dir = temp.path().join("access");
    let store = coder_host::access::host::Host::new(&access_dir, POLICY);
    let host_key = store.init(&pubkey(&owner)).unwrap();

    // The host listens on loopback behind a forwarder it cannot see, and
    // advertises only the forwarder, so the test can cut the direct path.
    let target = Arc::new(Mutex::new(None));
    let path = Path_::start(target.clone()).await;
    let mut config = Config::new(access_dir.clone(), vec![relay.clone()], GENERATION);
    config.policy = POLICY;
    config.advertise_listener = false;
    config.advertise = vec![Advertised {
        class: Class::Loopback,
        address: path.address.to_string(),
    }];
    config.workspaces = BTreeMap::from([("checkout".to_owned(), workspace.clone())]);
    config.recheck_every = Duration::from_millis(100);
    config.runtime = Some(temp.path().join("host/runtime"));
    let running = coder_host::start(config, tasks).await.unwrap();
    *target.lock().unwrap() = Some(running.local_addr());
    assert_eq!(running.host_key(), host_key);
    let runtime = std::fs::read_to_string(temp.path().join("host/runtime")).unwrap();
    assert_eq!(
        runtime,
        format!(
            "schema=openagents.coder.host-runtime.v1\npid={}\nport={}\n",
            std::process::id(),
            running.local_addr().port()
        )
    );

    step("enroll a device by invitation");
    let issued = {
        let now = now();
        store
            .invite(&relay, Rights::standard(), now, now + 3600)
            .unwrap()
    };
    let secret = key();
    let access = coder_host::access::client::redeem(&issued.code, &secret, POLICY)
        .await
        .unwrap();
    assert_eq!(access.grant.host, host_key);
    assert_eq!(access.grant.rights, Rights::standard());
    assert_eq!(access.grant.epoch, 0);
    assert_eq!(access.grant.relay, relay);
    // The invitation admits one device.
    let other = coder_host::access::client::redeem(&issued.code, &key(), POLICY)
        .await
        .unwrap_err();
    assert_eq!(other.code, Code::Forbidden);
    let device = Arc::new(Device::new(access, secret, POLICY).unwrap());

    step("discover the host through the owner directory");
    let listed = Directory::empty(&pubkey(&owner), now())
        .with_host(
            HostEntry {
                host: host_key.clone(),
                label: "Workstation".into(),
                relays: vec![relay.clone()],
                weight: 100,
                added_at: now(),
                worlds: Vec::new(),
            },
            now(),
        )
        .unwrap();
    publish_directory(&relay, &owner, &listed, &new_id(), now() + 3600, POLICY)
        .await
        .unwrap();
    let directory = fetch_directory(&relay, &owner, POLICY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(directory.revision, 1);
    assert_eq!(directory.hosts.len(), 1);
    let entry = directory.entry(device.host()).unwrap();
    assert_eq!(entry.relays, vec![relay.clone()]);
    // The device's key cannot read the owner's directory.
    assert_eq!(
        fetch_directory(&relay, &secret, POLICY).await.unwrap(),
        None
    );
    let reach = eventually("presence", || async {
        fetch_reach(&device, &entry.relays[0]).await.ok()
    })
    .await;
    assert_eq!(reach.presence.presence.generation, GENERATION);
    assert_eq!(reach.presence.presence.owner, pubkey(&owner));
    assert!(reach.presence.presence.supports("terminal"));
    // The host's coding agents reach the device in its presence, each with
    // its state, in the host's order (#10119), once the host has read them.
    let named = eventually("the host's coding agents in presence", || async {
        fetch_reach(&device, &entry.relays[0])
            .await
            .ok()
            .map(|reach| {
                coder_host::access::protocol::engine_flags(
                    reach
                        .presence
                        .presence
                        .capabilities
                        .iter()
                        .map(String::as_str),
                )
            })
            .filter(|named| !named.is_empty())
    })
    .await;
    let expected: Vec<(String, &str)> = ENGINES
        .iter()
        .map(|(engine, state)| ((*engine).to_owned(), *state))
        .collect();
    assert_eq!(named, expected);
    // Presence carries coarse telemetry, so placement ranks the host rather
    // than skipping it for lack of a sample.
    let telemetry = reach.presence.presence.telemetry.expect("telemetry");
    assert!(telemetry.cpu_count >= 1);
    let client = coder_host::reach::presence::ClientProfile {
        protocol: coder_host::PROTOCOL_VERSION,
        accepts: coder_host::reach::presence::VersionRange {
            min: coder_host::PROTOCOL_VERSION,
            max: coder_host::PROTOCOL_VERSION,
        },
    };
    let assessed = coder_host::reach::placement::assess(
        &[coder_host::reach::placement::Candidate {
            host: &host_key,
            weight: 100,
            presence: Some(&reach.presence),
            admitted: true,
        }],
        &client,
        now(),
        coder_host::reach::presence::Freshness::default(),
        coder_host::reach::placement::Limits {
            max_cpu_utilization_pct: 100,
            min_memory_available_pct: 0,
        },
    );
    assert!(
        !matches!(
            assessed[0],
            coder_host::reach::placement::Assessment::Skipped {
                reason: coder_host::reach::placement::Skip::NoTelemetry,
                ..
            }
        ),
        "{assessed:?}"
    );
    let same = select(&reach.hints, Locality::SameMachine, GENERATION, now()).unwrap();
    assert_eq!(same[0].transport, Transport::Tcp);
    assert_eq!(same[0].address, path.address.to_string());
    // Another machine is never offered a loopback route.
    let other = select(&reach.hints, Locality::OtherMachine, GENERATION, now()).unwrap();
    assert!(other.iter().all(|hint| hint.transport != Transport::Tcp));

    step("connect directly");
    let (mut connector, reports) =
        Connector::new(tokio::runtime::Handle::current(), Locality::SameMachine);
    let host = connector.add(device.clone()).unwrap();
    let policy = Policy {
        establish_timeout: Duration::from_secs(20),
        probe_timeout: Duration::from_secs(20),
        ladder: vec![Duration::from_millis(200), Duration::from_millis(400)],
        ..Policy::default()
    };
    let mut link_owner = Supervised {
        registry: Registry::new(SystemClock::new(), connector, policy).unwrap(),
        reports,
        host: host.clone(),
    };
    link_owner
        .registry
        .register(host.clone(), Default::default())
        .unwrap();
    link_owner.registry.signal(&host, Signal::Connect).unwrap();
    link_owner
        .until("a direct connection", |s, _| s.phase == Phase::Connected)
        .await;
    let direct = link_owner.link();
    assert_eq!(direct.route(), &Route::Direct(path.address.to_string()));
    direct.ping().await.unwrap();

    step("copy files to and from the computer, checked by digest");
    computer_needs_the_terminal_right_and_round_trips_a_file(
        &direct,
        &store,
        &relay,
        &temp.path().join("files"),
    )
    .await;

    step("open a terminal and run a command");
    let generation = terminal_generation(&host_key, GENERATION);
    let opened = terminal(
        &direct,
        TermRequest::Open(Open::new(
            new_id(),
            workspace_id("checkout"),
            "",
            Launch::Shell,
            Size::new(24, 80),
        )),
    )
    .await;
    assert_eq!(opened.status, TermStatus::Accepted, "{opened:?}");
    let Some(Value::Opened {
        terminal: reference,
        size,
    }) = opened.value
    else {
        panic!("open answered {opened:?}")
    };
    assert_eq!(reference.generation, generation);
    assert_eq!(size, Size::new(24, 80));
    let mut screen = Ordered::new(TerminalState::new(reference.clone(), 200, 120));
    let first = attached(
        &terminal(
            &direct,
            TermRequest::Attach(Attach::new(
                new_id(),
                reference.clone(),
                Mode::Interact,
                0,
                64 * 1024,
            )),
        )
        .await,
    );
    let command = "printf 'direct-%s\\n' ok\n";
    let written = terminal(
        &direct,
        TermRequest::Input(Input::new(new_id(), reference.clone(), command)),
    )
    .await;
    assert_eq!(
        written.value,
        Some(Value::Written {
            bytes: command.len() as u64
        })
    );
    read_until(&direct, &mut screen, "direct-ok").await;
    // A request for a terminal from another host generation is lost.
    let lost = terminal(
        &direct,
        TermRequest::Input(Input::new(
            new_id(),
            TerminalRef {
                generation: terminal_generation(&host_key, GENERATION - 1),
                terminal: reference.terminal.clone(),
            },
            "true\n",
        )),
    )
    .await;
    assert_eq!(refused(&lost), Reason::Lost);

    step("create a task");
    let create = TaskCreate {
        title: "Fix the flaky parser test".into(),
        prompt: "Find why the parser test fails one run in ten.".into(),
        workspace: "checkout".into(),
        images: Vec::new(),
        engine: None,
    };
    let Outcome::Dispatched { receipt } = direct
        .call(Operation::CreateTask {
            task: create.clone(),
        })
        .await
        .unwrap()
    else {
        panic!("task.create answered another outcome")
    };
    assert_eq!(receipt.operation, "task.create");
    let task = receipt.reference.clone();
    let view = inspect(&task).unwrap();
    assert_eq!(
        view,
        TaskView {
            revision: 1,
            title: create.title.clone(),
            prompt: create.prompt.clone(),
            status: "queued".into(),
            started: false,
            images: Vec::new(),
        }
    );
    let summary = eventually("the queued summary", || async {
        fetch_summaries(&device, &relay)
            .await
            .ok()?
            .into_iter()
            .find(|s| s.subject == task)
    })
    .await;
    assert_eq!(
        (summary.sequence, summary.phase, summary.headline.as_str()),
        (1, TaskPhase::Queued, "Task queued")
    );
    // A workspace label the host does not admit is refused.
    let unknown = direct
        .call(Operation::CreateTask {
            task: TaskCreate {
                workspace: "elsewhere".into(),
                ..create.clone()
            },
        })
        .await
        .unwrap_err();
    assert_eq!(access_code(unknown), Code::Forbidden);

    step("drop the direct channel and continue through the relay");
    path.cut();
    let lost = link_owner
        .until("the loss of the direct channel", |s, _| {
            s.phase != Phase::Connected
        })
        .await;
    assert_eq!(lost.last_failure, Some(Failure::Closed));
    assert_eq!(direct.closed(), Some(None));
    link_owner
        .until("a relay connection", |s, _| s.phase == Phase::Connected)
        .await;
    let fallback = link_owner.link();
    assert_eq!(fallback.route(), &Route::Relay(relay.clone()));

    step("send a screenshot through the relay and create a task naming it");
    send_screenshot(&fallback, &inspect).await;
    // The direct attachment ended with its transport; the terminal did not.
    let second = attached(
        &terminal(
            &fallback,
            TermRequest::Attach(Attach::new(
                new_id(),
                reference.clone(),
                Mode::Interact,
                screen.state().resume_after(),
                16 * 1024,
            )),
        )
        .await,
    );
    assert_ne!(second, first);
    let command = "printf 'relay-%s\\n' ok\n";
    let written = terminal(
        &fallback,
        TermRequest::Input(Input::new(new_id(), reference.clone(), command)),
    )
    .await;
    assert_eq!(
        written.value,
        Some(Value::Written {
            bytes: command.len() as u64
        })
    );
    read_until(&fallback, &mut screen, "relay-ok").await;
    let Outcome::Dispatched { receipt } = fallback
        .call(Operation::SteerTask {
            task: task.clone(),
            revision: 1,
            prompt: "Only look at the tokenizer.".into(),
        })
        .await
        .unwrap()
    else {
        panic!("task.steer answered another outcome")
    };
    assert_eq!(
        (receipt.operation.as_str(), receipt.reference.as_str()),
        ("task.steer", task.as_str())
    );
    let view = inspect(&task).unwrap();
    assert_eq!(
        (view.revision, view.prompt.as_str()),
        (2, "Only look at the tokenizer.")
    );
    // A steer at the old revision is stale.
    let stale = fallback
        .call(Operation::SteerTask {
            task: task.clone(),
            revision: 1,
            prompt: "Again".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(access_code(stale), Code::Stale);
    // Output written while no attachment reads it waits in the host's buffer.
    let written = terminal(
        &fallback,
        TermRequest::Input(Input::new(
            new_id(),
            reference.clone(),
            "sleep 1; printf 'later-%s\\n' ok\n",
        )),
    )
    .await;
    assert_eq!(written.status, TermStatus::Accepted);
    let detached = terminal(
        &fallback,
        TermRequest::Detach(Detach::new(new_id(), reference.clone(), second.clone())),
    )
    .await;
    assert_eq!(detached.value, Some(Value::Done));
    // Take every frame the relay already delivered for that attachment.
    while let Some(frame) = fallback.next_frame(Duration::from_millis(500)).await {
        screen.push(frame);
    }
    let resume = screen.state().resume_after();
    assert!(!screen.state().screen().text().contains("later-ok"));

    step("reconnect directly and catch up");
    tokio::time::sleep(Duration::from_millis(1500)).await; // The command finishes.
    path.restore();
    link_owner.registry.signal(&host, Signal::RetryNow).unwrap();
    link_owner
        .until("a direct connection again", |s, route| {
            s.phase == Phase::Connected && matches!(route, Some(Route::Direct(_)))
        })
        .await;
    let again = link_owner.link();
    assert_eq!(again.route(), &Route::Direct(path.address.to_string()));
    attached(
        &terminal(
            &again,
            TermRequest::Attach(Attach::new(
                new_id(),
                reference.clone(),
                Mode::Interact,
                resume,
                64 * 1024,
            )),
        )
        .await,
    );
    read_until(&again, &mut screen, "later-ok").await;
    // Catching up replayed exactly the missed frames: no gap, no loss.
    assert_eq!(screen.state().missing().count(), 0);
    assert!(!screen.state().behind());
    let text = screen.state().screen().text();
    for needle in ["direct-ok", "relay-ok", "later-ok"] {
        assert_eq!(text.matches(needle).count(), 1, "{needle} in {text:?}");
    }
    let summary = eventually("the steered summary", || async {
        fetch_summaries(&device, &relay)
            .await
            .ok()?
            .into_iter()
            .find(|s| s.subject == task && s.sequence == 2)
    })
    .await;
    assert_eq!(summary.phase, TaskPhase::Queued);
    let connection = link_owner
        .registry
        .status(&host)
        .unwrap()
        .connection
        .unwrap();
    link_owner
        .registry
        .report(&host, coder_host::link::Report::DataCurrent(connection))
        .unwrap();
    assert!(matches!(
        link_owner.registry.status(&host).unwrap().freshness,
        coder_host::link::Freshness::Current { .. }
    ));

    step("revoke the device");
    let owner_client = coder_host::access::Client::owner(&host_key, &relay, owner, POLICY).unwrap();
    let Outcome::Revoked {
        device: revoked,
        epoch,
        grants,
    } = owner_client
        .call(Operation::Revoke {
            device: device.key(),
        })
        .await
        .unwrap()
    else {
        panic!("device.revoke answered another outcome")
    };
    assert_eq!(
        (revoked.as_str(), epoch, grants.len()),
        (device.key().as_str(), 1, 1)
    );

    step("every later operation is refused");
    // The open channel closes with the host's code.
    let blocked = link_owner
        .until("the revoked channel to close", |s, _| {
            s.phase == Phase::Blocked(BlockReason::Revoked)
        })
        .await;
    assert_eq!(
        blocked.last_failure,
        Some(Failure::Blocked(BlockReason::Revoked))
    );
    assert_eq!(again.closed(), Some(Some("revoked".into())));
    let closed = again
        .call(Operation::CreateTask {
            task: create.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!(closed, Error::Closed(Some("revoked".into())));
    // A new direct handshake: the host's signed verdict refuses the grant.
    let stream = TcpStream::connect(path.address).await.unwrap();
    let handshake = Link::direct(
        device.clone(),
        stream,
        path.address.to_string(),
        GENERATION,
        Duration::from_secs(5),
    )
    .await
    .unwrap_err();
    match handshake {
        Error::Reach(refusal) => {
            assert_eq!(refusal.code, Refusal::Revoked);
            assert_eq!(refusal.detail, "host refused the channel");
        }
        other => panic!("expected a handshake refusal, got {other:?}"),
    }
    // Relay-carried host operations.
    let relayed = Link::relay(device.clone(), relay.clone());
    for op in [
        Operation::CreateTask {
            task: create.clone(),
        },
        Operation::SteerTask {
            task: task.clone(),
            revision: 2,
            prompt: "One more try".into(),
        },
        Operation::CancelTask {
            task: task.clone(),
            revision: 2,
            reason: "Stop".into(),
        },
        Operation::OpenTerminal { cols: 80, rows: 24 },
    ] {
        let name = op.name();
        let error = relayed.call(op).await.unwrap_err();
        assert_eq!(access_code(error), Code::Revoked, "{name}");
    }
    // Relay-carried terminal operations.
    for request in [
        TermRequest::Input(Input::new(new_id(), reference.clone(), "echo nope\n")),
        TermRequest::Attach(Attach::new(
            new_id(),
            reference.clone(),
            Mode::Observe,
            0,
            1024,
        )),
        TermRequest::Open(Open::new(
            new_id(),
            workspace_id("checkout"),
            "",
            Launch::Shell,
            Size::new(24, 80),
        )),
    ] {
        let result = relayed.terminal(request).await.unwrap();
        assert_eq!(refused(&result), Reason::Revoked);
    }
    // The connection owner stays blocked: a retry proves nothing new.
    link_owner.registry.signal(&host, Signal::RetryNow).unwrap();
    let retried = link_owner
        .until("the retry to settle", |s, _| {
            !matches!(s.phase, Phase::Connecting(_))
        })
        .await;
    assert_eq!(retried.phase, Phase::Blocked(BlockReason::Revoked));
    // Nothing reached the task owner after revocation.
    let view = inspect(&task).unwrap();
    assert_eq!((view.revision, view.status.as_str()), (2, "queued"));
    assert!(!view.started);

    running.shutdown().await;
    assert!(!temp.path().join("host/runtime").exists());
    step("done");
}

/// NIP-HOST `computer` over a real direct channel: a file pushed and pulled
/// back matches by digest, an existing file is kept without `overwrite`,
/// a missing file answers a sentence, and a device without `terminal` is
/// refused before anything is read.
async fn computer_needs_the_terminal_right_and_round_trips_a_file(
    direct: &Link,
    store: &coder_host::access::host::Host,
    relay: &str,
    dir: &std::path::Path,
) {
    use coder_host::access::computer::{self, Answer, MAX_FILE_BYTES, Request};
    std::fs::create_dir_all(dir).unwrap();
    let handle = tokio::runtime::Handle::current();
    let mut call = |request: Request| {
        tokio::task::block_in_place(|| {
            handle.block_on(direct.call(Operation::Computer { computer: request }))
        })
        .map_err(|error| match error {
            Error::Access(error) => error,
            other => coder_host::access::Error::new(Code::Transport, other.to_string()),
        })
        .and_then(|outcome| match outcome {
            Outcome::Computer { computer } => Ok(computer),
            _ => panic!("computer answered another outcome"),
        })
    };
    let bytes: Vec<u8> = (0..(3 * computer::CHUNK_BYTES as u32 + 777))
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
        .collect();
    let remote = dir.join("pushed.bin").display().to_string();
    let digest = computer::send(&mut call, &remote, &bytes, false, &mut |_, _| {}).unwrap();
    assert_eq!(digest, computer::digest(&bytes));
    assert_eq!(std::fs::read(&remote).unwrap(), bytes);
    let mut back = Vec::new();
    let file = computer::fetch(
        &mut call,
        &remote,
        MAX_FILE_BYTES,
        &mut back,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(back, bytes);
    assert_eq!(file.digest, digest);
    let kept = computer::send(&mut call, &remote, b"other", false, &mut |_, _| {}).unwrap_err();
    assert_eq!(kept.code, Code::Conflict);
    assert_eq!(std::fs::read(&remote).unwrap(), bytes);
    let missing = call(Request::Stat {
        path: dir.join("nothing").display().to_string(),
    })
    .unwrap();
    assert!(matches!(missing, Answer::Unable { reason } if reason.contains("no file")));

    // A device whose grant leaves out `terminal` is refused.
    let now = now();
    let issued = store
        .invite(
            relay,
            Rights::parse_list("observe,operate").unwrap(),
            now,
            now + 3600,
        )
        .unwrap();
    let secret = key();
    let access = coder_host::access::client::redeem(&issued.code, &secret, POLICY)
        .await
        .unwrap();
    let narrow = Link::relay(
        Arc::new(Device::new(access, secret, POLICY).unwrap()),
        relay.to_owned(),
    );
    let refused = narrow
        .call(Operation::Computer {
            computer: Request::Stat { path: remote },
        })
        .await
        .unwrap_err();
    let Error::Access(refused) = refused else {
        panic!("computer answered {refused:?}")
    };
    assert_eq!(refused.code, Code::MissingRight);
}

/// A screenshot reaches the task owner as its exact bytes, chunk by chunk,
/// through the relay binding's message bound; a task names it by digest.
async fn send_screenshot(link: &Link, inspect: &Inspect) {
    use coder_host::access::media::{self, ArtifactState, Upload};
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend((0..100_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8));
    let upload = Upload::new("Settings layout.png", Arc::new(bytes.clone())).unwrap();
    let mut last = None;
    for chunk in upload.chunks(0) {
        let answered = link
            .call(Operation::PutArtifact {
                artifact: chunk.clone(),
            })
            .await
            .unwrap();
        let Outcome::Artifact { artifact } = answered else {
            panic!("artifact.put answered another outcome")
        };
        // An exact retry after a lost reply answers the same.
        let again = link
            .call(Operation::PutArtifact { artifact: chunk })
            .await
            .unwrap();
        assert_eq!(
            again,
            Outcome::Artifact {
                artifact: artifact.clone()
            }
        );
        last = Some(artifact);
    }
    let ArtifactState {
        complete, received, ..
    } = last.unwrap();
    assert!(complete);
    assert_eq!(received, bytes.len() as u64);
    let create = TaskCreate {
        title: "Fix the settings layout".into(),
        prompt: "Fix the layout in this screenshot.".into(),
        workspace: "checkout".into(),
        images: vec![upload.reference.clone()],
        engine: None,
    };
    let Outcome::Dispatched { receipt } = link
        .call(Operation::CreateTask { task: create })
        .await
        .unwrap()
    else {
        panic!("task.create answered another outcome")
    };
    let view = inspect(&receipt.reference).unwrap();
    assert_eq!(
        view.images,
        vec![(upload.reference.digest.clone(), bytes.clone())]
    );
    assert_eq!(media::digest(&view.images[0].1), upload.reference.digest);
    // A task naming an image this device never sent is refused, and so is
    // a chunk that is not PNG or JPEG.
    let mut other = upload.reference.clone();
    other.digest = media::digest(b"never sent");
    let refused = link
        .call(Operation::CreateTask {
            task: TaskCreate {
                title: "Another".into(),
                prompt: "Another".into(),
                workspace: "checkout".into(),
                images: vec![other],
                engine: None,
            },
        })
        .await
        .unwrap_err();
    assert_eq!(access_code(refused), Code::Conflict);
}
