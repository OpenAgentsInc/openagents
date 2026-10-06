#![cfg(unix)]

use coder_pty::{
    host::{Config, Host, Right, Rights, channel},
    proposal::{Action, Binding, Proposal, Request, State},
    wire::{Attach, Input, Launch, Mode, Open, Reason, Size, Status, TerminalRef, Value},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

fn id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{:064x}", NEXT.fetch_add(1, Ordering::Relaxed))
}
struct Grants {
    phone: AtomicBool,
}
impl Rights for Grants {
    fn holds(&self, principal: &str, right: Right) -> bool {
        right == Right::Terminal
            && (principal == "desktop" || principal == "phone" && self.phone.load(Ordering::SeqCst))
    }
}
struct Fixture {
    host: Host,
    home: tempfile::TempDir,
    terminal: TerminalRef,
    phone: String,
    watcher: String,
    grants: Arc<Grants>,
    _frames: Vec<std::sync::mpsc::Receiver<coder_pty::wire::Frame>>,
    proposal: Proposal,
}
impl Fixture {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().canonicalize().unwrap();
        let workspace = id();
        let mut config = Config::new().workspace(&workspace, &path);
        config.base_env = vec![
            ("HOME".into(), path.display().to_string()),
            ("PATH".into(), "/usr/bin:/bin".into()),
            ("TERM".into(), "xterm".into()),
        ];
        config.emulator = Some(coder_vt::Authority::factory(100));
        config.shell_args = vec!["-c".into(), r#"stty -echo; printf '\033]7;file://host%s\007\033]133;A\007' "$PWD"; while IFS= read -r line; do hex=$(printf %s "$line" | od -An -tx1 | tr -d ' \n'); printf '\033]133;B\007\033]777;openagents;command;%s\007\033]133;C\007' "$hex"; eval "$line"; status=$?; printf '\033]133;D;%s\007\033]133;A\007' "$status"; done"#.into()];
        let grants = Arc::new(Grants {
            phone: AtomicBool::new(true),
        });
        let host = Host::new(config, grants.clone());
        let terminal = match host
            .open(
                "desktop",
                &Open::new(id(), workspace, "", Launch::Shell, Size::new(24, 80)),
            )
            .unwrap()
            .1
        {
            Value::Opened { terminal, .. } => terminal,
            value => panic!("{value:?}"),
        };
        let mut frames = Vec::new();
        let mut attachment = |principal, mode| {
            let (sink, receiver) = channel(4096);
            frames.push(receiver);
            match host
                .attach(
                    principal,
                    &Attach::new(id(), terminal.clone(), mode, 0, 1 << 20).with_effects(),
                    Box::new(sink),
                )
                .unwrap()
                .1
            {
                Value::Attached { attachment, .. } => attachment,
                value => panic!("{value:?}"),
            }
        };
        let phone = attachment("phone", Mode::Interact);
        let watcher = attachment("phone", Mode::Observe);
        let proposal = Proposal {
            thread: id(),
            id: id(),
            revision: 1,
            command: "printf hit >> counter".into(),
            binding: Binding {
                terminal: terminal.terminal.clone(),
                generation: terminal.generation.clone(),
                cwd: path.display().to_string(),
                shell_directory: Some(path.display().to_string()),
                context_digest: id(),
            },
        };
        let result = Self {
            host,
            home,
            terminal,
            phone,
            watcher,
            grants,
            _frames: frames,
            proposal,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if result
                .host
                .proposal(
                    "desktop",
                    &Request::new(
                        id(),
                        result.terminal.clone(),
                        Action::Offer {
                            proposal: result.proposal.clone(),
                        },
                    ),
                )
                .is_ok()
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "shell never offered an empty prompt"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        result
    }
    fn decision(&self, revision: u64, attachment: &str, approve: bool) -> Request {
        Request::new(
            id(),
            self.terminal.clone(),
            Action::Decide {
                thread: self.proposal.thread.clone(),
                proposal: self.proposal.id.clone(),
                revision,
                approve,
                attachment: attachment.into(),
            },
        )
    }
    fn page(&self) -> coder_pty::proposal::Page {
        match self
            .host
            .proposal(
                "phone",
                &Request::new(id(), self.terminal.clone(), Action::Read { limit: 8 }),
            )
            .unwrap()
            .1
        {
            Value::Proposals { page } => page,
            value => panic!("{value:?}"),
        }
    }
}
#[test]
fn desktop_offer_phone_approval_runs_once_and_records_its_block() {
    let f = Fixture::new();
    assert_eq!(f.page().entries[0].proposal, f.proposal);
    assert!(!f.home.path().join("counter").exists());
    let warning = f.decision(1, &f.phone, true);
    assert!(
        matches!(f.host.proposal("phone", &warning).unwrap().1, Value::Proposals { page } if matches!(page.entries[0].state, State::Warned { .. }))
    );
    assert_eq!(
        f.host.proposal("phone", &warning).unwrap().0,
        Status::Duplicate
    );
    assert!(!f.home.path().join("counter").exists());
    let approve = f.decision(1, &f.phone, true);
    f.host.proposal("phone", &approve).unwrap();
    assert_eq!(
        f.host.proposal("phone", &approve).unwrap().0,
        Status::Duplicate
    );
    assert_eq!(
        f.host
            .proposal("phone", &f.decision(1, &f.phone, true))
            .unwrap_err()
            .reason,
        Reason::Stale
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if matches!(f.page().entries[0].state, State::Completed { .. }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no resulting block: {:?}",
            f.page()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        std::fs::read(f.home.path().join("counter")).unwrap(),
        b"hit"
    );
}
#[test]
fn stale_revisions_changed_context_watchers_and_revoked_grants_cannot_approve() {
    let f = Fixture::new();
    assert_eq!(
        f.host
            .proposal("phone", &f.decision(1, &f.watcher, true))
            .unwrap_err()
            .reason,
        Reason::NotAdmitted
    );
    let mut revision = f.proposal.clone();
    revision.revision = 2;
    f.host
        .proposal(
            "desktop",
            &Request::new(
                id(),
                f.terminal.clone(),
                Action::Offer { proposal: revision },
            ),
        )
        .unwrap();
    assert_eq!(
        f.host
            .proposal("phone", &f.decision(1, &f.phone, true))
            .unwrap_err()
            .reason,
        Reason::Stale
    );
    f.grants.phone.store(false, Ordering::SeqCst);
    assert_eq!(
        f.host
            .proposal("phone", &f.decision(2, &f.phone, true))
            .unwrap_err()
            .reason,
        Reason::NotAdmitted
    );
    f.grants.phone.store(true, Ordering::SeqCst);
    f.host
        .input("phone", &Input::new(id(), f.terminal.clone(), b"x"))
        .unwrap();
    assert_eq!(
        f.host
            .proposal("phone", &f.decision(2, &f.phone, true))
            .unwrap_err()
            .reason,
        Reason::Stale
    );
    assert!(!f.home.path().join("counter").exists());
}
#[test]
fn rejection_is_inert_and_exact_request_retries_cannot_relabel_a_proposal() {
    let f = Fixture::new();
    let reject = f.decision(1, &f.phone, false);
    f.host.proposal("phone", &reject).unwrap();
    assert_eq!(f.page().entries[0].state, State::Rejected);
    assert_eq!(
        f.host.proposal("phone", &reject).unwrap().0,
        Status::Duplicate
    );
    let mut changed = reject.clone();
    if let Action::Decide { approve, .. } = &mut changed.action {
        *approve = true;
    }
    assert_eq!(
        f.host.proposal("phone", &changed).unwrap_err().reason,
        Reason::IdempotencyConflict
    );
    assert!(!f.home.path().join("counter").exists());
}
