//! One typist per terminal, on real PTYs: whoever types first holds the
//! role, take and release move it, and nothing else can keep or steal it.

// `cat` and `/bin/sh` are Unix's.
#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use coder_pty::ext::Seat;
use coder_pty::host::{Config, Host, Right, Rights, channel};
use coder_pty::wire::{
    Attach, Body, Detach, Frame, Input, Launch, Mode, Open, Reason, Resize, Signal, SignalKind,
    Size, Status, TerminalRef, Value,
};

const WORKSPACE: &str = "5757575757575757575757575757575757575757575757575757575757575757";
const LAPTOP: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const PHONE: &str = "0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d";
const OLD: &str = "0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e";

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
    grants: Arc<Grants>,
    terminal: TerminalRef,
    _root: tempfile::TempDir,
}

fn id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{:064x}", NEXT.fetch_add(1, Ordering::SeqCst))
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let grants = Arc::new(Grants::default());
    for device in [LAPTOP, PHONE, OLD] {
        grants
            .0
            .lock()
            .unwrap()
            .entry(device)
            .or_default()
            .insert("terminal");
    }
    let host = Host::new(
        Config::new().workspace(WORKSPACE, root.path()),
        grants.clone(),
    );
    let launch = Launch::Command {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), "stty -echo; cat".into()],
    };
    let terminal = match host.open(
        LAPTOP,
        &Open::new(id(), WORKSPACE, "", launch, Size::new(24, 80)),
    ) {
        Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
        other => panic!("open: {other:?}"),
    };
    Fixture {
        host,
        grants,
        terminal,
        _root: root,
    }
}

struct Device {
    principal: &'static str,
    attachment: String,
    frames: Receiver<Frame>,
    typed: bool,
}

impl Fixture {
    fn attach(&self, principal: &'static str, typed: bool) -> Device {
        let (sink, frames) = channel(4096);
        let mut request = Attach::new(id(), self.terminal.clone(), Mode::Interact, 0, 1 << 20);
        if typed {
            request = request.with_typist();
        }
        let attachment = match self.host.attach(principal, &request, Box::new(sink)) {
            Ok((Status::Accepted, Value::Attached { attachment, .. })) => attachment,
            other => panic!("attach: {other:?}"),
        };
        Device {
            principal,
            attachment,
            frames,
            typed,
        }
    }

    fn input(&self, device: &Device, text: &str) -> Result<(), Reason> {
        let mut request = Input::new(id(), self.terminal.clone(), text.as_bytes());
        if device.typed {
            request = request.from_attachment(&device.attachment);
        }
        self.host
            .input(device.principal, &request)
            .map(drop)
            .map_err(|refusal| refusal.reason)
    }

    fn resize(&self, device: &Device, size: Size) -> Result<(), Reason> {
        let mut request = Resize::new(id(), self.terminal.clone(), size);
        if device.typed {
            request = request.from_attachment(&device.attachment);
        }
        self.host
            .resize(device.principal, &request)
            .map(drop)
            .map_err(|refusal| refusal.reason)
    }

    fn signal(&self, device: &Device) -> Result<(), Reason> {
        let mut request = Signal::new(id(), self.terminal.clone(), SignalKind::Interrupt);
        if device.typed {
            request = request.from_attachment(&device.attachment);
        }
        self.host
            .signal(device.principal, &request)
            .map(drop)
            .map_err(|refusal| refusal.reason)
    }

    fn take(&self, device: &Device) -> Result<(), Reason> {
        let request = Seat::take(id(), self.terminal.clone(), &device.attachment);
        self.host
            .seat(device.principal, &request)
            .map(drop)
            .map_err(|refusal| refusal.reason)
    }

    fn release(&self, device: &Device) -> Result<(), Reason> {
        let request = Seat::release(id(), self.terminal.clone(), &device.attachment);
        self.host
            .seat(device.principal, &request)
            .map(drop)
            .map_err(|refusal| refusal.reason)
    }

    fn detach(&self, device: &Device) {
        let request = Detach::new(id(), self.terminal.clone(), &device.attachment);
        self.host.detach(device.principal, &request).unwrap();
    }
}

impl Device {
    /// The typist frames this device received since the last call.
    fn seats(&self) -> Vec<(Option<String>, Size)> {
        let mut seats = Vec::new();
        let deadline = Instant::now() + Duration::from_millis(300);
        while Instant::now() < deadline {
            while let Ok(frame) = self.frames.try_recv() {
                if let Body::Typist { typist, size } = frame.body {
                    seats.push((typist, size));
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        seats
    }

    /// The typist the latest frame names.
    fn typist(&self) -> Option<Option<String>> {
        self.seats().pop().map(|(typist, _)| typist)
    }
}

#[test]
fn the_first_to_type_holds_the_role_and_others_are_refused() {
    let f = fixture();
    let laptop = f.attach(LAPTOP, true);
    let phone = f.attach(PHONE, true);
    // Both learn there is no typist yet.
    assert_eq!(laptop.typist(), Some(None));
    assert_eq!(phone.typist(), Some(None));
    // A viewer resize at a terminal without a typist does not take it.
    f.resize(&phone, Size::new(30, 90)).unwrap();
    f.input(&laptop, "a\n").unwrap();
    assert_eq!(phone.typist(), Some(Some(laptop.attachment.clone())));
    assert_eq!(f.input(&phone, "b\n"), Err(Reason::NotTypist));
    assert_eq!(f.resize(&phone, Size::new(10, 40)), Err(Reason::NotTypist));
    assert_eq!(f.signal(&phone), Err(Reason::NotTypist));
    f.input(&laptop, "c\n").unwrap();
    // The typist's size reaches the viewer.
    f.resize(&laptop, Size::new(20, 70)).unwrap();
    let seats = phone.seats();
    assert_eq!(
        seats.last(),
        Some(&(Some(laptop.attachment.clone()), Size::new(20, 70)))
    );
    // Naming another device's attachment is not admitted.
    let forged =
        Input::new(id(), f.terminal.clone(), b"x".to_vec()).from_attachment(&laptop.attachment);
    assert_eq!(
        f.host.input(PHONE, &forged).unwrap_err().reason,
        Reason::NotAdmitted
    );
}

#[test]
fn take_and_release_move_the_role_once() {
    let f = fixture();
    let laptop = f.attach(LAPTOP, true);
    let phone = f.attach(PHONE, true);
    f.input(&laptop, "a\n").unwrap();
    // Only the typist releases.
    assert_eq!(f.release(&phone), Err(Reason::NotTypist));
    f.take(&phone).unwrap();
    let seats = laptop.seats();
    assert_eq!(seats.last().unwrap().0, Some(phone.attachment.clone()));
    assert_eq!(f.input(&laptop, "b\n"), Err(Reason::NotTypist));
    f.input(&phone, "c\n").unwrap();
    f.release(&phone).unwrap();
    assert_eq!(laptop.typist(), Some(None));
    // With no typist, the next to type takes it.
    f.input(&laptop, "d\n").unwrap();
    assert_eq!(phone.typist(), Some(Some(laptop.attachment.clone())));
}

#[test]
fn a_detached_or_revoked_typist_keeps_nothing() {
    let f = fixture();
    let laptop = f.attach(LAPTOP, true);
    let phone = f.attach(PHONE, true);
    f.input(&laptop, "a\n").unwrap();
    f.detach(&laptop);
    assert_eq!(phone.typist(), Some(None));
    f.input(&phone, "b\n").unwrap();
    // Revoking the typist's right ends its attachment and the role.
    f.grants.0.lock().unwrap().remove(PHONE);
    let laptop = f.attach(LAPTOP, true);
    f.host.tick(Instant::now());
    assert_eq!(laptop.typist(), Some(None));
    f.input(&laptop, "c\n").unwrap();
}

#[test]
fn a_new_attachment_of_the_typist_device_must_take_the_role() {
    let f = fixture();
    let first = f.attach(LAPTOP, true);
    f.input(&first, "a\n").unwrap();
    // The same device on a new route: it does not inherit the role.
    let second = f.attach(LAPTOP, true);
    assert_eq!(f.input(&second, "b\n"), Err(Reason::NotTypist));
    f.take(&second).unwrap();
    assert_eq!(f.input(&first, "c\n"), Err(Reason::NotTypist));
    f.input(&second, "d\n").unwrap();
}

#[test]
fn an_older_client_types_as_its_device() {
    let f = fixture();
    let old = f.attach(OLD, false);
    let laptop = f.attach(LAPTOP, true);
    f.input(&old, "a\n").unwrap();
    // Newer devices see the older device's attachment as the typist.
    assert_eq!(laptop.typist(), Some(Some(old.attachment.clone())));
    assert_eq!(f.input(&laptop, "b\n"), Err(Reason::NotTypist));
    // The older device keeps typing; a take moves the role.
    f.input(&old, "c\n").unwrap();
    f.take(&laptop).unwrap();
    assert_eq!(f.input(&old, "d\n"), Err(Reason::NotTypist));
    assert_eq!(f.resize(&old, Size::new(10, 40)), Err(Reason::NotTypist));
    // When the older device's last attachment ends, it holds nothing.
    f.release(&laptop).unwrap();
    f.input(&old, "e\n").unwrap();
    f.detach(&old);
    assert_eq!(laptop.typist(), Some(None));
}
