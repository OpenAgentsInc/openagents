//! An agent typist on real PTYs: it types only after an explicit handoff,
//! its commands are journaled as the agent's, and it loses the role the
//! moment the owner types, the handing attachment ends, or the right is
//! revoked.

// The programs these run (`/bin/sh`, `printf`) are Unix's.
#![cfg(unix)]

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use coder_pty::ext::{AgentInput, BlockPage, BlockPageRead, Handoff, Origin};
use coder_pty::host::{Config, Host, Right, Rights, channel};
use coder_pty::wire::{
    Attach, Detach, Input, Launch, Mode, Open, Reason, Size, Status, TerminalRef, Value,
};
use coder_vt::Authority;

const WORKSPACE: &str = "5757575757575757575757575757575757575757575757575757575757575757";
const OWNER: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const PHONE: &str = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";
const AGENT: &str = "0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c";
const THREAD: &str = "7777777777777777777777777777777777777777777777777777777777777777";
const RUN: &str = "8888888888888888888888888888888888888888888888888888888888888888";

/// The devices that hold `terminal`; the agent holds nothing.
struct Grants(Mutex<BTreeSet<&'static str>>);

impl Rights for Grants {
    fn holds(&self, principal: &str, right: Right) -> bool {
        right == Right::Terminal && self.0.lock().unwrap().contains(principal)
    }
}

fn id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{:064x}", NEXT.fetch_add(1, Ordering::SeqCst))
}

/// A program that journals every line it reads as one finished command.
fn journaling() -> Launch {
    let script = r#"
mark() { printf '\033]%s\007' "$1"; }
stty -echo
while IFS= read -r line; do
  mark '133;A'; printf '$ '; mark '133;B'; printf '%s\r\n' "$line"
  mark '133;C'; printf 'ran %s\r\n' "$line"; mark '133;D;0'
done"#;
    Launch::Command {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
    }
}

struct Fixture {
    host: Host,
    grants: Arc<Grants>,
    _root: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let grants = Arc::new(Grants(Mutex::new(BTreeSet::from([OWNER, PHONE]))));
    let mut config = Config::new().workspace(WORKSPACE, root.path());
    config.emulator = Some(Authority::factory(100));
    let host = Host::new(config, grants.clone());
    Fixture {
        host,
        grants,
        _root: root,
    }
}

impl Fixture {
    fn open(&self) -> TerminalRef {
        match self.host.open(
            OWNER,
            &Open::new(id(), WORKSPACE, "", journaling(), Size::new(24, 80)),
        ) {
            Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
            other => panic!("open: {other:?}"),
        }
    }

    fn attach(&self, principal: &str, terminal: &TerminalRef) -> String {
        let (sink, frames) = channel(4096);
        std::mem::forget(frames);
        let request = Attach::new(id(), terminal.clone(), Mode::Interact, 0, 1 << 20).with_typist();
        match self.host.attach(principal, &request, Box::new(sink)) {
            Ok((_, Value::Attached { attachment, .. })) => attachment,
            other => panic!("attach: {other:?}"),
        }
    }

    fn type_in(&self, principal: &str, terminal: &TerminalRef, attachment: &str, text: &str) {
        let request =
            Input::new(id(), terminal.clone(), text.as_bytes()).from_attachment(attachment);
        self.host.input(principal, &request).unwrap();
    }

    fn hand_off(&self, terminal: &TerminalRef, attachment: &str) -> String {
        let request = Handoff::new(id(), terminal.clone(), attachment, AGENT, THREAD, RUN);
        match self.host.hand_off(OWNER, &request) {
            Ok((_, Value::HandedOff { lease })) => lease,
            other => panic!("handoff: {other:?}"),
        }
    }

    fn agent(&self, request: &AgentInput) -> Result<Status, Reason> {
        self.host
            .agent_input(AGENT, request)
            .map(|(status, _)| status)
            .map_err(|refusal| refusal.reason)
    }

    /// The journal once it holds `count` finished blocks.
    fn blocks(&self, terminal: &TerminalRef, count: usize) -> BlockPage {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let request = BlockPageRead::new(id(), terminal.clone(), None, 8);
            let Ok((_, Value::Blocks { page })) = self.host.block_page(OWNER, &request) else {
                panic!("blocks")
            };
            if page.blocks.iter().filter(|b| b.ended.is_some()).count() >= count {
                return page;
            }
            assert!(Instant::now() < deadline, "timed out: {page:?}");
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}

#[test]
fn an_agent_types_only_under_a_handoff_until_the_owner_types() {
    let f = fixture();
    let terminal = f.open();
    let other = f.open();
    let owner = f.attach(OWNER, &terminal);
    let phone = f.attach(PHONE, &terminal);

    // Before a handoff the agent types nothing.
    let early = AgentInput::new(id(), terminal.clone(), id(), b"early\n".to_vec());
    assert_eq!(f.agent(&early), Err(Reason::NotTypist));

    f.type_in(OWNER, &terminal, &owner, "one\n");
    // Attribution follows who holds the role when a command begins on the
    // screen, so the owner's command finishes first.
    f.blocks(&terminal, 1);
    // Only the role's holder hands it over.
    let refused = Handoff::new(id(), terminal.clone(), &phone, AGENT, THREAD, RUN);
    assert_eq!(
        f.host.hand_off(PHONE, &refused).unwrap_err().reason,
        Reason::NotTypist
    );
    let lease = f.hand_off(&terminal, &owner);
    let typed = AgentInput::new(id(), terminal.clone(), &lease, b"two\n".to_vec());
    assert_eq!(f.agent(&typed), Ok(Status::Accepted));
    let page = f.blocks(&terminal, 2);
    let origins: Vec<(&str, Origin)> = page
        .blocks
        .iter()
        .map(|block| (block.command.as_str(), block.origin))
        .collect();
    assert_eq!(
        origins,
        vec![("two", Origin::Agent), ("one", Origin::Unattributed)]
    );
    // The lease names one terminal.
    let elsewhere = AgentInput::new(id(), other.clone(), &lease, b"x\n".to_vec());
    assert_eq!(f.agent(&elsewhere), Err(Reason::NotTypist));
    // The evidence holds who typed how much for which thread, not bytes.
    let (bound, log) = f.host.agent_evidence(&terminal).unwrap().unwrap();
    assert_eq!((bound.thread.as_str(), bound.run.as_str()), (THREAD, RUN));
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].bytes, 4);

    // A key from a device with `terminal` takes the role back at once.
    f.type_in(PHONE, &terminal, &phone, "three\n");
    assert_eq!(
        f.agent(&typed),
        Ok(Status::Duplicate),
        "a replay writes nothing"
    );
    let late = AgentInput::new(id(), terminal.clone(), &lease, b"four\n".to_vec());
    assert_eq!(f.agent(&late), Err(Reason::NotTypist));
    let page = f.blocks(&terminal, 3);
    assert_eq!(page.blocks[0].command, "three");
    assert_eq!(page.blocks[0].origin, Origin::Unattributed);
    assert!(f.host.agent_evidence(&terminal).unwrap().is_none());
}

#[test]
fn a_handoff_ends_with_its_attachment_or_its_right() {
    let f = fixture();
    let terminal = f.open();
    let owner = f.attach(OWNER, &terminal);
    let lease = f.hand_off(&terminal, &owner);
    f.host
        .detach(OWNER, &Detach::new(id(), terminal.clone(), &owner))
        .unwrap();
    let input = AgentInput::new(id(), terminal.clone(), &lease, b"x\n".to_vec());
    assert_eq!(f.agent(&input), Err(Reason::NotTypist));

    let owner = f.attach(OWNER, &terminal);
    let lease = f.hand_off(&terminal, &owner);
    f.grants.0.lock().unwrap().remove(OWNER);
    f.host.tick(Instant::now());
    let input = AgentInput::new(id(), terminal.clone(), &lease, b"y\n".to_vec());
    assert_eq!(f.agent(&input), Err(Reason::NotTypist));
}
