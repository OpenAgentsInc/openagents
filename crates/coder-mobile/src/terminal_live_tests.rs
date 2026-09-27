//! The terminal screen, end to end: the mobile application state against a
//! real `coder host serve` and a real shell on a PTY, over the local
//! authenticated relay fixture.
//!
//! The app enrolls by a pasted invitation, opens the host's screen, taps
//! **Terminal**, reports its grid, types a command, and reads the output from
//! the Rust Native grid it would draw. Further tests cover the accessory
//! row's Ctrl-C, a resize the shell observes, a device without the terminal
//! right, and a host restart that leaves the terminal `lost`. One machine;
//! not a device or production-relay test.
use super::{App, Config, Reply, Request, TerminalPacket};
use coder_host::access::host::Host;
use coder_host::access::{Code, RelayPolicy, Right, Rights};
use coder_host::config::Config as HostConfig;
use coder_host::{NoTasks, Running};
use secp256k1::SecretKey;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
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

fn walk<'a>(node: &'a Value, out: &mut Vec<&'a Value>) {
    out.push(node);
    if let Some(children) = node["element"]["props"]["children"].as_array() {
        for child in children {
            walk(child, out);
        }
    }
}

fn find<'a>(view: &'a Value, key: &str) -> Option<&'a Value> {
    let mut nodes = Vec::new();
    walk(&view["root"], &mut nodes);
    nodes.into_iter().find(|node| node["key"] == key)
}

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

/// The grid's rows as text, the way the native host draws them.
fn grid(view: &Value) -> String {
    let Some(grid) = find(view, "terminal-grid") else {
        return String::new();
    };
    grid["element"]["props"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let mut nodes = Vec::new();
            walk(row, &mut nodes);
            nodes
                .iter()
                .filter_map(|node| node["element"]["props"]["value"].as_str())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn status(view: &Value) -> String {
    find(view, "terminal-status")
        .and_then(|node| node["element"]["props"]["value"].as_str())
        .unwrap_or_default()
        .to_owned()
}

fn computers(app: &mut App) -> Value {
    app.call(Request::ComputersRefresh).computers.unwrap()
}

fn press(app: &mut App, key: &str) -> super::Packet {
    let view = computers(app);
    let node = find(&view, key).unwrap_or_else(|| panic!("no {key}:\n{}", all_text(&view)));
    assert_eq!(
        node["element"]["props"]["enabled"],
        true,
        "{key} is disabled:\n{}",
        all_text(&view)
    );
    app.call(Request::ComputersActivate {
        instance: view["instance"].as_str().unwrap().into(),
        revision: view["revision"].as_u64().unwrap(),
        node: key.into(),
    })
}

fn until_computers(app: &mut App, what: &str, done: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + WAIT;
    loop {
        let view = computers(app);
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

fn terminal(app: &mut App, request: Request) -> TerminalPacket {
    match app.respond(request) {
        Reply::Terminal(packet) => packet,
        Reply::Packet(_) => panic!("a terminal request answered the application packet"),
    }
}

/// Poll the terminal screen the way the native host's timer does, until
/// `done` holds for its view.
fn until_terminal(app: &mut App, what: &str, done: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + WAIT;
    loop {
        let packet = terminal(app, Request::TerminalPoll { known: None });
        assert!(packet.open, "the terminal screen closed");
        let view = packet.view.expect("a poll without a known revision draws");
        if done(&view) {
            return view;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}:\n{}\n{}",
            status(&view),
            grid(&view)
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn tap(app: &mut App, key: &str) -> super::Packet {
    let packet = terminal(app, Request::TerminalPoll { known: None });
    let view = packet.view.unwrap();
    assert!(find(&view, key).is_some(), "no {key}:\n{}", all_text(&view));
    app.call(Request::TerminalActivate {
        instance: view["instance"].as_str().unwrap().into(),
        revision: view["revision"].as_u64().unwrap(),
        node: key.into(),
    })
}

fn workspace(root: &Path) -> std::path::PathBuf {
    let workspace = root.join("checkout");
    std::fs::create_dir_all(&workspace).unwrap();
    workspace.canonicalize().unwrap()
}

/// Enroll a new app with `rights` from `store`'s host and return it once the
/// host shows online.
fn enrolled_app(root: &Path, store: &Host, relay: &str, rights: Rights) -> App {
    let invitation = when_free(|| store.invite(relay, rights.clone(), now(), now() + 3600));
    let mut app = App::new(Config {
        cache_dir: root.join(format!("app-{}", coder_connect::protocol::random_id())),
        secret_hex: key().display_secret().to_string(),
        synthetic: false,
        loopback_test: true,
        push: None,
    })
    .unwrap();
    let asked = press(&mut app, "invite-paste");
    let input = asked.computers_input.unwrap();
    app.call(Request::ComputersInput {
        token: input.token,
        value: invitation.code.clone(),
    });
    press(&mut app, "first-run-continue");
    until_computers(&mut app, "the host online", |view| {
        find(view, "host-0-status")
            .and_then(|node| node["element"]["props"]["value"].as_str())
            .is_some_and(|status| status.starts_with("Online"))
    });
    app
}

/// Open the host's screen, tap **Terminal**, and report a grid.
fn open_terminal(app: &mut App, rows: u16, cols: u16) {
    press(app, "host-0-open");
    let opened = press(app, "host-terminal");
    assert!(
        opened.terminal.is_some(),
        "Terminal opened no terminal screen"
    );
    terminal(app, Request::TerminalResize { rows, cols });
}

fn type_text(app: &mut App, text: &str) {
    terminal(
        app,
        Request::TerminalText {
            text: text.to_owned(),
        },
    );
}

#[test]
fn the_app_opens_a_shell_on_a_real_host_and_runs_commands() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()
        .unwrap();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());

    step("the operator sets up and serves the host with `coder host serve`");
    let owner = key();
    let access_dir = temp.path().join("access");
    let store = Host::new(&access_dir, POLICY);
    store.init(&coder_host::reach::pubkey(&owner)).unwrap();
    let checkout = workspace(temp.path());
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
        &format!("checkout={}", checkout.display()),
        "--runtime",
        &runtime_record.to_string_lossy(),
        "--tasks",
        &temp.path().join("tasks").to_string_lossy(),
        "--generation",
        "5",
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect();
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(3)
            .enable_all()
            .build()
            .unwrap()
            .block_on(coder_host::cli::run(
                &args,
                Box::new(|_, _| Ok(Arc::new(NoTasks))),
            ))
    });
    let deadline = Instant::now() + WAIT;
    while !runtime_record.exists() {
        assert!(Instant::now() < deadline, "the host did not start serving");
        std::thread::sleep(Duration::from_millis(50));
    }

    step("enroll the app with every right and open a terminal");
    let mut app = enrolled_app(temp.path(), &store, &relay, Rights::all());
    open_terminal(&mut app, 24, 60);
    let view = until_terminal(&mut app, "the shell attached", |view| {
        status(view).starts_with("Connected")
    });
    println!("status: {}", status(&view));

    step("type a command and read its output from the grid");
    type_text(&mut app, "echo terminal-$((6*7))\n");
    let view = until_terminal(&mut app, "the command's output", |view| {
        grid(view).lines().any(|line| line == "terminal-42")
    });
    // The shell runs in the admitted workspace. A long path wraps across
    // grid rows.
    type_text(&mut app, "pwd\n");
    let expected = checkout.display().to_string();
    until_terminal(&mut app, "the working directory", |view| {
        grid(view).replace('\n', "").contains(&expected)
    });
    println!("grid:\n{}", grid(&view));

    step("an unchanged screen is not sent again");
    let packet = terminal(&mut app, Request::TerminalPoll { known: None });
    let current = packet.revision;
    std::thread::sleep(Duration::from_millis(300));
    let again = terminal(
        &mut app,
        Request::TerminalPoll {
            known: Some(current),
        },
    );
    if again.revision == current {
        assert!(again.view.is_none());
    }

    step("the shell sees a resize");
    terminal(&mut app, Request::TerminalResize { rows: 20, cols: 50 });
    type_text(&mut app, "stty size\n");
    until_terminal(&mut app, "the new size", |view| {
        grid(view).lines().any(|line| line == "20 50")
    });

    step("Ctrl-C from the accessory row interrupts a running command");
    type_text(&mut app, "sleep 30 && echo slept\n");
    std::thread::sleep(Duration::from_millis(500));
    tap(&mut app, "terminal-key-interrupt");
    type_text(&mut app, "echo after-$((1+1))\n");
    let started = Instant::now();
    let view = until_terminal(&mut app, "the prompt after the interrupt", |view| {
        grid(view).lines().any(|line| line == "after-2")
    });
    assert!(started.elapsed() < Duration::from_secs(20));
    assert!(!grid(&view).lines().any(|line| line == "slept"));

    step("the latched Ctrl and named keys reach the shell");
    // Ctrl, then `u`, erases the typed line: the shell never runs it.
    type_text(&mut app, "echo never");
    tap(&mut app, "terminal-key-ctrl");
    type_text(&mut app, "u");
    type_text(&mut app, "echo ctrl-$((2+3))");
    terminal(
        &mut app,
        Request::TerminalKey {
            key: "enter".into(),
            ctrl: false,
            alt: false,
            shift: false,
        },
    );
    let view = until_terminal(&mut app, "the line after Ctrl-U", |view| {
        grid(view).lines().any(|line| line == "ctrl-5")
    });
    assert!(!grid(&view).lines().any(|line| line == "never"));

    step("a paste is sent as typed text");
    tap(&mut app, "terminal-key-paste");
    let packet = terminal(&mut app, Request::TerminalPoll { known: None });
    assert!(packet.paste, "the screen asks the host for the clipboard");
    terminal(
        &mut app,
        Request::TerminalPaste {
            text: "echo pasted-$((3*3))\n".into(),
        },
    );
    until_terminal(&mut app, "the pasted command's output", |view| {
        grid(view).lines().any(|line| line == "pasted-9")
    });

    step("closing ends the shell and reports it");
    tap(&mut app, "terminal-end");
    let view = until_terminal(&mut app, "the terminal closed", |view| {
        status(view) == "Terminal closed."
    });
    assert!(find(&view, "terminal-reopen").is_some());

    step("Back leaves the screen");
    let left = tap(&mut app, "terminal-close");
    assert!(left.terminal.is_none());
    assert!(!terminal(&mut app, Request::TerminalPoll { known: None }).open);
}

/// A host run on its own runtime that the test can stop.
fn serve(
    access: &Path,
    relay: &str,
    generation: u64,
    checkout: &Path,
) -> (Running, tokio::runtime::Runtime) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let mut config = HostConfig::new(access.to_path_buf(), vec![relay.to_owned()], generation);
    config.policy = POLICY;
    config.workspaces = BTreeMap::from([("checkout".to_owned(), checkout.to_path_buf())]);
    config.presence_every = Duration::from_secs(1);
    let running = runtime
        .block_on(coder_host::start(config, Arc::new(NoTasks)))
        .unwrap();
    (running, runtime)
}

#[test]
fn a_host_restart_leaves_the_terminal_lost() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    let owner = key();
    let access_dir = temp.path().join("access");
    let store = Host::new(&access_dir, POLICY);
    store.init(&coder_host::reach::pubkey(&owner)).unwrap();
    let checkout = workspace(temp.path());
    let (first, first_runtime) = serve(&access_dir, &relay, 21, &checkout);

    let mut app = enrolled_app(temp.path(), &store, &relay, Rights::all());
    open_terminal(&mut app, 20, 60);
    until_terminal(&mut app, "the shell attached", |view| {
        status(view).starts_with("Connected")
    });
    type_text(&mut app, "echo before-restart\n");
    until_terminal(&mut app, "output before the restart", |view| {
        grid(view).lines().any(|line| line == "before-restart")
    });

    step("the host restarts with a new generation");
    first_runtime.block_on(first.shutdown());
    drop(first_runtime);
    let (second, second_runtime) = serve(&access_dir, &relay, 22, &checkout);
    let view = until_terminal(&mut app, "the terminal lost", |view| {
        status(view).starts_with("Lost")
    });
    // The output it had stays on the screen, and it offers a new terminal.
    assert!(grid(&view).lines().any(|line| line == "before-restart"));
    assert!(find(&view, "terminal-reopen").is_some());
    assert!(find(&view, "terminal-end").is_none());

    step("a new terminal opens on the restarted host");
    tap(&mut app, "terminal-reopen");
    until_terminal(&mut app, "the new shell attached", |view| {
        status(view).starts_with("Connected")
    });
    type_text(&mut app, "echo after-restart\n");
    until_terminal(&mut app, "output after the restart", |view| {
        grid(view).lines().any(|line| line == "after-restart")
    });
    app.call(Request::TerminalClose);
    second_runtime.block_on(second.shutdown());
}

#[test]
fn a_device_without_the_terminal_right_is_refused_clearly() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    let owner = key();
    let access_dir = temp.path().join("access");
    let store = Host::new(&access_dir, POLICY);
    store.init(&coder_host::reach::pubkey(&owner)).unwrap();
    let checkout = workspace(temp.path());
    let (running, host_runtime) = serve(&access_dir, &relay, 31, &checkout);

    let rights = Rights::new([Right::Observe, Right::Operate]).unwrap();
    let mut app = enrolled_app(temp.path(), &store, &relay, rights);
    press(&mut app, "host-0-open");
    let view = computers(&mut app);
    let control = find(&view, "host-terminal").unwrap();
    assert_eq!(control["element"]["props"]["enabled"], false);
    let reason = find(&view, "host-terminal-reason").unwrap()["element"]["props"]["value"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(reason.contains("\"Open terminals\" right"), "{reason}");
    // The screen offers nothing to open.
    assert!(app.call(Request::Snapshot).terminal.is_none());
    host_runtime.block_on(running.shutdown());
}

/// Serve a host with a real shell for a simulator or emulator terminal run.
///
/// 1. Write `invitation.txt` (every right), `relay-port.txt`, and
///    `host-port.txt` to `CODER_TERMINAL_FIXTURE_DIR`.
/// 2. When a `restart` file appears there, remove it, stop the host, start it
///    again with the next generation, and write `restarted.txt`. The app's
///    open terminal then reports `lost`.
/// 3. Stop when `stop` appears, or ten minutes later.
///
/// ```sh
/// CODER_TERMINAL_FIXTURE_DIR=/private/tmp/terminal-run \
///   cargo test -p coder-mobile --lib serve_a_host_for_a_terminal_run -- --ignored --nocapture
/// ```
#[test]
#[ignore = "serves a host for a simulator or emulator run; see its documentation"]
fn serve_a_host_for_a_terminal_run() {
    let Some(out) = std::env::var_os("CODER_TERMINAL_FIXTURE_DIR").map(std::path::PathBuf::from)
    else {
        panic!("set CODER_TERMINAL_FIXTURE_DIR to a private directory");
    };
    std::fs::create_dir_all(&out).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    let owner = key();
    let access_dir = temp.path().join("access");
    let store = Host::new(&access_dir, POLICY);
    store.init(&coder_host::reach::pubkey(&owner)).unwrap();
    let checkout = workspace(temp.path());
    let mut generation = 41;
    let (mut running, mut host_runtime) = serve(&access_dir, &relay, generation, &checkout);
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
    let invitation = when_free(|| store.invite(&relay, Rights::all(), now(), now() + 3600));
    write("relay-port.txt", relay.rsplit(':').next().unwrap());
    write("host-port.txt", &running.local_addr().port().to_string());
    write("invitation.txt", &invitation.code);
    println!(
        "fixture: relay {relay}, host {}; waiting for the app",
        running.local_addr()
    );
    let stop = Instant::now() + Duration::from_secs(600);
    while Instant::now() < stop && !out.join("stop").exists() {
        if out.join("restart").exists() {
            let _ = std::fs::remove_file(out.join("restart"));
            host_runtime.block_on(running.shutdown());
            drop(host_runtime);
            generation += 1;
            (running, host_runtime) = serve(&access_dir, &relay, generation, &checkout);
            write("host-port.txt", &running.local_addr().port().to_string());
            write("restarted.txt", &generation.to_string());
            println!("fixture: restarted the host at generation {generation}");
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    host_runtime.block_on(running.shutdown());
    println!("fixture: done");
}
