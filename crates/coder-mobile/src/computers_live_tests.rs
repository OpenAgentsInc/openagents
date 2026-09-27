//! The Computers acceptance run: the mobile application state against a real
//! `coder host serve` on a local authenticated relay.
//!
//! One machine, the synthetic NIP-42 relay fixture, and the `coder host
//! serve` command path (`coder_host::cli::run`) with an in-memory task
//! owner. The app enrolls by a pasted invitation, sees the host online,
//! creates an invitation with narrowed rights, sees activity after a task is
//! created, and sees the host become revoked, with the reason, after another
//! device revokes it. This is not a device or production-relay test.
use super::{App, Config, Packet, Request};
use coder_host::access::client::redeem;
use coder_host::access::host::Host;
use coder_host::access::protocol::{Operation, Outcome, TaskCreate};
use coder_host::access::{Client, Code, RelayPolicy, Right, Rights};
use coder_host::{TaskRef, Tasks};
use nostr::activity_summary::Phase;
use secp256k1::SecretKey;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::connection_tests::relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
const WAIT: Duration = Duration::from_secs(45);

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn now() -> u64 {
    coder_host::unix_time().unwrap()
}

fn step(name: &str) {
    println!("step: {name}");
}

/// Tasks by ID. Creating one records it queued; nothing runs.
#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, String>>);

impl Tasks for Memory {
    fn create(&self, key: &str, _: &str, task: &TaskCreate) -> Result<TaskRef, Code> {
        if task.workspace != "checkout" {
            return Err(Code::Forbidden);
        }
        self.0
            .lock()
            .unwrap()
            .insert(key.to_owned(), task.title.clone());
        Ok(TaskRef {
            task: key.to_owned(),
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

fn walk<'a>(node: &'a Value, out: &mut Vec<&'a Value>) {
    out.push(node);
    if let Some(children) = node["element"]["props"]["children"].as_array() {
        for child in children {
            walk(child, out);
        }
    }
}

fn node<'a>(view: &'a Value, key: &str) -> Option<&'a Value> {
    let mut nodes = Vec::new();
    walk(&view["root"], &mut nodes);
    nodes.into_iter().find(|node| node["key"] == key)
}

fn text(view: &Value, key: &str) -> Option<String> {
    node(view, key).and_then(|node| {
        node["element"]["props"]["value"]
            .as_str()
            .map(str::to_owned)
    })
}

/// Every text and button label in the view, one per line.
fn all_text(view: &Value) -> String {
    let mut nodes = Vec::new();
    walk(&view["root"], &mut nodes);
    nodes
        .iter()
        .filter_map(|node| {
            let props = &node["element"]["props"];
            props["value"].as_str().or(props["label"].as_str())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn refresh(app: &mut App) -> Packet {
    app.call(Request::ComputersRefresh)
}

fn press(app: &mut App, key: &str) -> Packet {
    let view = refresh(app).computers.unwrap();
    assert!(
        node(&view, key).is_some_and(|node| node["element"]["props"]["enabled"] == true),
        "{key} is not an enabled control:\n{}",
        all_text(&view)
    );
    app.call(Request::ComputersActivate {
        instance: view["instance"].as_str().unwrap().into(),
        revision: view["revision"].as_u64().unwrap(),
        node: key.into(),
    })
}

/// Poll the app, the way the native hosts' timers do, until `done` holds.
fn until(app: &mut App, what: &str, done: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + WAIT;
    loop {
        let view = refresh(app).computers.unwrap();
        if done(&view) {
            return view;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}:\n{}",
            all_text(&view)
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn status(view: &Value) -> String {
    text(view, "host-0-status").unwrap_or_default()
}

#[test]
fn the_app_enrolls_watches_invites_sees_activity_and_is_revoked_against_a_real_host() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()
        .unwrap();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());

    step("the operator sets up and serves the host");
    let owner = key();
    let access_dir = temp.path().join("access");
    let store = Host::new(&access_dir, POLICY);
    let host_key = store.init(&coder_host::reach::pubkey(&owner)).unwrap();
    let workspace = temp.path().join("checkout");
    std::fs::create_dir_all(&workspace).unwrap();
    let runtime_record = temp.path().join("host/runtime");
    let args: Vec<String> = [
        "serve",
        "--state",
        &access_dir.to_string_lossy(),
        "--root",
        &temp.path().join("host").to_string_lossy(),
        "--loopback-test",
        "--relay",
        &relay,
        "--workspace",
        &format!("checkout={}", workspace.canonicalize().unwrap().display()),
        "--runtime",
        &runtime_record.to_string_lossy(),
        "--tasks",
        &temp.path().join("tasks").to_string_lossy(),
        "--generation",
        "9",
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect();
    let tasks = Arc::new(Memory::default());
    let owned: Arc<dyn Tasks> = tasks.clone();
    // The host runs as the command does: on its own runtime, until the test
    // process ends.
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(3)
            .enable_all()
            .build()
            .unwrap()
            .block_on(coder_host::cli::run(&args, Box::new(move |_, _| Ok(owned))))
    });
    let deadline = Instant::now() + WAIT;
    while !runtime_record.exists() {
        assert!(Instant::now() < deadline, "the host did not start serving");
        std::thread::sleep(Duration::from_millis(50));
    }
    // `coder host invite --rights all`: an invitation string for the phone.
    let invitation = store
        .invite(&relay, Rights::all(), now(), now() + 3600)
        .unwrap();

    step("enroll the app by a pasted invitation");
    let dir = temp.path().join("app");
    let secret = key();
    let mut app = App::new(Config {
        cache_dir: dir.clone(),
        secret_hex: secret.display_secret().to_string(),
        synthetic: false,
        loopback_test: true,
        push: None,
    })
    .unwrap();
    let device = app.call(Request::Snapshot).public_key;
    let asked = press(&mut app, "invite-paste");
    let input = asked.computers_input.unwrap();
    let added = app.call(Request::ComputersInput {
        token: input.token,
        value: invitation.code.clone(),
    });
    let view = added.computers.unwrap();
    assert!(
        all_text(&view).contains("Added Computer"),
        "{}",
        all_text(&view)
    );
    assert!(added.computers_input.is_none());
    // The grant is saved encrypted under the device key, never in clear.
    let saved = std::fs::read_to_string(dir.join("computers/computers.cache")).unwrap();
    assert!(!saved.contains(&host_key) && !saved.contains("host-grant"));

    // First run is satisfied by one current grant.
    let finished = press(&mut app, "first-run-continue");
    assert!(finished.computers_exit);

    step("see the host online, with fresh data");
    let view = until(&mut app, "the host online", |view| {
        status(view).starts_with("Online") && status(view).ends_with("Up to date.")
    });
    println!("status: {}", status(&view));

    step("background and return: the supervisor probes and stays online");
    app.call(Request::Lifecycle { active: false });
    app.call(Request::Lifecycle { active: true });
    until(&mut app, "the host online after the probe", |view| {
        status(view).starts_with("Online")
    });

    step("the host lists devices with a host-observed last seen");
    press(&mut app, "host-0-access");
    let view = until(&mut app, "the device list", |view| {
        text(view, "device-0-label").is_some()
    });
    let mine = text(&view, "device-0-label").unwrap();
    assert!(mine.starts_with("This device. Active"), "{mine}");
    assert!(
        mine.contains("last seen just now") || mine.contains("min ago"),
        "{mine}"
    );
    assert!(!all_text(&view).contains("last seen: unknown"));

    step("create an invitation with narrowed rights");
    // The default draft is the standard rights this device holds. Leave out
    // terminals and reviews.
    press(&mut app, "share-right-terminal");
    press(&mut app, "share-right-review");
    let created = press(&mut app, "share-create");
    let view = created.computers.unwrap();
    let code = text(&view, "share-code").unwrap();
    assert!(code.starts_with("coder-host:"));
    assert!(
        text(&view, "share-code-detail")
            .unwrap()
            .contains("Grants: View sessions and tasks, Run and steer tasks.")
    );
    // The same screen carries the locally rendered QR code for the phone
    // to draw.
    let qr = created.computers_qr.expect("the invitation's QR code");
    assert_eq!(qr.rows.len(), qr.size);
    assert!(qr.rows.iter().all(|row| row.len() == qr.size));
    let expected = coder_computers::qr::modules(&code).unwrap();
    assert!(
        qr.rows
            .iter()
            .zip(&expected)
            .all(|(row, modules)| row.chars().map(|c| c == '1').eq(modules.iter().copied()))
    );
    // Another device redeems it and holds exactly the narrowed rights.
    let other = key();
    let narrowed = runtime.block_on(redeem(&code, &other, POLICY)).unwrap();
    assert_eq!(
        narrowed.grant.rights,
        Rights::new([Right::Observe, Right::Operate]).unwrap()
    );
    assert_eq!(narrowed.grant.origin.issuer, device);

    step("see activity after that device creates a task");
    let operator = Client::device(narrowed, other, POLICY).unwrap();
    let Outcome::Dispatched { receipt } = runtime
        .block_on(operator.call(Operation::CreateTask {
            task: TaskCreate {
                title: "Fix the flaky parser test".into(),
                prompt: "Find why the parser test fails one run in ten.".into(),
                workspace: "checkout".into(),
            },
        }))
        .unwrap()
    else {
        panic!("task.create answered another outcome")
    };
    assert_eq!(tasks.0.lock().unwrap().len(), 1);
    press(&mut app, "tab-activity");
    let view = until(&mut app, "the task summary", |view| {
        text(view, "activity-0-headline").as_deref() == Some("Task queued")
    });
    assert!(
        text(&view, "activity-0-subject")
            .unwrap()
            .ends_with("Task, queued."),
        "{}",
        all_text(&view)
    );
    // A summary is generic: the title the other device sent stays private.
    assert!(!all_text(&view).contains("flaky parser"));
    assert!(!receipt.reference.is_empty());

    step("another device revokes this one");
    let admin = key();
    let admin_invitation = store
        .invite(&relay, Rights::all(), now(), now() + 3600)
        .unwrap();
    let admin_access = runtime
        .block_on(redeem(&admin_invitation.code, &admin, POLICY))
        .unwrap();
    let admin = Client::device(admin_access, admin, POLICY).unwrap();
    let Outcome::Revoked {
        device: revoked, ..
    } = runtime
        .block_on(admin.call(Operation::Revoke {
            device: device.clone(),
        }))
        .unwrap()
    else {
        panic!("device.revoke answered another outcome")
    };
    assert_eq!(revoked, device);

    step("the host becomes revoked, with the reason");
    press(&mut app, "tab-computers");
    let view = until(&mut app, "the revoked status", |view| {
        status(view).starts_with("Revoked")
    });
    assert_eq!(
        status(&view),
        "Revoked: this computer removed this device's access. Add it again with a new invitation."
    );
    // The revocation persists: a new app instance over the same protected
    // state starts revoked without reaching the host.
    drop(app);
    let mut reopened = App::new(Config {
        cache_dir: dir,
        secret_hex: secret.display_secret().to_string(),
        synthetic: false,
        loopback_test: true,
        push: None,
    })
    .unwrap();
    let view = refresh(&mut reopened).computers.unwrap();
    assert!(status(&view).starts_with("Revoked"), "{}", all_text(&view));
    step("done");
}

/// Serve a host for a simulator or emulator run, and play the other devices'
/// part on a timeline:
///
/// 1. Write `invitation.txt`, `relay-port.txt`, and `host-port.txt` to
///    `CODER_COMPUTERS_FIXTURE_DIR`.
/// 2. After the app's device enrolls, enroll a second device with narrowed
///    rights and create a task, so the app shows activity.
/// 3. `CODER_COMPUTERS_REVOKE_AFTER` seconds (default 60) after the app
///    enrolled, a third device revokes it; the run writes `revoked.txt`.
/// 4. Stop when `stop` appears in the directory, or five minutes later.
///
/// ```sh
/// CODER_COMPUTERS_FIXTURE_DIR=/private/tmp/computers-run \
///   cargo test -p coder-mobile --lib serve_a_host_for_a_device_run -- --ignored --nocapture
/// ```
#[test]
#[ignore = "serves a host for a simulator or emulator run; see its documentation"]
fn serve_a_host_for_a_device_run() {
    let Some(out) = std::env::var_os("CODER_COMPUTERS_FIXTURE_DIR").map(std::path::PathBuf::from)
    else {
        panic!("set CODER_COMPUTERS_FIXTURE_DIR to a private directory");
    };
    let revoke_after = std::env::var("CODER_COMPUTERS_REVOKE_AFTER")
        .ok()
        .and_then(|text| text.parse::<u64>().ok())
        .unwrap_or(60);
    std::fs::create_dir_all(&out).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()
        .unwrap();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    let owner = key();
    let access_dir = temp.path().join("access");
    let store = Host::new(&access_dir, POLICY);
    store.init(&coder_host::reach::pubkey(&owner)).unwrap();
    let workspace = temp.path().join("checkout");
    std::fs::create_dir_all(&workspace).unwrap();
    let runtime_record = temp.path().join("host/runtime");
    let args: Vec<String> = [
        "serve",
        "--state",
        &access_dir.to_string_lossy(),
        "--root",
        &temp.path().join("host").to_string_lossy(),
        "--loopback-test",
        "--relay",
        &relay,
        "--workspace",
        &format!("checkout={}", workspace.canonicalize().unwrap().display()),
        "--runtime",
        &runtime_record.to_string_lossy(),
        "--tasks",
        &temp.path().join("tasks").to_string_lossy(),
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect();
    let owned: Arc<dyn Tasks> = Arc::new(Memory::default());
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(3)
            .enable_all()
            .build()
            .unwrap()
            .block_on(coder_host::cli::run(&args, Box::new(move |_, _| Ok(owned))))
    });
    let deadline = Instant::now() + WAIT;
    while !runtime_record.exists() {
        assert!(Instant::now() < deadline, "the host did not start serving");
        std::thread::sleep(Duration::from_millis(50));
    }
    let record = std::fs::read_to_string(&runtime_record).unwrap();
    let host_port = record
        .lines()
        .find_map(|line| line.strip_prefix("port="))
        .unwrap()
        .to_owned();
    let relay_port = relay.rsplit(':').next().unwrap().to_owned();
    let invitation = store
        .invite(&relay, Rights::all(), now(), now() + 3600)
        .unwrap();
    let write = |name: &str, text: &str| {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(out.join(name))
            .unwrap();
        file.write_all(text.as_bytes()).unwrap();
    };
    write("relay-port.txt", &relay_port);
    write("host-port.txt", &host_port);
    write("invitation.txt", &invitation.code);
    println!("fixture: relay port {relay_port}, host port {host_port}; waiting for the app");

    let mut helpers = Vec::new();
    let app = loop {
        let devices = store.devices(now()).unwrap();
        if let Some(entry) = devices
            .iter()
            .find(|entry| !helpers.contains(&entry.device))
        {
            break entry.device.clone();
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    let enrolled = Instant::now();
    println!("fixture: the app enrolled as {app}");
    std::thread::sleep(Duration::from_secs(5));
    let other = key();
    helpers.push(coder_host::reach::pubkey(&other));
    let narrowed = store
        .invite(
            &relay,
            Rights::new([Right::Observe, Right::Operate]).unwrap(),
            now(),
            now() + 3600,
        )
        .unwrap();
    let access = runtime
        .block_on(redeem(&narrowed.code, &other, POLICY))
        .unwrap();
    let operator = Client::device(access, other, POLICY).unwrap();
    runtime
        .block_on(operator.call(Operation::CreateTask {
            task: TaskCreate {
                title: "Device run task".into(),
                prompt: "Created by the fixture for the activity screen.".into(),
                workspace: "checkout".into(),
            },
        }))
        .unwrap();
    println!("fixture: another device created a task");
    while enrolled.elapsed() < Duration::from_secs(revoke_after) {
        std::thread::sleep(Duration::from_millis(250));
    }
    let admin = key();
    let admin_invitation = store
        .invite(&relay, Rights::all(), now(), now() + 3600)
        .unwrap();
    let admin_access = runtime
        .block_on(redeem(&admin_invitation.code, &admin, POLICY))
        .unwrap();
    let admin = Client::device(admin_access, admin, POLICY).unwrap();
    runtime
        .block_on(admin.call(Operation::Revoke {
            device: app.clone(),
        }))
        .unwrap();
    write("revoked.txt", &app);
    println!("fixture: a third device revoked the app");
    let stop = Instant::now() + Duration::from_secs(300);
    while Instant::now() < stop && !out.join("stop").exists() {
        std::thread::sleep(Duration::from_millis(250));
    }
    println!("fixture: done");
}
