//! Terminal sessions through real PTYs on this machine.
//!
//! Every test opens actual processes on actual PTYs and reads what they
//! print. The only process groups signaled are the ones these hosts
//! created.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use coder_pty::host::{self, Config, Host, Right, Rights};
use coder_pty::wire::{
    Attach, Body, Cause, Close, Detach, Detached, EnvVar, Frame, Input, Launch, Mode, Open, Reason,
    Resize, Signal, SignalKind, Size, Status, TerminalRef, Value,
};
use coder_pty::{Applied, Line, TerminalState};

const WORKSPACE: &str = "5757575757575757575757575757575757575757575757575757575757575757";
const OWNER: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const OBSERVER: &str = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";
const STRANGER: &str = "0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c";
const WAIT: Duration = Duration::from_secs(10);
const RATE: u64 = 1 << 20;

/// Grants held in memory, changeable during a test.
#[derive(Default)]
struct Grants(Mutex<BTreeMap<String, BTreeSet<&'static str>>>);

impl Grants {
    fn standard() -> Arc<Self> {
        let grants = Grants::default();
        grants.grant(OWNER, "terminal");
        grants.grant(OBSERVER, "observe");
        Arc::new(grants)
    }

    fn grant(&self, principal: &str, right: &'static str) {
        self.0
            .lock()
            .unwrap()
            .entry(principal.into())
            .or_default()
            .insert(right);
    }

    fn revoke(&self, principal: &str) {
        self.0.lock().unwrap().remove(principal);
    }
}

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
    grants: Arc<Grants>,
    root: tempfile::TempDir,
}

fn fixture_with(adjust: impl FnOnce(&mut Config)) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::new().workspace(WORKSPACE, root.path());
    adjust(&mut config);
    let grants = Grants::standard();
    let host = Host::new(config, grants.clone());
    Fixture { host, grants, root }
}

fn fixture() -> Fixture {
    fixture_with(|_| {})
}

fn id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{:064x}", NEXT.fetch_add(1, Ordering::SeqCst))
}

fn command(program: &str, args: &[&str]) -> Launch {
    Launch::Command {
        program: program.into(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
    }
}

fn sh(script: &str) -> Launch {
    command("/bin/sh", &["-c", script])
}

fn open(host: &Host, launch: Launch) -> TerminalRef {
    match host.open(
        OWNER,
        &Open::new(id(), WORKSPACE, "", launch, Size::new(24, 80)),
    ) {
        Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
        other => panic!("open: {other:?}"),
    }
}

struct Reader {
    state: TerminalState,
    frames: Receiver<Frame>,
    attachment: String,
    log: Vec<Body>,
}

impl Reader {
    fn attach(
        host: &Host,
        principal: &str,
        terminal: &TerminalRef,
        mode: Mode,
        after: u64,
    ) -> Self {
        let (sink, frames) = host::channel(1024);
        let attach = Attach::new(id(), terminal.clone(), mode, after, RATE);
        let attachment = match host.attach(principal, &attach, Box::new(sink)) {
            Ok((Status::Accepted, Value::Attached { attachment, .. })) => attachment,
            other => panic!("attach: {other:?}"),
        };
        Reader {
            state: TerminalState::new(terminal.clone(), 1000, 200).starting_after(after),
            frames,
            attachment,
            log: Vec::new(),
        }
    }

    /// Applies frames until `done` holds or the wait passes.
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

fn type_in(host: &Host, principal: &str, terminal: &TerminalRef, text: &str) -> host::Outcome {
    host.input(
        principal,
        &Input::new(id(), terminal.clone(), text.as_bytes()),
    )
}

#[test]
fn echo_round_trips_through_a_real_pty() {
    let fixture = fixture();
    let terminal = open(&fixture.host, command("/bin/cat", &[]));
    let mut reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Interact, 0);
    let typed = type_in(&fixture.host, OWNER, &terminal, "hello pty\n").unwrap();
    assert_eq!(typed, (Status::Accepted, Value::Written { bytes: 10 }));
    // The line discipline echoes the line, then cat prints it back.
    assert!(
        reader.until(|state| state.screen().text().matches("hello pty").count() >= 2),
        "{}",
        reader.text()
    );
    // Sequence numbers start at one and increase by one.
    let seqs: Vec<u64> = reader.log.iter().filter_map(Body::seq).collect();
    assert_eq!(seqs, (1..=seqs.len() as u64).collect::<Vec<_>>());
}

#[test]
fn the_process_sees_a_terminal_and_its_workspace() {
    let fixture = fixture();
    std::fs::create_dir(fixture.root.path().join("sub")).unwrap();
    let open_request = Open::new(
        id(),
        WORKSPACE,
        "sub",
        sh("test -t 0 && test -t 1 && echo tty-yes; pwd"),
        Size::new(24, 80),
    );
    let Ok((_, Value::Opened { terminal, .. })) = fixture.host.open(OWNER, &open_request) else {
        panic!("open");
    };
    let mut reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Observe, 0);
    assert!(reader.exits(), "{}", reader.text());
    let text = reader.text();
    assert!(text.contains("tty-yes"), "{text}");
    let sub = fixture.root.path().join("sub").canonicalize().unwrap();
    assert!(text.contains(sub.to_str().unwrap()), "{text}");
}

#[test]
fn resize_reaches_the_process() {
    let fixture = fixture();
    let terminal = open(&fixture.host, sh("stty size; read line; stty size"));
    let mut reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Interact, 0);
    assert!(reader.sees("24 80"), "{}", reader.text());
    let resize = Resize::new(id(), terminal.clone(), Size::new(40, 100));
    assert_eq!(
        fixture.host.resize(OWNER, &resize),
        Ok((Status::Accepted, Value::Done))
    );
    type_in(&fixture.host, OWNER, &terminal, "\n").unwrap();
    assert!(reader.sees("40 100"), "{}", reader.text());
    // A later attachment learns the current size.
    let (sink, _frames) = host::channel(16);
    let attach = Attach::new(id(), terminal, Mode::Observe, 0, RATE);
    match fixture.host.attach(OWNER, &attach, Box::new(sink)) {
        Ok((_, Value::Attached { size, .. })) => assert_eq!(size, Size::new(40, 100)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn two_attached_readers_see_the_same_frames() {
    let fixture = fixture();
    let terminal = open(&fixture.host, command("/bin/cat", &[]));
    let mut first = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Interact, 0);
    let mut second = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Observe, 0);
    type_in(&fixture.host, OWNER, &terminal, "shared line\n").unwrap();
    let twice = |state: &TerminalState| state.screen().text().matches("shared line").count() >= 2;
    assert!(first.until(twice), "{}", first.text());
    assert!(second.until(twice), "{}", second.text());
    assert_eq!(first.text(), second.text());
    assert_eq!(first.state.resume_after(), second.state.resume_after());
}

#[test]
fn a_detached_client_reattaches_and_replays_what_it_missed() {
    let fixture = fixture();
    let terminal = open(&fixture.host, command("/bin/cat", &[]));
    let mut phone = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Interact, 0);
    type_in(&fixture.host, OWNER, &terminal, "before detach\n").unwrap();
    assert!(phone.until(|state| state.screen().text().matches("before detach").count() >= 2));
    let seen = phone.state.resume_after();
    let detach = Detach::new(id(), terminal.clone(), phone.attachment.clone());
    assert_eq!(
        fixture.host.detach(OWNER, &detach),
        Ok((Status::Accepted, Value::Done))
    );
    assert!(phone.until(|state| state.detached() == Some(Detached::Requested)));

    // Nobody is attached; the terminal keeps running and keeps its output.
    type_in(&fixture.host, OWNER, &terminal, "while away\n").unwrap();
    let deadline = Instant::now() + WAIT;
    while fixture.host.head(&terminal).unwrap().0 <= seen && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }

    // Another device picks up exactly where the first left off.
    let mut laptop = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Interact, seen);
    assert!(laptop.until(|state| state.screen().text().matches("while away").count() >= 2));
    assert!(
        !laptop.text().contains("before detach"),
        "{}",
        laptop.text()
    );
    assert_eq!(laptop.log.first().and_then(Body::seq), Some(seen + 1));
    assert!(
        !laptop
            .log
            .iter()
            .any(|body| matches!(body, Body::Gap { .. }))
    );
}

#[test]
fn replay_after_the_ring_wrapped_reports_a_gap() {
    let fixture = fixture_with(|config| {
        config.frame_max = 1024;
        config.ring_bytes = 4096;
    });
    let terminal = open(
        &fixture.host,
        sh("i=0; while [ $i -lt 2000 ]; do echo line$i; i=$((i+1)); done"),
    );
    let deadline = Instant::now() + WAIT;
    while fixture.host.head(&terminal).unwrap().1 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut late = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Observe, 0);
    assert!(late.exits(), "{}", late.text());
    let Some(Body::Gap { from, to, bytes }) = late.log.first().cloned() else {
        panic!("the first frame is not a gap: {:?}", late.log.first());
    };
    assert_eq!(from, 1);
    assert!(to >= 1);
    let missed = bytes.expect("a replay from the start knows how much it missed");
    assert!(missed > 10_000, "{missed}");
    assert_eq!(late.state.missed_bytes(), (missed, false));
    assert_eq!(late.state.missing().collect::<Vec<_>>(), vec![(1, to)]);
    // The screen shows the gap, then the retained tail through the last line.
    let lines: Vec<Line> = late.state.screen().lines().collect();
    assert_eq!(
        lines[0],
        Line::Gap {
            bytes: Some(missed)
        }
    );
    assert!(late.text().contains("line1999"));
    assert!(!late.text().contains("line0\n"));
    let retained: usize = late
        .log
        .iter()
        .map(|body| match body {
            Body::Output { data, .. } => data.len(),
            _ => 0,
        })
        .sum();
    assert!(retained <= 4096, "{retained}");
}

#[test]
fn input_without_the_terminal_right_is_refused() {
    let fixture = fixture_with(|config| config.observers_read = true);
    let terminal = open(&fixture.host, command("/bin/cat", &[]));
    let mut observer = Reader::attach(&fixture.host, OBSERVER, &terminal, Mode::Observe, 0);

    let refused = type_in(&fixture.host, OBSERVER, &terminal, "observer typed\n").unwrap_err();
    assert_eq!(refused.reason, Reason::NotAdmitted);
    let resize = Resize::new(id(), terminal.clone(), Size::new(10, 10));
    assert_eq!(
        fixture.host.resize(OBSERVER, &resize).unwrap_err().reason,
        Reason::NotAdmitted
    );
    let signal = Signal::new(id(), terminal.clone(), SignalKind::Kill);
    assert_eq!(
        fixture.host.signal(OBSERVER, &signal).unwrap_err().reason,
        Reason::NotAdmitted
    );
    let close = Close::new(id(), terminal.clone());
    assert_eq!(
        fixture.host.close(OBSERVER, &close).unwrap_err().reason,
        Reason::NotAdmitted
    );
    let interact = Attach::new(id(), terminal.clone(), Mode::Interact, 0, RATE);
    let (sink, _frames) = host::channel(4);
    assert_eq!(
        fixture
            .host
            .attach(OBSERVER, &interact, Box::new(sink))
            .unwrap_err()
            .reason,
        Reason::NotAdmitted
    );
    let open_request = Open::new(id(), WORKSPACE, "", Launch::Shell, Size::new(24, 80));
    assert_eq!(
        fixture
            .host
            .open(STRANGER, &open_request)
            .unwrap_err()
            .reason,
        Reason::NotAdmitted
    );

    // The owner's input arrives; the refused input never reached the PTY.
    type_in(&fixture.host, OWNER, &terminal, "owner typed\n").unwrap();
    assert!(observer.sees("owner typed"), "{}", observer.text());
    assert!(
        !observer.text().contains("observer typed"),
        "{}",
        observer.text()
    );
}

#[test]
fn observers_read_only_when_the_host_allows_it() {
    let fixture = fixture();
    let terminal = open(&fixture.host, command("/bin/cat", &[]));
    let (sink, _frames) = host::channel(4);
    let attach = Attach::new(id(), terminal, Mode::Observe, 0, RATE);
    assert_eq!(
        fixture
            .host
            .attach(OBSERVER, &attach, Box::new(sink))
            .unwrap_err()
            .reason,
        Reason::NotAdmitted
    );
}

#[test]
fn a_revoked_attachment_is_ended_on_the_next_tick() {
    let fixture = fixture_with(|config| config.observers_read = true);
    let terminal = open(&fixture.host, command("/bin/cat", &[]));
    let mut observer = Reader::attach(&fixture.host, OBSERVER, &terminal, Mode::Observe, 0);
    fixture.grants.revoke(OBSERVER);
    fixture.host.tick(Instant::now());
    assert!(observer.until(|state| state.detached() == Some(Detached::Revoked)));
}

#[test]
fn process_exit_is_reported_with_its_status() {
    let fixture = fixture();
    let terminal = open(&fixture.host, sh("echo bye; exit 7"));
    let mut reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Observe, 0);
    assert!(reader.exits());
    let exit = reader.state.exit().unwrap();
    assert_eq!(
        (exit.cause, exit.code, exit.signal),
        (Cause::Exited, Some(7), None)
    );
    assert!(reader.text().contains("bye"));
    // The exit is the last sequenced frame, after the output.
    assert!(matches!(reader.log.last(), Some(Body::Exit { .. })));
    // Input to an ended terminal is refused as closed.
    assert_eq!(
        type_in(&fixture.host, OWNER, &terminal, "late\n")
            .unwrap_err()
            .reason,
        Reason::Closed
    );

    let terminal = open(&fixture.host, sh("kill -TERM $$"));
    let mut reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Observe, 0);
    assert!(reader.exits());
    let exit = reader.state.exit().unwrap();
    assert_eq!(
        (exit.cause, exit.code, exit.signal),
        (Cause::Exited, None, Some(libc_sigterm()))
    );
}

#[test]
fn a_signal_reaches_the_foreground_process() {
    let fixture = fixture();
    let terminal = open(
        &fixture.host,
        sh("trap 'echo caught; exit 3' INT; echo armed; while :; do sleep 1; done"),
    );
    let mut reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Interact, 0);
    assert!(reader.sees("armed"));
    let signal = Signal::new(id(), terminal, SignalKind::Interrupt);
    assert_eq!(
        fixture.host.signal(OWNER, &signal),
        Ok((Status::Accepted, Value::Done))
    );
    assert!(reader.exits(), "{}", reader.text());
    assert!(reader.text().contains("caught"));
    assert_eq!(reader.state.exit().unwrap().code, Some(3));
}

#[test]
fn close_ends_the_terminal_and_reports_why() {
    let fixture = fixture();
    let terminal = open(&fixture.host, command("/bin/cat", &[]));
    let group = fixture.host.process_group(&terminal).unwrap();
    let mut reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Interact, 0);
    let close = Close::new(id(), terminal.clone());
    assert_eq!(
        fixture.host.close(OWNER, &close),
        Ok((Status::Accepted, Value::Done))
    );
    // An exact retry is answered, not applied again.
    assert_eq!(
        fixture.host.close(OWNER, &close),
        Ok((Status::Duplicate, Value::Done))
    );
    assert!(reader.exits());
    assert_eq!(reader.state.exit().unwrap().cause, Cause::Closed);
    assert!(!supervise::running(group));
}

#[test]
fn host_shutdown_kills_the_process_group() {
    let fixture = fixture();
    // The shell ignores the hang-up and termination; its background child
    // shares its group. Only SIGKILL to the group ends both.
    let terminal = open(
        &fixture.host,
        sh("trap '' HUP TERM; sleep 60 & echo ready; wait; sleep 60"),
    );
    let group = fixture.host.process_group(&terminal).unwrap();
    let mut reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Observe, 0);
    assert!(reader.sees("ready"));
    assert!(supervise::running(group));
    fixture.host.shutdown();
    assert!(
        !supervise::running(group),
        "the terminal's process group outlived the host"
    );
    assert!(reader.exits());
    let exit = reader.state.exit().unwrap();
    assert_eq!(exit.cause, Cause::HostShutdown);
    assert_eq!(exit.signal, Some(9));
    let open_request = Open::new(id(), WORKSPACE, "", Launch::Shell, Size::new(24, 80));
    assert_eq!(
        fixture.host.open(OWNER, &open_request).unwrap_err().reason,
        Reason::Unavailable
    );
}

#[test]
fn dropping_the_host_kills_the_process_group() {
    let fixture = fixture();
    let terminal = open(&fixture.host, sh("trap '' HUP TERM; echo ready; sleep 60"));
    let group = fixture.host.process_group(&terminal).unwrap();
    let mut reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Observe, 0);
    assert!(reader.sees("ready"));
    drop(fixture);
    assert!(!supervise::running(group));
}

#[test]
fn an_idle_terminal_expires() {
    let fixture = fixture_with(|config| config.idle = Duration::from_secs(3600));
    let terminal = open(&fixture.host, command("/bin/cat", &[]));
    let group = fixture.host.process_group(&terminal).unwrap();
    let reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Interact, 0);
    // Attached clients keep it alive however long they wait.
    fixture
        .host
        .tick(Instant::now() + Duration::from_secs(7200));
    assert!(supervise::running(group));
    let detach = Detach::new(id(), terminal.clone(), reader.attachment.clone());
    fixture.host.detach(OWNER, &detach).unwrap();
    fixture
        .host
        .tick(Instant::now() + Duration::from_secs(1800));
    assert!(supervise::running(group));
    fixture
        .host
        .tick(Instant::now() + Duration::from_secs(7200));
    assert!(!supervise::running(group));
    let attach = Attach::new(id(), terminal, Mode::Interact, 0, RATE);
    let (sink, _frames) = host::channel(4);
    assert_eq!(
        fixture
            .host
            .attach(OWNER, &attach, Box::new(sink))
            .unwrap_err()
            .reason,
        Reason::Closed
    );
}

#[test]
fn a_terminal_from_an_earlier_host_generation_is_lost() {
    let fixture = fixture();
    let terminal = open(&fixture.host, command("/bin/cat", &[]));
    let restarted = fixture_with(|_| {});
    let attach = Attach::new(id(), terminal.clone(), Mode::Interact, 0, RATE);
    let (sink, _frames) = host::channel(4);
    assert_eq!(
        restarted
            .host
            .attach(OWNER, &attach, Box::new(sink))
            .unwrap_err()
            .reason,
        Reason::Lost
    );
    let unknown = TerminalRef {
        generation: terminal.generation,
        terminal: "ff".repeat(32),
    };
    let attach = Attach::new(id(), unknown, Mode::Interact, 0, RATE);
    let (sink, _frames) = host::channel(4);
    assert_eq!(
        fixture
            .host
            .attach(OWNER, &attach, Box::new(sink))
            .unwrap_err()
            .reason,
        Reason::Unavailable
    );
}

#[test]
fn an_exact_input_retry_is_typed_once() {
    let fixture = fixture();
    let terminal = open(&fixture.host, command("/bin/cat", &[]));
    let mut reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Interact, 0);
    let input = Input::new(id(), terminal.clone(), b"once\n".to_vec());
    assert_eq!(
        fixture.host.input(OWNER, &input).unwrap().0,
        Status::Accepted
    );
    assert_eq!(
        fixture.host.input(OWNER, &input).unwrap().0,
        Status::Duplicate
    );
    let mut changed = input.clone();
    changed.data = b"twice\n".to_vec();
    assert_eq!(
        fixture.host.input(OWNER, &changed).unwrap_err().reason,
        Reason::IdempotencyConflict
    );
    type_in(&fixture.host, OWNER, &terminal, "end\n").unwrap();
    assert!(reader.until(|state| state.screen().text().matches("end").count() >= 2));
    assert_eq!(
        reader.text().matches("once").count(),
        2,
        "{}",
        reader.text()
    );
}

#[test]
fn the_working_directory_and_environment_are_admitted_by_the_host() {
    let fixture = fixture_with(|config| {
        config.env_allow.insert("GREETING".into());
    });
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), fixture.root.path().join("escape")).unwrap();
    let escape = Open::new(id(), WORKSPACE, "escape", Launch::Shell, Size::new(24, 80));
    assert_eq!(
        fixture.host.open(OWNER, &escape).unwrap_err().reason,
        Reason::NotAdmitted
    );
    let other = Open::new(id(), "ee".repeat(32), "", Launch::Shell, Size::new(24, 80));
    assert_eq!(
        fixture.host.open(OWNER, &other).unwrap_err().reason,
        Reason::NotAdmitted
    );

    let mut secret = Open::new(id(), WORKSPACE, "", sh("env"), Size::new(24, 80));
    secret.env.push(EnvVar {
        name: "SECRET".into(),
        value: "x".into(),
    });
    assert_eq!(
        fixture.host.open(OWNER, &secret).unwrap_err().reason,
        Reason::NotAdmitted
    );

    let mut allowed = Open::new(id(), WORKSPACE, "", sh("env"), Size::new(24, 80));
    allowed.env.push(EnvVar {
        name: "GREETING".into(),
        value: "hello-env".into(),
    });
    let Ok((_, Value::Opened { terminal, .. })) = fixture.host.open(OWNER, &allowed) else {
        panic!("open");
    };
    let mut reader = Reader::attach(&fixture.host, OWNER, &terminal, Mode::Observe, 0);
    assert!(reader.exits());
    let text = reader.text();
    assert!(text.contains("GREETING=hello-env"), "{text}");
    assert!(text.contains("TERM=xterm-256color"), "{text}");
    // The host's own environment is cleared, not inherited.
    assert!(!text.contains("CARGO_MANIFEST_DIR"), "{text}");
}

#[test]
fn a_slow_attachment_is_held_to_its_rate() {
    let fixture = fixture();
    let terminal = open(
        &fixture.host,
        sh(
            "i=0; while [ $i -lt 600 ]; do echo 0123456789012345678901234567890123456789; i=$((i+1)); done",
        ),
    );
    let deadline = Instant::now() + WAIT;
    while fixture.host.head(&terminal).unwrap().1 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    // About 25 KiB of output at 8 KiB per second: the first bucket covers
    // the first frames, the rest arrive on later ticks.
    let (sink, frames) = host::channel(4096);
    let attach = Attach::new(id(), terminal, Mode::Observe, 0, 8 * 1024);
    let started = Instant::now();
    fixture.host.attach(OWNER, &attach, Box::new(sink)).unwrap();
    let immediate: usize = frames
        .try_iter()
        .map(|frame| match frame.body {
            Body::Output { data, .. } => data.len(),
            _ => 0,
        })
        .sum();
    assert!(immediate <= 16 * 1024, "{immediate}");
    let mut total = immediate;
    let mut exited = false;
    while !exited && started.elapsed() < WAIT {
        if let Ok(frame) = frames.recv_timeout(Duration::from_millis(200)) {
            match frame.body {
                Body::Output { data, .. } => total += data.len(),
                Body::Exit { .. } => exited = true,
                _ => {}
            }
        }
    }
    assert!(exited);
    assert!(total > 16 * 1024, "{total}");
}

fn libc_sigterm() -> i32 {
    15
}

/// Runs terminals inside a `coder-boundary` write boundary.
struct Bounded(coder_boundary::Boundary);

impl host::Wrap for Bounded {
    fn command(
        &self,
        program: &std::path::Path,
        args: &[std::ffi::OsString],
    ) -> Result<std::process::Command, String> {
        self.0
            .command(program, args)
            .map_err(|error| error.to_string())
    }
}

#[test]
fn a_wrapped_terminal_writes_only_where_its_boundary_allows() {
    let outside = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let boundary = coder_boundary::Boundary::writing(root.path().canonicalize().unwrap())
        .build()
        .unwrap();
    let mut config = Config::new().workspace(WORKSPACE, root.path());
    config.wrap = Some(Arc::new(Bounded(boundary)));
    let host = Host::new(config, Grants::standard());
    let target = outside.path().join("outside.txt");
    let script = "touch inside.txt && echo inside-ok; \
                  if touch \"$1\" 2>/dev/null; then echo outside-written; else echo outside-denied; fi";
    let terminal = open(
        &host,
        command("/bin/sh", &["-c", script, "sh", target.to_str().unwrap()]),
    );
    let mut reader = Reader::attach(&host, OWNER, &terminal, Mode::Observe, 0);
    assert!(reader.exits(), "{}", reader.text());
    let text = reader.text();
    assert!(text.contains("inside-ok"), "{text}");
    assert!(text.contains("outside-denied"), "{text}");
    assert!(root.path().join("inside.txt").exists());
    assert!(!target.exists());
}
