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

/// Run a local access-store operation, waiting while the running host holds
/// the store's lock, as the `coder host` commands do.
fn when_free<T>(mut operation: impl FnMut() -> coder_host::access::Result<T>) -> T {
    let started = Instant::now();
    loop {
        match operation() {
            Err(error) if error.code == Code::Conflict && started.elapsed() < WAIT => {
                std::thread::sleep(Duration::from_millis(20));
            }
            other => return other.unwrap(),
        }
    }
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
    let admin_invitation = when_free(|| store.invite(&relay, Rights::all(), now(), now() + 3600));
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
///    With `never`, the app stays enrolled for ordering work.
/// 4. The host shares the workspaces `checkout` and `scratch`. Each task
///    the app orders, steers, or stops is written to `host-tasks.txt` as
///    the host's task owner received it.
/// 5. Stop when `stop` appears in the directory, or five minutes (ten with
///    `never`) later.
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
    // `never` keeps the app enrolled, for runs that order and follow work.
    let revoke_after = match std::env::var("CODER_COMPUTERS_REVOKE_AFTER").ok() {
        Some(text) if text == "never" => None,
        text => Some(text.and_then(|text| text.parse::<u64>().ok()).unwrap_or(60)),
    };
    std::fs::create_dir_all(&out).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()
        .unwrap();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    let ledger = Arc::new(Ledger::default());
    let (store, _, host_port) = serve(temp.path(), &relay, ledger.clone());
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
        let devices = when_free(|| store.devices(now()));
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
    // A run that orders work keeps the phone's own task the only one.
    if revoke_after.is_some() {
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
    }
    // Report what the phone orders, steers, and stops, as the host's task
    // owner received it. Titles are the phone's; prompts stay private.
    let mut reported = String::new();
    let mut report = |ledger: &Ledger| {
        let lines: Vec<String> = ledger
            .tasks()
            .iter()
            .filter(|held| held.title != "Device run task")
            .map(|held| {
                format!(
                    "{} | {} | revision {} | {:?} | {} steer(s)",
                    held.title,
                    held.workspace,
                    held.revision,
                    held.phase,
                    held.steers.len()
                )
            })
            .collect();
        let text = lines.join("\n");
        if text != reported {
            println!("fixture: host tasks:\n{text}");
            write("host-tasks.txt", &text);
            reported = text;
        }
    };
    let Some(revoke_after) = revoke_after else {
        let stop = Instant::now() + Duration::from_secs(600);
        while Instant::now() < stop && !out.join("stop").exists() {
            report(&ledger);
            std::thread::sleep(Duration::from_millis(250));
        }
        println!("fixture: done");
        return;
    };
    while enrolled.elapsed() < Duration::from_secs(revoke_after) {
        report(&ledger);
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

/// One task as a host's owner holds it.
#[derive(Clone, Debug)]
struct Held {
    workspace: String,
    title: String,
    revision: u64,
    phase: Phase,
    steers: Vec<String>,
}

/// Tasks with revisions: a steer or a cancel at another revision is stale.
/// Nothing runs; creating records a queued task.
#[derive(Default)]
struct Ledger(Mutex<BTreeMap<String, Held>>);

impl Ledger {
    fn tasks(&self) -> Vec<Held> {
        self.0.lock().unwrap().values().cloned().collect()
    }

    fn change(
        &self,
        task: &str,
        revision: u64,
        change: impl FnOnce(&mut Held),
    ) -> Result<TaskRef, Code> {
        let mut tasks = self.0.lock().unwrap();
        let held = tasks.get_mut(task).ok_or(Code::Forbidden)?;
        if held.revision != revision {
            return Err(Code::Stale);
        }
        change(held);
        held.revision += 1;
        Ok(TaskRef {
            task: task.to_owned(),
            revision: held.revision,
            phase: held.phase,
        })
    }
}

impl Tasks for Ledger {
    fn create(&self, key: &str, _: &str, task: &TaskCreate) -> Result<TaskRef, Code> {
        if !["checkout", "scratch"].contains(&task.workspace.as_str()) {
            return Err(Code::Forbidden);
        }
        let mut tasks = self.0.lock().unwrap();
        let held = tasks.entry(key.to_owned()).or_insert_with(|| Held {
            workspace: task.workspace.clone(),
            title: task.title.clone(),
            revision: 1,
            phase: Phase::Queued,
            steers: Vec::new(),
        });
        Ok(TaskRef {
            task: key.to_owned(),
            revision: held.revision,
            phase: held.phase,
        })
    }
    fn steer(
        &self,
        _: &str,
        _: &str,
        task: &str,
        revision: u64,
        prompt: &str,
    ) -> Result<TaskRef, Code> {
        self.change(task, revision, |held| held.steers.push(prompt.to_owned()))
    }
    fn cancel(
        &self,
        _: &str,
        _: &str,
        task: &str,
        revision: u64,
        _: &str,
    ) -> Result<TaskRef, Code> {
        self.change(task, revision, |held| held.phase = Phase::Cancelled)
    }
}

/// Serve a host through the `coder host serve` command path with two
/// workspaces, `checkout` and `scratch`, and wait until it serves.
fn serve(temp: &std::path::Path, relay: &str, tasks: Arc<dyn Tasks>) -> (Host, String, String) {
    let owner = key();
    let access_dir = temp.join("access");
    let store = Host::new(&access_dir, POLICY);
    let host_key = store.init(&coder_host::reach::pubkey(&owner)).unwrap();
    let mut workspaces = Vec::new();
    for name in ["checkout", "scratch"] {
        let path = temp.join(name);
        std::fs::create_dir_all(&path).unwrap();
        workspaces.push(format!("{name}={}", path.canonicalize().unwrap().display()));
    }
    let runtime_record = temp.join("host/runtime");
    let mut args: Vec<String> = [
        "serve",
        "--state",
        &access_dir.to_string_lossy(),
        "--root",
        &temp.join("host").to_string_lossy(),
        "--loopback-test",
        "--relay",
        relay,
        "--runtime",
        &runtime_record.to_string_lossy(),
        "--tasks",
        &temp.join("tasks").to_string_lossy(),
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect();
    for workspace in workspaces {
        args.push("--workspace".into());
        args.push(workspace);
    }
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(3)
            .enable_all()
            .build()
            .unwrap()
            .block_on(coder_host::cli::run(&args, Box::new(move |_, _| Ok(tasks))))
    });
    let deadline = Instant::now() + WAIT;
    while !runtime_record.exists() {
        assert!(Instant::now() < deadline, "the host did not start serving");
        std::thread::sleep(Duration::from_millis(50));
    }
    let record = std::fs::read_to_string(&runtime_record).unwrap();
    let port = record
        .lines()
        .find_map(|line| line.strip_prefix("port="))
        .unwrap()
        .to_owned();
    (store, host_key, port)
}

/// Answer the current input request with `value`.
fn answer(app: &mut App, purpose: &str, value: &str) -> Packet {
    let packet = app.call(Request::Snapshot);
    let input = packet.computers_input.expect("an input request");
    assert_eq!(
        serde_json::to_value(input.purpose).unwrap(),
        Value::from(purpose)
    );
    app.call(Request::ComputersInput {
        token: input.token,
        value: value.into(),
    })
}

#[test]
fn the_app_orders_steers_and_stops_a_task_on_a_real_host() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()
        .unwrap();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    step("serve a host with two workspaces");
    let ledger = Arc::new(Ledger::default());
    let (store, _, _) = serve(temp.path(), &relay, ledger.clone());
    let invitation = store
        .invite(&relay, Rights::all(), now(), now() + 3600)
        .unwrap();

    step("enroll the app and see the host online");
    let mut app = App::new(Config {
        cache_dir: temp.path().join("app"),
        secret_hex: key().display_secret().to_string(),
        synthetic: false,
        loopback_test: true,
        push: None,
    })
    .unwrap();
    press(&mut app, "invite-paste");
    answer(&mut app, "invitation", &invitation.code);
    press(&mut app, "first-run-continue");
    until(&mut app, "the host online", |view| {
        status(view).starts_with("Online") && status(view).ends_with("Up to date.")
    });

    step("open the host: its route, rights, and workspaces");
    press(&mut app, "host-0-open");
    let view = until(&mut app, "the host's workspaces", |view| {
        text(view, "host-workspaces").as_deref() == Some("Workspaces: checkout, scratch.")
    });
    assert!(text(&view, "host-status").unwrap().starts_with("Online"));
    assert!(
        text(&view, "host-rights")
            .unwrap()
            .contains("Run and steer tasks")
    );

    step("order work: a listed workspace and a prompt");
    press(&mut app, "host-order");
    press(&mut app, "order-workspace-1");
    press(&mut app, "order-prompt");
    answer(
        &mut app,
        "task_prompt",
        "Fix the flaky parser test\nFind why it fails one run in ten.",
    );
    let sent = press(&mut app, "order-submit");
    let view = sent.computers.unwrap();
    assert!(
        text(&view, "notice")
            .unwrap()
            .contains("Sent \"Fix the flaky parser test\""),
        "{}",
        all_text(&view)
    );
    let held = ledger.tasks();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].workspace, "scratch");
    assert_eq!(held[0].title, "Fix the flaky parser test");

    step("follow it in Activity");
    until(&mut app, "the queued task", |view| {
        text(view, "activity-0-time").is_some_and(|time| time.contains("Revision 1."))
            && text(view, "activity-0-subject").is_some_and(|s| s.ends_with("Task, queued."))
    });

    step("steer it");
    press(&mut app, "activity-0-steer");
    answer(&mut app, "steer_prompt", "Only the parser module.");
    assert_eq!(ledger.tasks()[0].steers, ["Only the parser module."]);
    until(&mut app, "the steered revision", |view| {
        text(view, "activity-0-time").is_some_and(|time| time.contains("Revision 2."))
    });

    step("stop it");
    press(&mut app, "activity-0-cancel");
    press(&mut app, "activity-0-cancel-yes");
    assert_eq!(ledger.tasks()[0].phase, Phase::Cancelled);
    let view = until(&mut app, "the cancelled task", |view| {
        text(view, "activity-0-subject").is_some_and(|s| s.ends_with("Task, cancelled."))
    });
    assert!(node(&view, "activity-0-steer").is_none());

    step("open the terminal entry point and leave it");
    press(&mut app, "tab-computers");
    press(&mut app, "host-0-open");
    let opened = press(&mut app, "host-terminal");
    let terminal = opened.terminal.expect("the terminal screen");
    assert!(all_text(&terminal).contains("Terminal on Computer"));
    let closed = app.call(Request::TerminalActivate {
        instance: terminal["instance"].as_str().unwrap().into(),
        revision: terminal["revision"].as_u64().unwrap(),
        node: "terminal-close".into(),
    });
    assert!(closed.terminal.is_none());
    step("done");
}
