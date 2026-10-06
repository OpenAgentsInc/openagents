//! Joining a host terminal by snapshot on real PTYs: a device that joins
//! during output, falls behind, or reads older history converges with one
//! that read everything, without missing or applying a byte twice.

// The programs these run (`/bin/sh`) are Unix's.
#![cfg(unix)]

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use coder_pty::client::{Applied, TerminalState};
use coder_pty::ext::{History, Join};
use coder_pty::host::{Config, Delivery, Host, Right, Rights, channel, deliveries};
use coder_pty::wire::{
    Attach, Body, Frame, Input, Launch, Mode, Open, Reason, Size, Status, TerminalRef, Value,
};
use coder_vt::{Authority, StreamEvent, Streams, Terminal};

const WORKSPACE: &str = "5757575757575757575757575757575757575757575757575757575757575757";
const OWNER: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const WAIT: Duration = Duration::from_secs(20);
const LINES: usize = 4000;

struct Everyone;

impl Rights for Everyone {
    fn holds(&self, principal: &str, _: Right) -> bool {
        principal == OWNER
    }
}

struct Fixture {
    host: Host,
    _root: tempfile::TempDir,
}

fn fixture(adjust: impl FnOnce(&mut Config)) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::new().workspace(WORKSPACE, root.path());
    config.emulator = Some(Authority::factory(5000));
    adjust(&mut config);
    Fixture {
        host: Host::new(config, Arc::new(Everyone)),
        _root: root,
    }
}

fn id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{:064x}", NEXT.fetch_add(1, Ordering::SeqCst))
}

/// Waits for a line, prints numbered lines, then `END`, and waits again.
fn counting(lines: usize) -> Launch {
    let script = format!(
        "stty -echo; IFS= read -r go; i=0; while [ $i -lt {lines} ]; do i=$((i+1)); echo \"line $i\"; done; echo END; IFS= read -r wait"
    );
    Launch::Command {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script],
    }
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

fn type_line(host: &Host, terminal: &TerminalRef, text: &str) {
    host.input(OWNER, &Input::new(id(), terminal.clone(), text.as_bytes()))
        .unwrap();
}

/// A device that reads everything by replay from the start.
struct Reader {
    frames: Receiver<Delivery>,
    vt: Terminal,
}

impl Reader {
    fn attach(host: &Host, terminal: &TerminalRef) -> Self {
        let (sink, frames) = deliveries(1 << 20);
        let request = Attach::new(id(), terminal.clone(), Mode::Interact, 0, 1 << 30);
        host.attach(OWNER, &request, Box::new(sink)).unwrap();
        Reader {
            frames,
            vt: Terminal::new(24, 80, 10_000),
        }
    }

    fn drain(&mut self) {
        while let Ok(delivery) = self.frames.try_recv() {
            match delivery {
                Delivery::Frame(Frame {
                    body: Body::Output { data, .. },
                    ..
                }) => self.vt.feed(&data),
                Delivery::Frame(Frame {
                    body: Body::Gap { .. },
                    ..
                }) => panic!("the reference reader missed output"),
                _ => {}
            }
        }
    }

    fn lines(&self) -> Vec<String> {
        (0..self.vt.scrollback_len() + self.vt.rows())
            .filter_map(|index| self.vt.line(index).map(coder_vt::Row::text))
            .collect()
    }
}

/// A device that joins by snapshot.
struct Joiner {
    terminal: TerminalRef,
    frames: Receiver<Delivery>,
    attachment: String,
    streams: Streams,
    vt: Option<Terminal>,
    state: Option<TerminalState>,
    held: Vec<Frame>,
    snapshots: usize,
    gaps: usize,
    duplicates: usize,
    last_seq: u64,
    finished: usize,
}

impl Joiner {
    fn attach(host: &Host, terminal: &TerminalRef, bound: usize) -> Self {
        let (sink, frames) = deliveries(bound);
        let request = Attach::new(id(), terminal.clone(), Mode::Interact, 0, 1 << 30)
            .joining(Join::Snapshot)
            .with_effects();
        let attachment = match host.attach(OWNER, &request, Box::new(sink)) {
            Ok((Status::Accepted, Value::Attached { attachment, .. })) => attachment,
            other => panic!("attach: {other:?}"),
        };
        Joiner {
            terminal: terminal.clone(),
            frames,
            attachment,
            streams: Streams::new(terminal.clone(), 10_000),
            vt: None,
            state: None,
            held: Vec::new(),
            snapshots: 0,
            gaps: 0,
            duplicates: 0,
            last_seq: 0,
            finished: 0,
        }
    }

    fn drain(&mut self) {
        while let Ok(delivery) = self.frames.try_recv() {
            match delivery {
                Delivery::Records(part) => {
                    assert_eq!(part.attachment, self.attachment);
                    for event in self.streams.push(&part).expect("the stream checks") {
                        self.event(event);
                    }
                }
                Delivery::Frame(frame) => {
                    if self.state.is_some() {
                        self.apply(&frame);
                    } else {
                        self.held.push(frame);
                    }
                }
            }
        }
    }

    fn event(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::Ready {
                terminal, through, ..
            } => {
                self.snapshots += 1;
                self.vt = Some(*terminal);
                self.state =
                    Some(TerminalState::new(self.terminal.clone(), 1, 1).starting_after(through));
                self.last_seq = through;
                for frame in std::mem::take(&mut self.held) {
                    self.apply(&frame);
                }
            }
            StreamEvent::History { epoch, page } => {
                let vt = self.vt.as_mut().expect("history after READY");
                vt.attach_history(epoch, &page).expect("the page attaches");
            }
            StreamEvent::Finished { .. } => self.finished += 1,
        }
    }

    fn apply(&mut self, frame: &Frame) {
        let state = self.state.as_mut().expect("ready");
        match state.apply(frame) {
            Applied::Output { seq, .. } => {
                assert_eq!(seq, self.last_seq + 1, "a byte missed or applied twice");
                self.last_seq = seq;
                if let Body::Output { data, .. } = &frame.body {
                    self.vt.as_mut().unwrap().feed(data);
                }
            }
            Applied::Duplicate => self.duplicates += 1,
            Applied::Gap { .. } => self.gaps += 1,
            _ => {}
        }
    }

    fn lines(&self) -> Vec<String> {
        let vt = self.vt.as_ref().expect("ready");
        (0..vt.scrollback_len() + vt.rows())
            .filter_map(|index| vt.line(index).map(coder_vt::Row::text))
            .collect()
    }
}

fn wait_for(mut drain: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !drain() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Whether `joined` is the tail of `all`, as a joiner's bounded history is.
fn is_tail(joined: &[String], all: &[String]) -> bool {
    joined.len() <= all.len() && all[all.len() - joined.len()..] == *joined
}

fn ended(lines: &[String]) -> bool {
    lines.iter().any(|line| line == "END")
}

#[test]
fn a_join_during_output_converges_without_loss_or_repeats() {
    let fixture = fixture(|_| {});
    let host = &fixture.host;
    let terminal = open(host, counting(LINES));
    let mut reader = Reader::attach(host, &terminal);
    type_line(host, &terminal, "go\n");
    // Join while the program is still printing.
    wait_for(|| {
        reader.drain();
        reader.lines().iter().any(|line| line == "line 300")
    });
    let mut joiner = Joiner::attach(host, &terminal, 1 << 20);
    wait_for(|| {
        reader.drain();
        joiner.drain();
        joiner.vt.is_some() && ended(&joiner.lines()) && ended(&reader.lines())
    });
    std::thread::sleep(Duration::from_millis(200));
    reader.drain();
    joiner.drain();
    assert_eq!(joiner.snapshots, 1);
    assert_eq!((joiner.gaps, joiner.duplicates), (0, 0));
    let vt = joiner.vt.as_ref().unwrap();
    assert_eq!(vt.text(), reader.vt.text());
    assert!(is_tail(&joiner.lines(), &reader.lines()));
    assert!(vt.scrollback_len() > 0);
}

#[test]
fn a_joiner_that_falls_behind_gets_a_fresh_snapshot_not_a_gap() {
    let fixture = fixture(|config| {
        config.ring_frames = 8;
        config.ring_bytes = 8 * 1024;
    });
    let host = &fixture.host;
    let terminal = open(host, counting(LINES));
    let mut reader_vt = Terminal::new(24, 80, 10_000);
    // The reference reads through a deep queue as output arrives.
    let (sink, reference) = channel(1 << 20);
    let request = Attach::new(id(), terminal.clone(), Mode::Interact, 0, 1 << 30);
    host.attach(OWNER, &request, Box::new(sink)).unwrap();
    // The joiner's queue holds two deliveries, and it reads nothing while
    // the program prints, so it falls behind the small ring.
    let mut joiner = Joiner::attach(host, &terminal, 2);
    type_line(host, &terminal, "go\n");
    let mut reference_lines = Vec::new();
    wait_for(|| {
        while let Ok(frame) = reference.try_recv() {
            if let Body::Output { data, .. } = &frame.body {
                reader_vt.feed(data);
            }
            assert!(!matches!(frame.body, Body::Gap { .. }));
        }
        reference_lines = (0..reader_vt.scrollback_len() + reader_vt.rows())
            .filter_map(|index| reader_vt.line(index).map(coder_vt::Row::text))
            .collect();
        ended(&reference_lines)
    });
    wait_for(|| {
        joiner.drain();
        joiner.vt.is_some() && ended(&joiner.lines())
    });
    std::thread::sleep(Duration::from_millis(300));
    joiner.drain();
    assert!(joiner.snapshots >= 2, "{} snapshots", joiner.snapshots);
    assert_eq!((joiner.gaps, joiner.duplicates), (0, 0));
    assert_eq!(joiner.vt.as_ref().unwrap().text(), reader_vt.text());
    assert!(is_tail(&joiner.lines(), &reference_lines));
}

#[test]
fn a_history_read_adds_older_rows_ahead_of_the_kept_ones() {
    let fixture = fixture(|_| {});
    let host = &fixture.host;
    let terminal = open(host, counting(LINES));
    let mut reader = Reader::attach(host, &terminal);
    type_line(host, &terminal, "go\n");
    wait_for(|| {
        reader.drain();
        ended(&reader.lines())
    });
    let mut joiner = Joiner::attach(host, &terminal, 1 << 20);
    wait_for(|| {
        joiner.drain();
        joiner.finished == 1
    });
    // The snapshot carried the newest 2,000 history rows.
    let vt = joiner.vt.as_ref().unwrap();
    assert_eq!(vt.scrollback_len(), 2000);
    let (epoch, before) = (vt.line_epoch(), vt.history_dropped());
    joiner.streams.expect_history(epoch, before, 500);
    let read = History::new(
        id(),
        terminal.clone(),
        &joiner.attachment,
        epoch,
        before,
        500,
    );
    match host.history(OWNER, &read) {
        Ok((Status::Accepted, Value::Stream { .. })) => {}
        other => panic!("history: {other:?}"),
    }
    wait_for(|| {
        joiner.drain();
        joiner.finished == 2
    });
    assert_eq!(joiner.vt.as_ref().unwrap().scrollback_len(), 2500);
    assert!(is_tail(&joiner.lines(), &reader.lines()));
    // A read of rows that left the host's history refuses.
    // A read at or before the host's oldest kept line (line 0 here, since
    // its history kept every line) refuses: those rows are gone.
    let gone = History::new(id(), terminal.clone(), &joiner.attachment, epoch, 0, 10);
    assert_eq!(
        host.history(OWNER, &gone).unwrap_err().reason,
        Reason::ContentUnavailable
    );
    let stale = History::new(id(), terminal, &joiner.attachment, epoch + 1, before, 10);
    assert_eq!(
        host.history(OWNER, &stale).unwrap_err().reason,
        Reason::Stale
    );
}

#[test]
fn a_snapshot_of_an_ended_terminal_carries_its_exit() {
    let fixture = fixture(|_| {});
    let host = &fixture.host;
    let launch = Launch::Command {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), "echo bye; exit 3".into()],
    };
    let terminal = open(host, launch);
    wait_for(|| !host.head(&terminal).unwrap().1);
    let mut joiner = Joiner::attach(host, &terminal, 1 << 10);
    wait_for(|| {
        joiner.drain();
        joiner.finished == 1
    });
    assert!(joiner.vt.as_ref().unwrap().text().contains("bye"));
    assert!(joiner.held.is_empty());
}

#[test]
fn a_transport_without_record_streams_cannot_join_by_snapshot() {
    let fixture = fixture(|_| {});
    let host = &fixture.host;
    assert!(host.features().snapshot);
    let terminal = open(host, counting(1));
    let (sink, _frames) = channel(16);
    let request = Attach::new(id(), terminal, Mode::Interact, 0, 1 << 20).joining(Join::Snapshot);
    let refusal = host.attach(OWNER, &request, Box::new(sink)).unwrap_err();
    assert_eq!(refusal.reason, Reason::UnsupportedFeature);
}
