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

// The host service, and its launcher, are Unix's.
#[cfg(unix)]
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

/// The output text a link or guest received within `wait`, and whether an
/// attachment ended as revoked.
async fn collect(
    mut next: impl AsyncFnMut(Duration) -> Option<coder_host::client::Incoming>,
    wait: Duration,
) -> (String, bool) {
    use coder_host::pty::wire::{Body, Detached};
    let deadline = tokio::time::Instant::now() + wait;
    let (mut text, mut revoked) = (String::new(), false);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return (text, revoked);
        }
        match next(left).await {
            Some(coder_host::client::Incoming::Frame(frame)) => match frame.body {
                Body::Output { data, .. } => text.push_str(&String::from_utf8_lossy(&data)),
                Body::Detached {
                    reason: Detached::Revoked,
                } => revoked = true,
                _ => {}
            },
            Some(_) => {}
            None => return (text, revoked),
        }
    }
}

async fn open_terminal(fixture: &Fixture, direct: &Link) -> TerminalRef {
    let Outcome::Dispatched { receipt } = direct
        .call(Operation::OpenTerminal { cols: 80, rows: 24 })
        .await
        .unwrap()
    else {
        panic!("terminal.open answered another outcome")
    };
    TerminalRef {
        generation: terminal_generation(fixture.running.host_key(), 3),
        terminal: receipt.reference,
    }
}

/// A device the host never enrolled watches exactly one shared terminal
/// over the relay, from the share on, and loses it when the share ends.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unenrolled_device_watches_one_shared_terminal_from_the_share_on() {
    use coder_host::client::Guest;
    use coder_host::pty::share::{ShareMode, ShareRequest, Unshare};

    let fixture = fixture(3).await;
    let operator = fixture.enroll(Rights::standard()).await;
    let host = fixture.running.host_key().to_owned();
    let direct = fixture.direct(&operator).await;
    let reference = open_terminal(&fixture, &direct).await;
    let other = open_terminal(&fixture, &direct).await;
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
    let typed = |text: &'static str| {
        TermRequest::Input(Input::new(
            coder_host::reach::new_id(),
            reference.clone(),
            text,
        ))
    };
    // Each marker appears only as a command's output, never in its echo.
    let result = direct
        .terminal(typed("printf 'BEFORE%s\\n' X\n"))
        .await
        .unwrap();
    assert_eq!(result.status, Status::Accepted, "{result:?}");
    let (seen, _) = collect(
        async |wait| direct.next_incoming(wait).await,
        Duration::from_secs(2),
    )
    .await;
    assert!(seen.contains("BEFOREX"), "{seen:?}");

    let secret = key();
    let share = ShareRequest::new(
        coder_host::reach::new_id(),
        reference.clone(),
        pubkey(&secret),
        ShareMode::Watch,
        coder_host::unix_time().unwrap() + 600,
    );
    let shared = direct.terminal(TermRequest::Share(share)).await.unwrap();
    let Some(Value::Shared {
        grant,
        authorization,
    }) = shared.value
    else {
        panic!("share: {shared:?}")
    };
    let guest = Guest::new(&authorization, secret, &host, &fixture.relay, POLICY).unwrap();
    assert_eq!(guest.grant(), &grant);
    // The envelope is sealed to the grantee alone.
    assert!(Guest::new(&authorization, key(), &host, &fixture.relay, POLICY).is_err());

    let watching = guest
        .terminal(TermRequest::Attach(Attach::new(
            coder_host::reach::new_id(),
            reference.clone(),
            Mode::Observe,
            0,
            64 * 1024,
        )))
        .await
        .unwrap();
    assert_eq!(watching.status, Status::Accepted, "{watching:?}");
    direct
        .terminal(typed("printf 'AFTER%s\\n' Y\n"))
        .await
        .unwrap();
    let (seen, _) = collect(
        async |wait| guest.next_incoming(wait).await,
        Duration::from_secs(3),
    )
    .await;
    assert!(seen.contains("AFTERY"), "{seen:?}");
    assert!(!seen.contains("BEFOREX"), "{seen:?}");

    // The share admits nothing else: no input, no open, no other terminal.
    for request in [
        typed("whoami\n"),
        TermRequest::Open(Open::new(
            coder_host::reach::new_id(),
            workspace_id("checkout"),
            "",
            Launch::Shell,
            Size::new(24, 80),
        )),
        TermRequest::Attach(Attach::new(
            coder_host::reach::new_id(),
            other.clone(),
            Mode::Observe,
            0,
            1024,
        )),
    ] {
        let result = guest.terminal(request).await.unwrap();
        assert_eq!(result.reason, Some(Reason::NotAdmitted), "{result:?}");
    }

    // Ending the share ends the attachment at once.
    let ended = direct
        .terminal(TermRequest::Unshare(Unshare::one(
            coder_host::reach::new_id(),
            reference.clone(),
            &grant.share,
        )))
        .await
        .unwrap();
    assert_eq!(ended.status, Status::Accepted, "{ended:?}");
    let (_, revoked) = collect(
        async |wait| guest.next_incoming(wait).await,
        Duration::from_secs(3),
    )
    .await;
    assert!(revoked);
    fixture.running.shutdown().await;
}

/// Two devices reopen one saved session without starting anything; after
/// the host restarts the layout and references remain and the old terminal
/// reads lost, and a share opens no session.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_saved_session_reopens_on_two_devices_and_survives_a_restart() {
    use coder_host::pty::ext::{
        Layout, Member, MemberState, Node, SessionList, SessionRead, SessionRecord, SessionRemove,
        SessionWrite, Tab,
    };

    let mut fixture = fixture(3).await;
    let phone = fixture.enroll(Rights::standard()).await;
    let laptop = fixture.enroll(Rights::standard()).await;
    let watcher = fixture.enroll(Rights::new([Right::Observe]).unwrap()).await;
    let direct = fixture.direct(&phone).await;
    let reference = open_terminal(&fixture, &direct).await;
    let record = SessionRecord {
        session: None,
        revision: 0,
        name: "build".into(),
        members: vec![
            Member::Terminal {
                member: 1,
                terminal: reference.clone(),
                state: None,
            },
            Member::Resource {
                member: 2,
                resource: serde_json::json!({"kind": "thread", "thread": "7".repeat(64)}),
            },
        ],
        layout: Layout {
            tabs: vec![Tab {
                name: "main".into(),
                root: Node::Split {
                    axis: coder_host::pty::ext::Axis::Columns,
                    ratio: 600,
                    first: Box::new(Node::Pane { member: 1 }),
                    second: Box::new(Node::Pane { member: 2 }),
                },
            }],
            active: 0,
        },
    };
    let written = direct
        .terminal(TermRequest::SessionWrite(SessionWrite::new(
            coder_host::reach::new_id(),
            None,
            0,
            record.clone(),
        )))
        .await
        .unwrap();
    let Some(Value::Session { record: saved }) = written.value else {
        panic!("write: {written:?}")
    };
    let session = saved.session.clone().unwrap();
    assert_eq!(saved.members[0].state(), Some(MemberState::Live));

    let terminals = fixture.running.terminals();
    let read = |link: Link| {
        let session = session.clone();
        async move {
            let result = link
                .terminal(TermRequest::SessionRead(SessionRead::new(
                    coder_host::reach::new_id(),
                    session,
                )))
                .await
                .unwrap();
            (link, result)
        }
    };
    let (_, from_laptop) = read(fixture.direct(&laptop).await).await;
    let Some(Value::Session { record: reopened }) = from_laptop.value else {
        panic!("read: {from_laptop:?}")
    };
    assert_eq!(reopened.layout, record.layout);
    assert_eq!(reopened.members[1], record.members[1]);
    assert_eq!(
        fixture.running.terminals(),
        terminals,
        "reading opened nothing"
    );

    // Sessions need the terminal right.
    let (_, refused) = read(fixture.direct(&watcher).await).await;
    assert_eq!(refused.reason, Some(Reason::NotAdmitted));

    // The host restarts with a new generation; the record stays.
    drop(direct);
    let access = fixture.temp.path().join("access");
    fixture.running.shutdown().await;
    let mut config = Config::new(access, vec![fixture.relay.clone()], 4);
    config.policy = POLICY;
    fixture.running = coder_host::start(config, Arc::new(NoTasks)).await.unwrap();
    let (link, after) = read(fixture.direct(&phone).await).await;
    let Some(Value::Session { record: restored }) = after.value else {
        panic!("read after restart: {after:?}")
    };
    assert_eq!(restored.members[0].state(), Some(MemberState::Lost));
    assert_eq!(restored.layout, record.layout);
    let listed = link
        .terminal(TermRequest::SessionList(SessionList::new(
            coder_host::reach::new_id(),
        )))
        .await
        .unwrap();
    let Some(Value::Sessions { sessions }) = listed.value else {
        panic!("list: {listed:?}")
    };
    assert_eq!(sessions.len(), 1);
    let removed = link
        .terminal(TermRequest::SessionRemove(SessionRemove::new(
            coder_host::reach::new_id(),
            session,
            restored.revision,
        )))
        .await
        .unwrap();
    assert_eq!(removed.status, Status::Accepted, "{removed:?}");
    fixture.running.shutdown().await;
}

/// An engine status read runs Claude Code's own status command inside the
/// computer and answers with a status that holds no text: the account's
/// email and anything credential-shaped the binary printed stay home. It
/// needs the terminal right, and an unconfigured host runs nothing.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_engine_status_read_answers_only_the_typed_status() {
    use coder_host::pty::engine::{Engine, EngineStatusRead, Method, Plan, State};
    use std::os::unix::fs::PermissionsExt;

    let mut fixture = fixture(5).await;
    let phone = fixture.enroll(Rights::standard()).await;
    let watcher = fixture.enroll(Rights::new([Right::Observe]).unwrap()).await;
    let read = || {
        TermRequest::EngineStatus(EngineStatusRead::new(
            coder_host::reach::new_id(),
            Engine::Claude,
        ))
    };
    let link = fixture.direct(&phone).await;
    let answer = link.terminal(read()).await.unwrap();
    let Some(Value::EngineStatus { status }) = answer.value else {
        panic!("unconfigured: {answer:?}")
    };
    assert_eq!(status.state, State::Unavailable);
    drop(link);

    // A fixture binary in place of the pinned one: it prints the account's
    // email and a credential-shaped value beside the typed fields.
    let token = format!("sk-ant-oat01-{}", "m5".repeat(40));
    let program = fixture.temp.path().join("claude");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\n[ \"$1 $2\" = 'auth status' ] || exit 9\necho '{{\"loggedIn\":true,\"authMethod\":\"claude.ai\",\"subscriptionType\":\"max\",\"email\":\"someone@example.com\",\"orgName\":\"{token}\"}}'\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let notices = fixture.temp.path().join("engine");
    std::fs::create_dir_all(&notices).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    std::fs::write(
        notices.join("claude.json"),
        format!(
            r#"{{"kind":"limited","resets_at":{},"at":{now}}}"#,
            now + 3_600
        ),
    )
    .unwrap();
    let access = fixture.temp.path().join("access");
    fixture.running.shutdown().await;
    let mut config = Config::new(access, vec![fixture.relay.clone()], 6);
    config.policy = POLICY;
    config.engine_status = Some(coder_host::config::EngineStatus {
        program,
        notices: Some(notices),
    });
    fixture.running = coder_host::start(config, Arc::new(NoTasks)).await.unwrap();

    let link = fixture.direct(&phone).await;
    let answer = link.terminal(read()).await.unwrap();
    let raw = serde_json::to_string(&answer).unwrap();
    assert!(
        !raw.contains("sk-ant") && !raw.contains("someone@"),
        "{raw}"
    );
    let Some(Value::EngineStatus { status }) = answer.value else {
        panic!("configured: {answer:?}")
    };
    assert_eq!(
        (status.state, status.method, status.plan, status.resets_at),
        (
            State::RateLimited,
            Some(Method::ClaudeAi),
            Some(Plan::Max),
            Some(now + 3_600)
        )
    );
    drop(link);

    // Observing is not enough to run anything on the computer.
    let link = fixture.direct(&watcher).await;
    let refused = link.terminal(read()).await.unwrap();
    assert_eq!(refused.reason, Some(Reason::NotAdmitted), "{refused:?}");
    fixture.running.shutdown().await;
}
