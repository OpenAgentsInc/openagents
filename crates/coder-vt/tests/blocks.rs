//! Block-journal reads from a host terminal on real PTYs: a device that
//! never attached lists commands and outcomes, and a block whose output
//! left the replay buffer says so.

// The programs these run (`/bin/sh`, `printf`) are Unix's.
#![cfg(unix)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use coder_pty::ext::{BlockPage, BlockPageRead, BlockState, Features};
use coder_pty::host::{Config, Host, Right, Rights};
use coder_pty::wire::{Launch, Open, Reason, Size, Status, TerminalRef, Value};
use coder_vt::Authority;

const WORKSPACE: &str = "5757575757575757575757575757575757575757575757575757575757575757";
const OWNER: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const OBSERVER: &str = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";

struct Grants;

impl Rights for Grants {
    fn holds(&self, principal: &str, right: Right) -> bool {
        match right {
            Right::Terminal => principal == OWNER,
            Right::Observe => principal == OBSERVER,
        }
    }
}

fn id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{:064x}", NEXT.fetch_add(1, Ordering::SeqCst))
}

fn host(root: &std::path::Path, adjust: impl FnOnce(&mut Config)) -> Host {
    let mut config = Config::new().workspace(WORKSPACE, root);
    config.emulator = Some(Authority::factory(100));
    adjust(&mut config);
    Host::new(config, Arc::new(Grants))
}

/// A program that prints the marks a hooked shell prints around two
/// commands, the second followed by much more output.
fn marked() -> Launch {
    let script = r#"
mark() { printf '\033]%s\007' "$1"; }
mark '7;file://host/srv/app'; mark '133;A'; printf '$ '; mark '133;B'; printf 'make\r\n'
mark '133;C'; printf 'building\r\n'; mark '133;D;0'
mark '133;A'; printf '$ '; mark '133;B'; printf 'make test\r\n'
mark '133;C'; printf 'failed\r\n'; mark '133;D;1'
i=0; while [ $i -lt 2000 ]; do i=$((i+1)); echo "after $i"; done
mark '133;A'; printf '$ '
IFS= read -r wait"#;
    Launch::Command {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
    }
}

fn open(host: &Host) -> TerminalRef {
    match host.open(
        OWNER,
        &Open::new(id(), WORKSPACE, "", marked(), Size::new(24, 80)),
    ) {
        Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
        other => panic!("open: {other:?}"),
    }
}

fn read(host: &Host, principal: &str, terminal: &TerminalRef) -> Result<BlockPage, Reason> {
    let request = BlockPageRead::new(id(), terminal.clone(), None, 8);
    match host.block_page(principal, &request) {
        Ok((Status::Accepted, Value::Blocks { page })) => {
            page.check(&request).expect("the page checks");
            Ok(page)
        }
        Ok(other) => panic!("block page: {other:?}"),
        Err(refusal) => Err(refusal.reason),
    }
}

/// Waits until both commands are journaled and the output after them ran.
fn journaled(host: &Host, terminal: &TerminalRef) -> BlockPage {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let page = read(host, OWNER, terminal).unwrap();
        let (head, _) = host.head(terminal).unwrap();
        if page.blocks.len() == 2
            && page.blocks[0].state == BlockState::Finished
            && page.blocks[0]
                .output
                .is_some_and(|output| head > output.to + 20)
        {
            return page;
        }
        assert!(Instant::now() < deadline, "timed out: {page:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_device_that_never_attached_lists_commands_and_outcomes() {
    let root = tempfile::tempdir().unwrap();
    let host = host(root.path(), |_| {});
    assert!(host.features().blocks);
    let terminal = open(&host);
    let page = journaled(&host, &terminal);
    let commands: Vec<(&str, Option<i32>, BlockState)> = page
        .blocks
        .iter()
        .map(|block| (block.command.as_str(), block.status, block.state))
        .collect();
    assert_eq!(
        commands,
        vec![
            ("make test", Some(1), BlockState::Finished),
            ("make", Some(0), BlockState::Finished),
        ]
    );
    assert!(page.blocks.iter().all(|block| block.dir == "/srv/app"));
    // The default ring still holds both commands' output.
    assert!(page.blocks.iter().all(|block| block.retained));
    // An observer reads under the observer policy only.
    assert_eq!(
        read(&host, OBSERVER, &terminal).unwrap_err(),
        Reason::NotAdmitted
    );
    let open_to_observers = tempfile::tempdir().unwrap();
    let watched = self::host(open_to_observers.path(), |config| {
        config.observers_read = true
    });
    let terminal = open(&watched);
    journaled(&watched, &terminal);
    assert_eq!(read(&watched, OBSERVER, &terminal).unwrap().blocks.len(), 2);
}

#[test]
fn a_block_whose_output_left_the_ring_is_not_retained() {
    let root = tempfile::tempdir().unwrap();
    let host = host(root.path(), |config| {
        config.ring_frames = 8;
        config.ring_bytes = 4096;
    });
    let terminal = open(&host);
    let page = journaled(&host, &terminal);
    assert!(page.blocks.iter().all(|block| !block.retained));
    assert!(page.blocks.iter().all(|block| block.output.is_some()));
}

#[test]
fn a_host_without_a_journal_refuses_block_reads() {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::new().workspace(WORKSPACE, root.path());
    config.emulator = None;
    let plain = Host::new(config, Arc::new(Grants));
    // Without an emulator the host serves only the typist rule.
    assert_eq!(
        plain.features(),
        Features {
            typist: true,
            ..Features::NONE
        }
    );
    let terminal = open(&plain);
    assert_eq!(
        read(&plain, OWNER, &terminal).unwrap_err(),
        Reason::UnsupportedFeature
    );
}
