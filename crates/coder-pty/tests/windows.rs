//! Terminal sessions through a real pseudoconsole on Windows.
//!
//! The same host as `pty.rs`, whose cases are Unix shell scripts, run
//! through `cmd.exe` and this test binary: output and exit codes, typed
//! input, resize, close, and a host that takes a terminal's whole tree
//! with it. A grandchild writes a harmless marker two seconds after it
//! starts; a marker that exists afterwards is a process that outlived its
//! terminal.

#![cfg(windows)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use coder_pty::host::{self, Config, Host, Right, Rights};
use coder_pty::wire::{
    Attach, Body, Cause, Close, EnvVar, Frame, Input, Launch, Mode, Open, Resize, Size, Status,
    TerminalRef, Value,
};
use coder_pty::{Applied, TerminalState};

const WORKSPACE: &str = "5757575757575757575757575757575757575757575757575757575757575757";
const OWNER: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const WAIT: Duration = Duration::from_secs(20);
const RATE: u64 = 1 << 20;
const ROLE: &str = "CODER_PTY_TEST_ROLE";
const MARKER: &str = "CODER_PTY_TEST_MARKER";

struct Grants(Mutex<BTreeMap<String, BTreeSet<&'static str>>>);

impl Rights for Grants {
    fn holds(&self, principal: &str, right: Right) -> bool {
        let name = match right {
            Right::Terminal => "terminal",
            Right::Observe => "observe",
        };
        self.0
            .lock()
            .unwrap()
            .get(principal)
            .is_some_and(|rights| rights.contains(name))
    }
}

struct Fixture {
    host: Host,
    root: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::new().workspace(WORKSPACE, root.path());
    config.env_allow.insert(ROLE.into());
    config.env_allow.insert(MARKER.into());
    let grants = Grants(Mutex::new(BTreeMap::from([(
        OWNER.to_string(),
        BTreeSet::from(["terminal"]),
    )])));
    Fixture {
        host: Host::new(config, Arc::new(grants)),
        root,
    }
}

fn id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{:064x}", NEXT.fetch_add(1, Ordering::SeqCst))
}

fn cmd(script: &str) -> Launch {
    let comspec =
        std::env::var("ComSpec").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into());
    Launch::Command {
        program: comspec,
        args: vec!["/d".into(), "/c".into(), script.into()],
    }
}

fn open_with(host: &Host, launch: Launch, env: Vec<EnvVar>) -> TerminalRef {
    let mut request = Open::new(id(), WORKSPACE, "", launch, Size::new(24, 80));
    request.env = env;
    match host.open(OWNER, &request) {
        Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
        other => panic!("open: {other:?}"),
    }
}

fn open(host: &Host, launch: Launch) -> TerminalRef {
    open_with(host, launch, Vec::new())
}

struct Reader {
    state: TerminalState,
    frames: Receiver<Frame>,
    log: Vec<Body>,
}

impl Reader {
    fn attach(host: &Host, terminal: &TerminalRef) -> Self {
        let (sink, frames) = host::channel(4096);
        let attach = Attach::new(id(), terminal.clone(), Mode::Interact, 0, RATE);
        match host.attach(OWNER, &attach, Box::new(sink)) {
            Ok((Status::Accepted, Value::Attached { .. })) => {}
            other => panic!("attach: {other:?}"),
        }
        Reader {
            state: TerminalState::new(terminal.clone(), 1000, 200),
            frames,
            log: Vec::new(),
        }
    }

    fn until(&mut self, done: impl Fn(&TerminalState) -> bool) -> bool {
        let deadline = Instant::now() + WAIT;
        while !done(&self.state) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            match self.frames.recv_timeout(left) {
                Ok(frame) => {
                    let applied = self.state.apply(&frame);
                    assert!(
                        !matches!(applied, Applied::Behind { .. } | Applied::Refused(_)),
                        "{applied:?}"
                    );
                    self.log.push(frame.body);
                }
                Err(_) => return false,
            }
        }
        true
    }

    fn text(&self) -> String {
        self.state.screen().text()
    }

    fn sees(&mut self, needle: &str) -> bool {
        self.until(|state| state.screen().text().contains(needle))
    }

    fn exits(&mut self) -> bool {
        self.until(|state| state.exit().is_some())
    }
}

fn type_in(host: &Host, terminal: &TerminalRef, text: &str) {
    host.input(OWNER, &Input::new(id(), terminal.clone(), text.as_bytes()))
        .unwrap();
}

/// Plays the role the variable names, when this binary is a terminal's
/// program. Does nothing in an ordinary test run.
#[test]
fn play_a_role_when_asked() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    let marker = std::path::PathBuf::from(std::env::var_os(MARKER).unwrap());
    match role.as_str() {
        "parent" => {
            let mut grandchild = std::process::Command::new(std::env::current_exe().unwrap());
            grandchild
                .args(["--exact", "play_a_role_when_asked", "--nocapture"])
                .env(ROLE, "grandchild")
                .env(MARKER, &marker);
            // Left running on purpose: outliving the terminal is what the
            // grandchild tries to do.
            #[allow(clippy::zombie_processes)]
            let _grandchild = grandchild.spawn().unwrap();
            println!("ready");
            std::thread::sleep(Duration::from_secs(60));
        }
        "grandchild" => {
            std::thread::sleep(Duration::from_secs(2));
            std::fs::write(&marker, "harmless").unwrap();
        }
        other => panic!("no role {other}"),
    }
}

#[test]
fn output_and_the_exit_code_come_back_in_order() {
    let fixture = fixture();
    let terminal = open(&fixture.host, cmd("echo hello-conpty & exit 7"));
    let mut reader = Reader::attach(&fixture.host, &terminal);
    assert!(reader.exits(), "{}", reader.text());
    assert!(reader.text().contains("hello-conpty"), "{}", reader.text());
    let exit = reader.state.exit().unwrap();
    assert_eq!(
        (exit.cause, exit.code, exit.signal),
        (Cause::Exited, Some(7), None)
    );
    assert!(matches!(reader.log.last(), Some(Body::Exit { .. })));
    let seqs: Vec<u64> = reader.log.iter().filter_map(Body::seq).collect();
    assert_eq!(seqs, (1..=seqs.len() as u64).collect::<Vec<_>>());
}

#[test]
fn a_shell_runs_what_is_typed_in_its_workspace() {
    let fixture = fixture();
    std::fs::write(fixture.root.path().join("found-me.txt"), "").unwrap();
    let terminal = open(&fixture.host, Launch::Shell);
    let mut reader = Reader::attach(&fixture.host, &terminal);
    // The typed line holds `6*7`; only the shell's answer holds 42.
    type_in(&fixture.host, &terminal, "set /a 6*7\r\n");
    assert!(reader.sees("42"), "{}", reader.text());
    type_in(&fixture.host, &terminal, "dir /b\r\n");
    assert!(reader.sees("found-me.txt"), "{}", reader.text());
    type_in(&fixture.host, &terminal, "exit 3\r\n");
    assert!(reader.exits(), "{}", reader.text());
    assert_eq!(reader.state.exit().unwrap().code, Some(3));
}

#[test]
fn a_resize_is_accepted_and_reported_to_a_later_attachment() {
    let fixture = fixture();
    let terminal = open(&fixture.host, Launch::Shell);
    let _reader = Reader::attach(&fixture.host, &terminal);
    let resize = Resize::new(id(), terminal.clone(), Size::new(40, 100));
    // Wine does not implement `ResizePseudoConsole`.
    if std::env::var_os("OPENAGENTS_TEST_UNDER_WINE").is_some() {
        return;
    }
    assert_eq!(
        fixture.host.resize(OWNER, &resize),
        Ok((Status::Accepted, Value::Done))
    );
    let (sink, _frames) = host::channel(16);
    let attach = Attach::new(id(), terminal, Mode::Observe, 0, RATE);
    match fixture.host.attach(OWNER, &attach, Box::new(sink)) {
        Ok((_, Value::Attached { size, .. })) => assert_eq!(size, Size::new(40, 100)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn close_ends_the_terminal_and_reports_why() {
    let fixture = fixture();
    let terminal = open(&fixture.host, Launch::Shell);
    let pid = fixture.host.process_group(&terminal).unwrap();
    let mut reader = Reader::attach(&fixture.host, &terminal);
    let close = Close::new(id(), terminal);
    assert_eq!(
        fixture.host.close(OWNER, &close),
        Ok((Status::Accepted, Value::Done))
    );
    assert!(reader.exits(), "{}", reader.text());
    assert_eq!(reader.state.exit().unwrap().cause, Cause::Closed);
    assert!(!supervise::process_running(u32::try_from(pid).unwrap()));
}

#[test]
fn host_shutdown_takes_the_terminals_whole_tree() {
    let fixture = fixture();
    let marker = fixture.root.path().join("after-shutdown");
    let program = std::env::current_exe().unwrap().display().to_string();
    let launch = Launch::Command {
        program,
        args: ["--exact", "play_a_role_when_asked", "--nocapture"]
            .map(String::from)
            .to_vec(),
    };
    let env = vec![
        EnvVar {
            name: ROLE.into(),
            value: "parent".into(),
        },
        EnvVar {
            name: MARKER.into(),
            value: marker.display().to_string(),
        },
    ];
    let terminal = open_with(&fixture.host, launch, env);
    let pid = fixture.host.process_group(&terminal).unwrap();
    let mut reader = Reader::attach(&fixture.host, &terminal);
    assert!(reader.sees("ready"), "{}", reader.text());
    fixture.host.shutdown();
    assert!(reader.exits());
    assert_eq!(reader.state.exit().unwrap().cause, Cause::HostShutdown);
    assert!(!supervise::process_running(u32::try_from(pid).unwrap()));
    std::thread::sleep(Duration::from_secs(3));
    assert!(
        !marker_exists(&marker),
        "a descendant outlived the terminal"
    );
}

fn marker_exists(marker: &Path) -> bool {
    marker.exists()
}
