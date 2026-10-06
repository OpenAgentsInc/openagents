//! The host's authoritative emulator on real PTYs: one reply to a query
//! however many devices watch, effects as frames, and nothing repeated on
//! reattach.

// The programs these run (`/bin/sh`, `stty`, `dd`) are Unix's.
#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use coder_pty::ext::{Effect, Features};
use coder_pty::host::{Config, Host, Right, Rights, channel};
use coder_pty::wire::{
    Attach, Body, Frame, Input, Launch, Mode, Open, Reason, Size, Status, TerminalRef, Value,
};
use coder_vt::{Authority, Terminal};

const WORKSPACE: &str = "5757575757575757575757575757575757575757575757575757575757575757";
const OWNER: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const OTHER: &str = "0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d";
const OBSERVER: &str = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";
const WAIT: Duration = Duration::from_secs(15);

#[derive(Default)]
struct Grants(Mutex<BTreeMap<&'static str, BTreeSet<&'static str>>>);

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
    _root: tempfile::TempDir,
}

fn fixture(emulator: bool) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::new().workspace(WORKSPACE, root.path());
    config.observers_read = true;
    if emulator {
        config.emulator = Some(Authority::factory(1000));
    }
    let grants = Grants::default();
    {
        let mut map = grants.0.lock().unwrap();
        map.entry(OWNER).or_default().insert("terminal");
        map.entry(OTHER).or_default().insert("terminal");
        map.entry(OBSERVER).or_default().insert("observe");
    }
    Fixture {
        host: Host::new(config, Arc::new(grants)),
        _root: root,
    }
}

fn id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{:064x}", NEXT.fetch_add(1, Ordering::SeqCst))
}

/// Waits for a line, asks the terminal for its status (`CSI 5 n`), reads
/// the four-byte reply, counts any further reply bytes for half a second,
/// then rings the bell, sets a title, asks to write the clipboard, and
/// reports what it read.
const QUERY: &str = r#"stty -echo
IFS= read -r go
stty -icanon min 1 time 0
printf 'ask\033[5n'
reply=$(dd bs=1 count=4 2>/dev/null | od -An -c | tr -d ' \n')
stty min 0 time 5
extra=$(dd bs=1 count=64 2>/dev/null | wc -c | tr -d ' ')
printf '\a\033]0;done title\007\033]52;c;aGk=\007'
printf 'reply=%s extra=%s\n' "$reply" "$extra"
stty icanon min 1
IFS= read -r wait"#;

fn open(host: &Host) -> TerminalRef {
    let launch = Launch::Command {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), QUERY.into()],
    };
    match host.open(
        OWNER,
        &Open::new(id(), WORKSPACE, "", launch, Size::new(24, 80)),
    ) {
        Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
        other => panic!("open: {other:?}"),
    }
}

/// One device: what it received, and its own emulator.
struct Device {
    frames: Receiver<Frame>,
    attachment: String,
    terminal: Terminal,
    effects: Vec<(u64, Effect)>,
}

impl Device {
    fn attach(
        host: &Host,
        principal: &str,
        terminal: &TerminalRef,
        mode: Mode,
        effects: bool,
    ) -> Self {
        let (sink, frames) = channel(4096);
        let mut request = Attach::new(id(), terminal.clone(), mode, 0, 1 << 20);
        if effects {
            request = request.with_effects();
        }
        let attachment = match host.attach(principal, &request, Box::new(sink)) {
            Ok((Status::Accepted, Value::Attached { attachment, .. })) => attachment,
            other => panic!("attach: {other:?}"),
        };
        Device {
            frames,
            attachment,
            terminal: Terminal::new(24, 80, 100),
            effects: Vec::new(),
        }
    }

    /// Applies what arrived, and answers whether output arrived.
    fn drain(&mut self) {
        while let Ok(frame) = self.frames.try_recv() {
            assert_eq!(frame.attachment, self.attachment);
            match frame.body {
                Body::Output { data, .. } => self.terminal.feed(&data),
                Body::Effect { after, effect } => self.effects.push((after, effect)),
                _ => {}
            }
        }
    }

    fn text(&self) -> String {
        self.terminal.text()
    }
}

fn type_line(host: &Host, principal: &str, terminal: &TerminalRef, text: &str) {
    host.input(
        principal,
        &Input::new(id(), terminal.clone(), text.as_bytes()),
    )
    .expect("input");
}

/// Drains every device until `done` holds for the first one.
fn until(devices: &mut [&mut Device], done: impl Fn(&Device) -> bool) {
    let deadline = Instant::now() + WAIT;
    loop {
        for device in devices.iter_mut() {
            device.drain();
        }
        if done(devices[0]) {
            // Let the other devices catch up with the same output.
            std::thread::sleep(Duration::from_millis(200));
            for device in devices.iter_mut() {
                device.drain();
            }
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out: {:?}",
            devices[0].text()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn reported(device: &Device) -> bool {
    device.text().contains("extra=")
}

#[test]
fn two_devices_see_one_reply_and_their_own_effects() {
    let fixture = fixture(true);
    let host = &fixture.host;
    assert!(host.features().effects);
    let terminal = open(host);
    let mut owner = Device::attach(host, OWNER, &terminal, Mode::Interact, true);
    let mut other = Device::attach(host, OTHER, &terminal, Mode::Interact, true);
    let mut observer = Device::attach(host, OBSERVER, &terminal, Mode::Observe, true);
    type_line(host, OWNER, &terminal, "go\n");
    until(&mut [&mut owner, &mut other, &mut observer], reported);
    // The host answered once; neither device answered.
    assert!(
        owner.text().contains("reply=033[0n extra=0"),
        "{}",
        owner.text()
    );
    assert_eq!(owner.text(), other.text());
    assert_eq!(owner.text(), observer.text());
    for device in [&owner, &other, &observer] {
        let kinds: Vec<&Effect> = device.effects.iter().map(|(_, effect)| effect).collect();
        assert!(kinds.contains(&&Effect::Bell { count: 1 }), "{kinds:?}");
        assert!(kinds.contains(&&Effect::Title {
            title: "done title".into()
        }));
        assert!(device.effects.iter().all(|(after, _)| *after > 0));
    }
    // Only the device that typed receives the clipboard write.
    let clipboard = Effect::Clipboard { text: "hi".into() };
    assert!(owner.effects.iter().any(|(_, effect)| *effect == clipboard));
    assert!(!other.effects.iter().any(|(_, effect)| *effect == clipboard));
    assert!(
        !observer
            .effects
            .iter()
            .any(|(_, effect)| *effect == clipboard)
    );

    // A reattach replays the output and repeats no bell or clipboard
    // write; it learns the current title as state.
    let mut late = Device::attach(host, OTHER, &terminal, Mode::Interact, true);
    until(&mut [&mut late], reported);
    assert_eq!(late.text(), owner.text());
    assert_eq!(
        late.effects,
        vec![(
            0,
            Effect::Title {
                title: "done title".into()
            }
        )]
    );
    // A device that names no feature receives no effect frame.
    let mut plain = Device::attach(host, OTHER, &terminal, Mode::Interact, false);
    until(&mut [&mut plain], reported);
    assert!(plain.effects.is_empty());
}

#[test]
fn an_older_device_keeps_answering_and_the_host_stays_quiet() {
    let fixture = fixture(true);
    let host = &fixture.host;
    let terminal = open(host);
    let mut older = Device::attach(host, OWNER, &terminal, Mode::Interact, false);
    let mut newer = Device::attach(host, OTHER, &terminal, Mode::Interact, true);
    type_line(host, OWNER, &terminal, "go\n");
    let deadline = Instant::now() + WAIT;
    // The older device answers from its own emulator, as it always did.
    while !older.text().contains("extra=") {
        older.drain();
        let replies = older.terminal.take_replies();
        if !replies.is_empty() {
            host.input(OWNER, &Input::new(id(), terminal.clone(), replies))
                .unwrap();
        }
        assert!(Instant::now() < deadline, "timed out: {:?}", older.text());
        std::thread::sleep(Duration::from_millis(20));
    }
    until(&mut [&mut newer], reported);
    assert!(
        older.text().contains("reply=033[0n extra=0"),
        "{}",
        older.text()
    );
    assert_eq!(older.text(), newer.text());
}

#[test]
fn a_host_without_an_emulator_refuses_the_effects_feature() {
    let fixture = fixture(false);
    let host = &fixture.host;
    assert_eq!(host.features(), Features::NONE);
    let terminal = open(host);
    let (sink, _frames) = channel(16);
    let request = Attach::new(id(), terminal, Mode::Interact, 0, 1 << 20).with_effects();
    let refusal = host.attach(OWNER, &request, Box::new(sink)).unwrap_err();
    assert_eq!(refusal.reason, Reason::UnsupportedFeature);
}
