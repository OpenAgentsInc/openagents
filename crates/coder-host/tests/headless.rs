//! Reverse enrollment of a headless host, end to end: a real resident host
//! on a local NIP-42 relay publishes an enrollment request, and an
//! administrator's device approves or denies it over the relay.
//!
//! Each test starts its own relay and host. The host has no screen: it
//! learns nothing from the approver except the signed decision, and the
//! code reaches the approver only as the test's stand-in for a person
//! reading it off an SSH session.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use coder_host::access::client::{OpenedEnrollment, open_enrollment, pending_enrollments};
use coder_host::access::host::{EnrollmentStatus, Host};
use coder_host::access::protocol::{Operation, Outcome};
use coder_host::access::{Access, Client, Code, RelayPolicy, Right, Rights};
use coder_host::client::{Device, Link, Ordered, fetch_reach};
use coder_host::config::Config;
use coder_host::enroll;
use coder_host::mailbox::{terminal_generation, workspace_id};
use coder_host::message::TermRequest;
use coder_host::pty::client::TerminalState;
use coder_host::pty::wire::{
    Attach, Input, Launch, Mode, Open, Size, Status as TermStatus, TerminalResult, Value,
};
use coder_host::reach::hints::{Locality, Transport, select};
use coder_host::reach::{new_id, pubkey};
use coder_host::{NoTasks, Running};
use secp256k1::SecretKey;
use tokio::net::TcpStream;

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
const GENERATION: u64 = 11;
const WAIT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(50);

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn now() -> u64 {
    coder_host::unix_time().unwrap()
}

/// One relay, one resident host without a screen, its owner, and one
/// administrator device enrolled by invitation.
struct Fixture {
    temp: tempfile::TempDir,
    relay: String,
    _relay_task: tokio::task::JoinHandle<()>,
    owner: SecretKey,
    host_key: String,
    access_dir: std::path::PathBuf,
    running: Running,
    admin_key: SecretKey,
    admin: Client,
}

impl Fixture {
    async fn start() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("checkout");
        std::fs::create_dir_all(&workspace).unwrap();
        let workspace = std::fs::canonicalize(&workspace).unwrap();
        let (relay, relay_task, _events) = relay::start().await;
        let owner = key();
        let access_dir = temp.path().join("access");
        let host_key = Host::new(&access_dir, POLICY)
            .init(&pubkey(&owner))
            .unwrap();

        let mut config = Config::new(access_dir.clone(), vec![relay.clone()], GENERATION);
        config.policy = POLICY;
        config.workspaces = BTreeMap::from([("checkout".to_owned(), workspace)]);
        config.recheck_every = Duration::from_millis(100);
        let running = coder_host::start(config, Arc::new(NoTasks)).await.unwrap();

        // The administrator holds every right, so its approvals are bounded
        // only by the request.
        let issued = {
            let now = now();
            Host::new(&access_dir, POLICY)
                .invite(&relay, Rights::all(), now, now + 3600)
                .unwrap()
        };
        let admin_key = key();
        let access = coder_host::access::client::redeem(&issued.code, &admin_key, POLICY)
            .await
            .unwrap();
        assert!(access.grant.rights.contains(Right::AccessAdmin));
        let admin = Client::device(access, admin_key, POLICY).unwrap();
        Self {
            temp,
            relay,
            _relay_task: relay_task,
            owner,
            host_key,
            access_dir,
            running,
            admin_key,
            admin,
        }
    }

    /// The headless host publishes a request for standard rights.
    async fn request(&self) -> enroll::Requested {
        let requested = enroll::request(&self.access_dir, POLICY, &self.relay, Rights::standard())
            .await
            .unwrap();
        // One sealed copy to the owner and one to the administrator.
        assert_eq!(requested.recipients, 2);
        requested
    }

    /// The administrator's device finds the request the host sealed to it.
    async fn open(&self, id: &str) -> OpenedEnrollment {
        let found = pending_enrollments(&self.relay, &self.admin_key, &self.host_key, POLICY)
            .await
            .unwrap();
        found
            .into_iter()
            .find(|opened| opened.enrollment.enrollment == id)
            .expect("the administrator received the request")
    }

    fn status(&self, id: &str) -> EnrollmentStatus {
        enroll::status(&self.access_dir, POLICY, id).unwrap()
    }

    fn devices(&self) -> Vec<String> {
        Host::new(&self.access_dir, POLICY)
            .devices(now())
            .unwrap()
            .into_iter()
            .map(|entry| entry.device)
            .collect()
    }

    /// A device the owner enrolls by invitation with `rights`.
    async fn enroll(&self, rights: Rights) -> (SecretKey, Client) {
        let issued = {
            let now = now();
            Host::new(&self.access_dir, POLICY)
                .invite(&self.relay, rights, now, now + 3600)
                .unwrap()
        };
        let secret = key();
        let access = coder_host::access::client::redeem(&issued.code, &secret, POLICY)
            .await
            .unwrap();
        let client = Client::device(access, secret, POLICY).unwrap();
        (secret, client)
    }
}

fn refusal(result: coder_host::access::Result<Outcome>) -> (Code, Option<Right>) {
    let error = result.expect_err("the host must refuse");
    (error.code, error.missing)
}

fn wrong_code(code: &str) -> String {
    if code.starts_with('0') {
        "1111-1111".into()
    } else {
        "0000-0000".into()
    }
}

async fn terminal(link: &Link, request: TermRequest) -> TerminalResult {
    link.terminal(request).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_administrator_approves_a_headless_host_and_the_new_device_opens_a_terminal() {
    let f = Fixture::start().await;
    let requested = f.request().await;
    assert_eq!(f.status(&requested.id), EnrollmentStatus::Pending);
    // The host waits on its own store; the resident host answers the relay.
    let waiter = {
        let (dir, id) = (f.access_dir.clone(), requested.id.clone());
        tokio::spawn(async move { enroll::wait(&dir, POLICY, &id, POLL).await })
    };

    let opened = f.open(&requested.id).await;
    assert_eq!(opened.enrollment.host, f.host_key);
    assert_eq!(opened.enrollment.rights, Rights::standard());
    // The published request never carries the code.
    let body = serde_json::to_string(&opened.enrollment).unwrap();
    assert!(!body.contains(&requested.code));

    // The person types the code as shown, in lowercase without the dash;
    // the host normalizes it.
    let typed = requested.code.replace('-', "").to_lowercase();
    let laptop = key();
    let approval = opened.approve(&typed, &pubkey(&laptop), Rights::standard(), now() + 3600);
    let Outcome::Granted { authorization } = f.admin.call(approval.clone()).await.unwrap() else {
        panic!("the approval must grant")
    };
    let access =
        Access::from_authorization(*authorization, &laptop, &f.host_key, now(), POLICY).unwrap();
    assert_eq!(access.grant.device, pubkey(&laptop));
    assert_eq!(access.grant.rights, Rights::standard());
    assert_eq!(access.grant.relay, f.relay);

    let decided = tokio::time::timeout(WAIT, waiter)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        decided,
        EnrollmentStatus::Approved {
            device: pubkey(&laptop),
            grant: access.grant.grant.clone(),
        }
    );
    // Repeating the approval returns the same grant; another device conflicts.
    let Outcome::Granted { authorization } = f.admin.call(approval).await.unwrap() else {
        panic!("a repeated approval must grant")
    };
    assert_eq!(
        authorization.tag_values("h").next(),
        Some(access.grant.grant.as_str())
    );
    let other = opened.approve(
        &requested.code,
        &pubkey(&key()),
        Rights::standard(),
        now() + 3600,
    );
    assert_eq!(refusal(f.admin.call(other).await).0, Code::Conflict);

    // The new device reads presence, connects directly, and runs a command.
    let device = Arc::new(Device::new(access, laptop, POLICY).unwrap());
    let deadline = tokio::time::Instant::now() + WAIT;
    let reach = loop {
        if let Ok(reach) = fetch_reach(&device, &f.relay).await {
            break reach;
        }
        assert!(tokio::time::Instant::now() < deadline, "no presence");
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    assert_eq!(reach.presence.presence.generation, GENERATION);
    let hints = select(&reach.hints, Locality::SameMachine, GENERATION, now()).unwrap();
    let hint = hints
        .iter()
        .find(|hint| hint.transport == Transport::Tcp)
        .expect("a direct TCP hint");
    assert_eq!(hint.address, f.running.local_addr().to_string());
    let stream = TcpStream::connect(&hint.address).await.unwrap();
    let link = Link::direct(
        device.clone(),
        stream,
        hint.address.clone(),
        GENERATION,
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    link.ping().await.unwrap();

    let opened_terminal = terminal(
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
    assert_eq!(
        opened_terminal.status,
        TermStatus::Accepted,
        "{opened_terminal:?}"
    );
    let Some(Value::Opened {
        terminal: reference,
        ..
    }) = opened_terminal.value
    else {
        panic!("open answered {opened_terminal:?}")
    };
    assert_eq!(
        reference.generation,
        terminal_generation(&f.host_key, GENERATION)
    );
    let attached = terminal(
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
    assert_eq!(attached.status, TermStatus::Accepted, "{attached:?}");
    let command = "printf 'headless-%s\\n' approved\n";
    let written = terminal(
        &link,
        TermRequest::Input(Input::new(new_id(), reference.clone(), command)),
    )
    .await;
    assert_eq!(written.status, TermStatus::Accepted, "{written:?}");
    let mut screen = Ordered::new(TerminalState::new(reference, 200, 120));
    let deadline = tokio::time::Instant::now() + WAIT;
    while !screen.state().screen().text().contains("headless-approved") {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out; screen {:?}",
            screen.state().screen().text()
        );
        if let Some(frame) = link.next_frame(Duration::from_millis(200)).await {
            screen.push(frame);
        }
    }
    link.shutdown();
    f.running.shutdown().await;
    drop(f.temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_denied_request_admits_nothing() {
    let f = Fixture::start().await;
    let requested = f.request().await;
    let waiter = {
        let (dir, id) = (f.access_dir.clone(), requested.id.clone());
        tokio::spawn(async move { enroll::wait(&dir, POLICY, &id, POLL).await })
    };
    let opened = f.open(&requested.id).await;
    assert_eq!(
        f.admin.call(opened.deny()).await.unwrap(),
        Outcome::Denied {}
    );
    let decided = tokio::time::timeout(WAIT, waiter)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(decided, EnrollmentStatus::Denied);
    // Denial is terminal, even with the right code.
    let late = opened.approve(
        &requested.code,
        &pubkey(&key()),
        Rights::standard(),
        now() + 3600,
    );
    assert_eq!(refusal(f.admin.call(late).await).0, Code::Denied);
    assert_eq!(f.devices(), vec![pubkey(&f.admin_key)]);
    f.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn five_wrong_codes_close_the_request() {
    let f = Fixture::start().await;
    let requested = f.request().await;
    let opened = f.open(&requested.id).await;
    let device = pubkey(&key());
    let wrong = wrong_code(&requested.code);
    for attempt in 1..=5 {
        let op = opened.approve(&wrong, &device, Rights::standard(), now() + 3600);
        assert_eq!(
            refusal(f.admin.call(op).await).0,
            Code::WrongCode,
            "attempt {attempt}"
        );
        let expected = if attempt < 5 {
            EnrollmentStatus::Pending
        } else {
            EnrollmentStatus::Closed
        };
        assert_eq!(f.status(&requested.id), expected, "attempt {attempt}");
    }
    // The right code no longer admits anything.
    let op = opened.approve(&requested.code, &device, Rights::standard(), now() + 3600);
    assert_eq!(refusal(f.admin.call(op).await).0, Code::RateLimited);
    let decided = enroll::wait(&f.access_dir, POLICY, &requested.id, POLL)
        .await
        .unwrap();
    assert_eq!(decided, EnrollmentStatus::Closed);
    assert_eq!(f.devices(), vec![pubkey(&f.admin_key)]);
    f.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_device_without_access_admin_cannot_approve() {
    let f = Fixture::start().await;
    let (operator_key, operator) = f.enroll(Rights::standard()).await;
    let requested = f.request().await;
    // The request is sealed only to the owner and the administrator.
    let visible = pending_enrollments(&f.relay, &operator_key, &f.host_key, POLICY)
        .await
        .unwrap();
    assert!(visible.is_empty());

    // Even holding the exact request and the right code, the operator is
    // refused for the right it lacks, and the attempt costs nothing.
    let opened = f.open(&requested.id).await;
    let op = opened.approve(
        &requested.code,
        &pubkey(&key()),
        Rights::parse_list("observe").unwrap(),
        now() + 3600,
    );
    assert_eq!(
        refusal(operator.call(op).await),
        (Code::MissingRight, Some(Right::AccessAdmin))
    );
    assert_eq!(
        refusal(operator.call(opened.deny()).await),
        (Code::MissingRight, Some(Right::AccessAdmin))
    );
    assert_eq!(f.status(&requested.id), EnrollmentStatus::Pending);

    // The owner can still approve it afterwards.
    let owner = Client::owner(&f.host_key, &f.relay, f.owner, POLICY).unwrap();
    let laptop = pubkey(&key());
    let op = opened.approve(&requested.code, &laptop, Rights::standard(), now() + 3600);
    assert!(matches!(
        owner.call(op).await.unwrap(),
        Outcome::Granted { .. }
    ));
    assert!(matches!(
        f.status(&requested.id),
        EnrollmentStatus::Approved { device, .. } if device == laptop
    ));
    f.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_expired_request_refuses() {
    let f = Fixture::start().await;
    // The host issued this request more than five minutes ago. It keeps an
    // expired request for another minute, so its refusal says why.
    let issued_at = now() - 330;
    let pending = Host::new(&f.access_dir, POLICY)
        .request_enrollment(&f.relay, Rights::standard(), issued_at)
        .unwrap();
    assert_eq!(f.status(&pending.id), EnrollmentStatus::Expired);
    // The administrator opened it while it was current.
    let event = pending
        .events
        .iter()
        .find(|event| event.tag_values("p").any(|p| p == pubkey(&f.admin_key)))
        .unwrap();
    let opened = open_enrollment(event, &f.admin_key, &f.host_key, issued_at + 1, POLICY).unwrap();
    // A client no longer offers it once it has expired.
    assert!(open_enrollment(event, &f.admin_key, &f.host_key, now(), POLICY).is_err());

    let device = pubkey(&key());
    let op = opened.approve(&pending.code, &device, Rights::standard(), now() + 3600);
    assert_eq!(refusal(f.admin.call(op).await).0, Code::Expired);
    let denial = Operation::Deny {
        enrollment: pending.id.clone(),
        request_digest: opened.digest.clone(),
    };
    assert_eq!(refusal(f.admin.call(denial).await).0, Code::Expired);
    assert_eq!(f.status(&pending.id), EnrollmentStatus::Expired);
    let decided = enroll::wait(&f.access_dir, POLICY, &pending.id, POLL)
        .await
        .unwrap();
    assert_eq!(decided, EnrollmentStatus::Expired);
    assert_eq!(f.devices(), vec![pubkey(&f.admin_key)]);
    f.running.shutdown().await;
}
