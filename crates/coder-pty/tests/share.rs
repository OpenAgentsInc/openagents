//! Terminal shares on real PTYs: a device without any right watches or
//! drives exactly one shared terminal, sees nothing written before the
//! share, and loses the attachment when the share expires or ends.

// `cat` and `/bin/sh` are Unix's.
#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use coder_pty::ext::Seat;
use coder_pty::host::{Authorize, Config, Host, Right, Rights, channel};
use coder_pty::share::{ShareGrant, ShareMode, ShareRequest, Unshare};
use coder_pty::wire::{
    Attach, Body, Close, Detached, Frame, Input, Launch, Mode, Open, Reason, Refusal, Resize,
    Signal, SignalKind, Size, Status, TerminalRef, Value,
};

const WORKSPACE: &str = "5757575757575757575757575757575757575757575757575757575757575757";
const OWNER: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const FRIEND: &str = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";
const THIRD: &str = "0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c";

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

/// Signs nothing: the envelope names the share it carries.
struct Stamp;

impl Authorize for Stamp {
    fn authorize(&self, grant: &ShareGrant) -> Result<serde_json::Value, Refusal> {
        Ok(serde_json::json!({ "share": grant.share }))
    }
}

fn id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{:064x}", NEXT.fetch_add(1, Ordering::SeqCst))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

struct Fixture {
    host: Host,
    grants: Arc<Grants>,
    _root: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let grants = Arc::new(Grants::default());
    grants
        .0
        .lock()
        .unwrap()
        .entry(OWNER)
        .or_default()
        .insert("terminal");
    let mut config = Config::new().workspace(WORKSPACE, root.path());
    config.shares = Some(Arc::new(Stamp));
    let host = Host::new(config, grants.clone());
    Fixture {
        host,
        grants,
        _root: root,
    }
}

impl Fixture {
    /// A terminal that echoes what it is sent, without the PTY's own echo.
    fn open(&self) -> TerminalRef {
        let launch = Launch::Command {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "stty -echo; cat".into()],
        };
        match self.host.open(
            OWNER,
            &Open::new(id(), WORKSPACE, "", launch, Size::new(24, 80)),
        ) {
            Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
            other => panic!("open: {other:?}"),
        }
    }

    fn share(&self, principal: &str, request: ShareRequest) -> Result<ShareGrant, Reason> {
        match self.host.share(principal, &request) {
            Ok((_, Value::Shared { grant, .. })) => Ok(grant),
            Ok(other) => panic!("share: {other:?}"),
            Err(refusal) => Err(refusal.reason),
        }
    }

    fn attach(
        &self,
        principal: &str,
        terminal: &TerminalRef,
        mode: Mode,
    ) -> Result<(String, Receiver<Frame>), Reason> {
        let (sink, frames) = channel(4096);
        let request = Attach::new(id(), terminal.clone(), mode, 0, 1 << 20).with_typist();
        match self.host.attach(principal, &request, Box::new(sink)) {
            Ok((Status::Accepted, Value::Attached { attachment, .. })) => Ok((attachment, frames)),
            Ok(other) => panic!("attach: {other:?}"),
            Err(refusal) => Err(refusal.reason),
        }
    }

    fn type_in(
        &self,
        principal: &str,
        terminal: &TerminalRef,
        attachment: &str,
        text: &str,
    ) -> Result<(), Reason> {
        let request =
            Input::new(id(), terminal.clone(), text.as_bytes()).from_attachment(attachment);
        self.host
            .input(principal, &request)
            .map(drop)
            .map_err(|refusal| refusal.reason)
    }

    /// Waits until `terminal` has produced output past `past` and then
    /// nothing more for a moment, and answers its head.
    fn settle(&self, terminal: &TerminalRef, past: u64) -> u64 {
        let deadline = Instant::now() + Duration::from_secs(5);
        let (mut head, mut since) = (self.host.head(terminal).unwrap().0, Instant::now());
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
            let (now, _) = self.host.head(terminal).unwrap();
            if now != head {
                (head, since) = (now, Instant::now());
            } else if head > past && since.elapsed() > Duration::from_millis(300) {
                break;
            }
        }
        head
    }
}

/// The output text and sequence numbers a device received within `wait`,
/// and how its attachment ended, if it did.
fn drain(frames: &Receiver<Frame>, wait: Duration) -> (String, Vec<u64>, Option<Detached>) {
    let deadline = Instant::now() + wait;
    let (mut text, mut seqs, mut ended) = (String::new(), Vec::new(), None);
    while Instant::now() < deadline {
        while let Ok(frame) = frames.try_recv() {
            match frame.body {
                Body::Output { seq, data } => {
                    seqs.push(seq);
                    text.push_str(&String::from_utf8_lossy(&data));
                }
                Body::Gap { from, .. } => seqs.push(from),
                Body::Detached { reason } => ended = Some(reason),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    (text, seqs, ended)
}

#[test]
fn a_watcher_without_a_grant_sees_one_terminal_from_the_share_on() {
    let f = fixture();
    let terminal = f.open();
    let other = f.open();
    let (owner, _owner_frames) = f.attach(OWNER, &terminal, Mode::Interact).unwrap();
    f.type_in(OWNER, &terminal, &owner, "before-share\n")
        .unwrap();
    let head = f.settle(&terminal, 0);

    // Without a share the friend reaches nothing.
    assert_eq!(
        f.attach(FRIEND, &terminal, Mode::Observe).unwrap_err(),
        Reason::NotAdmitted
    );
    let grant = f
        .share(
            OWNER,
            ShareRequest::new(
                id(),
                terminal.clone(),
                FRIEND,
                ShareMode::Watch,
                now() + 600,
            ),
        )
        .unwrap();
    assert_eq!(grant.from, head + 1);
    assert_eq!(grant.issuer, OWNER);
    assert!(f.host.shared_with(FRIEND));
    assert!(!f.host.shared_with(THIRD));

    // The watcher asks for everything and receives only what follows.
    let (watching, frames) = f.attach(FRIEND, &terminal, Mode::Observe).unwrap();
    f.type_in(OWNER, &terminal, &owner, "after-share\n")
        .unwrap();
    let (text, seqs, ended) = drain(&frames, Duration::from_millis(600));
    assert!(text.contains("after-share"), "{text:?}");
    assert!(!text.contains("before-share"), "{text:?}");
    assert!(seqs.iter().all(|seq| *seq > head), "{seqs:?} after {head}");
    assert_eq!(ended, None);

    // Watching grants no input, resize, signal, take, open, or close, and
    // no interacting attachment.
    assert_eq!(
        f.type_in(FRIEND, &terminal, &watching, "x"),
        Err(Reason::NotAdmitted)
    );
    let resize = Resize::new(id(), terminal.clone(), Size::new(10, 10));
    assert_eq!(
        f.host.resize(FRIEND, &resize).unwrap_err().reason,
        Reason::NotAdmitted
    );
    let signal = Signal::new(id(), terminal.clone(), SignalKind::Interrupt);
    assert_eq!(
        f.host.signal(FRIEND, &signal).unwrap_err().reason,
        Reason::NotAdmitted
    );
    let take = Seat::take(id(), terminal.clone(), &watching);
    assert_eq!(
        f.host.seat(FRIEND, &take).unwrap_err().reason,
        Reason::NotAdmitted
    );
    let open = Open::new(id(), WORKSPACE, "", Launch::Shell, Size::new(24, 80));
    assert_eq!(
        f.host.open(FRIEND, &open).unwrap_err().reason,
        Reason::NotAdmitted
    );
    let close = Close::new(id(), terminal.clone());
    assert_eq!(
        f.host.close(FRIEND, &close).unwrap_err().reason,
        Reason::NotAdmitted
    );
    assert_eq!(
        f.attach(FRIEND, &terminal, Mode::Interact).unwrap_err(),
        Reason::NotAdmitted
    );
    // Nor any other terminal.
    assert_eq!(
        f.attach(FRIEND, &other, Mode::Observe).unwrap_err(),
        Reason::NotAdmitted
    );
    // Nor a share of its own: re-sharing needs a delegation that narrows.
    assert_eq!(
        f.share(
            FRIEND,
            ShareRequest::new(id(), terminal.clone(), THIRD, ShareMode::Watch, now() + 60),
        )
        .unwrap_err(),
        Reason::NotAdmitted
    );
    let (_, ring_alive) = f.host.head(&terminal).unwrap();
    assert!(ring_alive, "the watcher closed nothing");
}

#[test]
fn a_driver_types_only_while_it_holds_the_typist_role() {
    let f = fixture();
    let terminal = f.open();
    let (owner, owner_frames) = f.attach(OWNER, &terminal, Mode::Interact).unwrap();
    f.type_in(OWNER, &terminal, &owner, "mine\n").unwrap();
    f.share(
        OWNER,
        ShareRequest::new(
            id(),
            terminal.clone(),
            FRIEND,
            ShareMode::Drive,
            now() + 600,
        ),
    )
    .unwrap();
    let (driving, _frames) = f.attach(FRIEND, &terminal, Mode::Interact).unwrap();
    // The owner holds the role, so the driver waits for it.
    assert_eq!(
        f.type_in(FRIEND, &terminal, &driving, "x\n"),
        Err(Reason::NotTypist)
    );
    let take = Seat::take(id(), terminal.clone(), &driving);
    f.host.seat(FRIEND, &take).unwrap();
    f.type_in(FRIEND, &terminal, &driving, "driven\n").unwrap();
    let (text, _, _) = drain(&owner_frames, Duration::from_millis(500));
    assert!(text.contains("driven"), "{text:?}");
    // A driver still never signals the terminal.
    let signal =
        Signal::new(id(), terminal.clone(), SignalKind::Interrupt).from_attachment(&driving);
    assert_eq!(
        f.host.signal(FRIEND, &signal).unwrap_err().reason,
        Reason::NotAdmitted
    );
    // Any key the owner presses after taking the role back wins.
    let take = Seat::take(id(), terminal.clone(), &owner);
    f.host.seat(OWNER, &take).unwrap();
    assert_eq!(
        f.type_in(FRIEND, &terminal, &driving, "y\n"),
        Err(Reason::NotTypist)
    );
}

#[test]
fn delegation_only_narrows_and_ending_a_share_ends_its_children() {
    let f = fixture();
    let terminal = f.open();
    let other = f.open();
    let (owner, _owner_frames) = f.attach(OWNER, &terminal, Mode::Interact).unwrap();
    f.type_in(OWNER, &terminal, &owner, "one\n").unwrap();
    let head = f.settle(&terminal, 0);
    let expires = now() + 600;
    let parent = f
        .share(
            OWNER,
            ShareRequest::new(id(), terminal.clone(), FRIEND, ShareMode::Watch, expires),
        )
        .unwrap();
    let child = |mode, from: Option<u64>, expires_at, on: &TerminalRef| {
        let mut request =
            ShareRequest::new(id(), on.clone(), THIRD, mode, expires_at).under(&parent.share);
        request.from = from;
        f.share(FRIEND, request)
    };
    // Wider mode, earlier sequence, later expiry, or another terminal: no.
    assert_eq!(
        child(ShareMode::Drive, None, expires, &terminal).unwrap_err(),
        Reason::NotAdmitted
    );
    assert_eq!(
        child(ShareMode::Watch, Some(1), expires, &terminal).unwrap_err(),
        Reason::NotAdmitted
    );
    assert_eq!(
        child(ShareMode::Watch, None, expires + 1, &terminal).unwrap_err(),
        Reason::NotAdmitted
    );
    assert_eq!(
        child(ShareMode::Watch, None, expires, &other).unwrap_err(),
        Reason::NotAdmitted
    );
    // Only the grantee delegates its share.
    let foreign = ShareRequest::new(id(), terminal.clone(), THIRD, ShareMode::Watch, expires)
        .under(&parent.share);
    assert_eq!(
        f.host.share(OWNER, &foreign).unwrap_err().reason,
        Reason::NotAdmitted
    );
    let narrowed = child(ShareMode::Watch, None, expires - 60, &terminal).unwrap();
    assert_eq!(narrowed.parent.as_deref(), Some(parent.share.as_str()));
    assert!(narrowed.from > head);
    let (_, third) = f.attach(THIRD, &terminal, Mode::Observe).unwrap();

    // Ending the parent ends the child's attachment at once.
    f.host
        .unshare(OWNER, &Unshare::one(id(), terminal.clone(), &parent.share))
        .unwrap();
    let (_, _, ended) = drain(&third, Duration::from_millis(300));
    assert_eq!(ended, Some(Detached::Revoked));
    assert_eq!(
        f.attach(THIRD, &terminal, Mode::Observe).unwrap_err(),
        Reason::NotAdmitted
    );
    assert!(!f.host.shared_with(THIRD));
}

#[test]
fn expiry_the_issuers_right_and_ending_every_share_end_delivery() {
    let f = fixture();
    let terminal = f.open();
    let (owner, _owner_frames) = f.attach(OWNER, &terminal, Mode::Interact).unwrap();

    // A share that expires in two seconds.
    f.share(
        OWNER,
        ShareRequest::new(id(), terminal.clone(), FRIEND, ShareMode::Watch, now() + 2),
    )
    .unwrap();
    let (_, frames) = f.attach(FRIEND, &terminal, Mode::Observe).unwrap();
    let (_, _, ended) = drain(&frames, Duration::from_millis(3500));
    assert_eq!(ended, Some(Detached::Revoked));
    f.type_in(OWNER, &terminal, &owner, "later\n").unwrap();
    assert_eq!(
        f.attach(FRIEND, &terminal, Mode::Observe).unwrap_err(),
        Reason::NotAdmitted
    );

    // Ending every share advances the epoch.
    f.share(
        OWNER,
        ShareRequest::new(
            id(),
            terminal.clone(),
            FRIEND,
            ShareMode::Watch,
            now() + 600,
        ),
    )
    .unwrap();
    let (_, frames) = f.attach(FRIEND, &terminal, Mode::Observe).unwrap();
    // Only a device with `terminal` ends them all.
    assert_eq!(
        f.host
            .unshare(FRIEND, &Unshare::all(id(), terminal.clone()))
            .unwrap_err()
            .reason,
        Reason::NotAdmitted
    );
    f.host
        .unshare(OWNER, &Unshare::all(id(), terminal.clone()))
        .unwrap();
    let (_, _, ended) = drain(&frames, Duration::from_millis(300));
    assert_eq!(ended, Some(Detached::Revoked));

    // A share lasts only while its issuer holds `terminal`.
    f.share(
        OWNER,
        ShareRequest::new(
            id(),
            terminal.clone(),
            FRIEND,
            ShareMode::Watch,
            now() + 600,
        ),
    )
    .unwrap();
    let (_, frames) = f.attach(FRIEND, &terminal, Mode::Observe).unwrap();
    f.grants.0.lock().unwrap().remove(OWNER);
    f.host.tick(Instant::now());
    let (_, _, ended) = drain(&frames, Duration::from_millis(300));
    assert_eq!(ended, Some(Detached::Revoked));
}

#[test]
fn a_share_request_is_bounded_and_idempotent() {
    let f = fixture();
    let terminal = f.open();
    let request = ShareRequest::new(
        id(),
        terminal.clone(),
        FRIEND,
        ShareMode::Watch,
        now() + 600,
    );
    let first = f.host.share(OWNER, &request).unwrap();
    let again = f.host.share(OWNER, &request).unwrap();
    assert_eq!(again.0, Status::Duplicate);
    assert_eq!(again.1, first.1);
    let Value::Shared {
        authorization,
        grant,
    } = first.1
    else {
        panic!("shared")
    };
    assert_eq!(authorization["share"], grant.share);
    for bad in [
        ShareRequest::new(id(), terminal.clone(), OWNER, ShareMode::Watch, now() + 600),
        ShareRequest::new(
            id(),
            terminal.clone(),
            FRIEND,
            ShareMode::Watch,
            now() + 8 * 86_400,
        ),
        ShareRequest::new(
            id(),
            terminal.clone(),
            FRIEND,
            ShareMode::Watch,
            now() + 600,
        )
        .from_sequence(1_000_000),
    ] {
        assert_eq!(
            f.host.share(OWNER, &bad).unwrap_err().reason,
            Reason::Malformed
        );
    }
    // A host without a signer serves no shares.
    let plain = Host::new(
        Config::new().workspace(WORKSPACE, f._root.path()),
        f.grants.clone(),
    );
    assert!(!plain.features().shares);
    assert_eq!(
        plain.share(OWNER, &request).unwrap_err().reason,
        Reason::UnsupportedFeature
    );
}
