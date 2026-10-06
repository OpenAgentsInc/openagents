//! What a terminal share discloses from a host with an authoritative
//! emulator: block records only for commands that began after the share,
//! no directory reported before it, and no snapshot or history unless the
//! share starts at the terminal's first output.

// The programs these run (`/bin/sh`, `printf`) are Unix's.
#![cfg(unix)]

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use coder_pty::ext::{BlockPage, BlockPageRead, Join};
use coder_pty::host::{Authorize, Config, Host, Right, Rights, deliveries};
use coder_pty::share::{ShareGrant, ShareMode, ShareRequest};
use coder_pty::wire::{
    Attach, Input, Launch, Mode, Open, Reason, Refusal, Size, Status, TerminalRef, Value,
};
use coder_vt::Authority;

const WORKSPACE: &str = "5757575757575757575757575757575757575757575757575757575757575757";
const OWNER: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const FRIEND: &str = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";

struct Grants;

impl Rights for Grants {
    fn holds(&self, principal: &str, right: Right) -> bool {
        right == Right::Terminal && principal == OWNER
    }
}

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

fn later(seconds: u64) -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + seconds
}

/// A program that prints the marks of one command in `/srv/secret`, waits
/// for a line, then prints the marks of another command.
fn marked() -> Launch {
    let script = r#"
mark() { printf '\033]%s\007' "$1"; }
mark '7;file://host/srv/secret'; mark '133;A'; printf '$ '; mark '133;B'; printf 'cat password\r\n'
mark '133;C'; printf 'hunter2\r\n'; mark '133;D;0'
mark '133;A'; printf '$ '
IFS= read -r wait
mark '133;B'; printf 'make\r\n'
mark '133;C'; printf 'building\r\n'; mark '133;D;0'
mark '133;A'; printf '$ '
IFS= read -r wait"#;
    Launch::Command {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
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

fn wait_for(host: &Host, terminal: &TerminalRef, blocks: usize) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while read(host, OWNER, terminal).unwrap().blocks.len() < blocks {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(50));
    }
    // Let the prompt after the command arrive too.
    std::thread::sleep(Duration::from_millis(300));
}

#[test]
fn a_share_reads_only_blocks_and_state_from_after_it_began() {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::new().workspace(WORKSPACE, root.path());
    config.emulator = Some(Authority::factory(100));
    config.shares = Some(Arc::new(Stamp));
    let host = Host::new(config, Arc::new(Grants));
    assert!(host.features().shares);
    let terminal = match host.open(
        OWNER,
        &Open::new(id(), WORKSPACE, "", marked(), Size::new(24, 80)),
    ) {
        Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
        other => panic!("open: {other:?}"),
    };
    wait_for(&host, &terminal, 1);

    // Without a share the friend reads no blocks.
    assert_eq!(
        read(&host, FRIEND, &terminal).unwrap_err(),
        Reason::NotAdmitted
    );
    let request = ShareRequest::new(id(), terminal.clone(), FRIEND, ShareMode::Watch, later(600));
    host.share(OWNER, &request).unwrap();
    // Nothing began after the share yet.
    let page = read(&host, FRIEND, &terminal).unwrap();
    assert_eq!((page.newest, page.oldest), (None, None));
    assert!(page.blocks.is_empty());

    // A snapshot would show the screen from before the share.
    let (sink, _parts) = deliveries(64);
    let snapshot =
        Attach::new(id(), terminal.clone(), Mode::Observe, 0, 1 << 20).joining(Join::Snapshot);
    assert_eq!(
        host.attach(FRIEND, &snapshot, Box::new(sink))
            .unwrap_err()
            .reason,
        Reason::NotAdmitted
    );

    host.input(OWNER, &Input::new(id(), terminal.clone(), b"go\n".to_vec()))
        .unwrap();
    wait_for(&host, &terminal, 2);
    let page = read(&host, FRIEND, &terminal).unwrap();
    let commands: Vec<&str> = page.blocks.iter().map(|b| b.command.as_str()).collect();
    assert_eq!(commands, vec!["make"]);
    assert_eq!(page.newest, page.oldest);
    assert!(!page.more);
    // The directory came before the share.
    assert_eq!(page.blocks[0].dir, "");
    // The owner still reads everything.
    let owned = read(&host, OWNER, &terminal).unwrap();
    assert_eq!(owned.blocks.len(), 2);
    assert_eq!(owned.blocks[1].dir, "/srv/secret");

    // A share from the first output discloses everything, snapshots too.
    let whole = ShareRequest::new(id(), terminal.clone(), FRIEND, ShareMode::Watch, later(600))
        .from_sequence(1);
    host.share(OWNER, &whole).unwrap();
    assert_eq!(read(&host, FRIEND, &terminal).unwrap().blocks.len(), 2);
    let (sink, _parts) = deliveries(4096);
    let snapshot =
        Attach::new(id(), terminal.clone(), Mode::Observe, 0, 1 << 20).joining(Join::Snapshot);
    assert!(host.attach(FRIEND, &snapshot, Box::new(sink)).is_ok());
}

#[test]
fn after_a_pause_no_read_reconstructs_what_ran_during_it() {
    use coder_pty::share::SharePause;
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::new().workspace(WORKSPACE, root.path());
    config.emulator = Some(Authority::factory(100));
    config.shares = Some(Arc::new(Stamp));
    let host = Host::new(config, Arc::new(Grants));
    let terminal = match host.open(
        OWNER,
        &Open::new(id(), WORKSPACE, "", marked(), Size::new(24, 80)),
    ) {
        Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
        other => panic!("open: {other:?}"),
    };
    wait_for(&host, &terminal, 1);
    // A share of everything, snapshots included, until the pause.
    let whole = ShareRequest::new(id(), terminal.clone(), FRIEND, ShareMode::Watch, later(600))
        .from_sequence(1);
    host.share(OWNER, &whole).unwrap();
    assert_eq!(read(&host, FRIEND, &terminal).unwrap().blocks.len(), 1);

    host.pause(OWNER, &SharePause::new(id(), terminal.clone(), true))
        .unwrap();
    let snapshot = || {
        let (sink, _parts) = deliveries(4096);
        let request =
            Attach::new(id(), terminal.clone(), Mode::Observe, 0, 1 << 20).joining(Join::Snapshot);
        host.attach(FRIEND, &request, Box::new(sink))
            .map(drop)
            .map_err(|refusal| refusal.reason)
    };
    assert_eq!(snapshot(), Err(Reason::NotAdmitted));
    host.input(OWNER, &Input::new(id(), terminal.clone(), b"go\n".to_vec()))
        .unwrap();
    wait_for(&host, &terminal, 2);
    assert!(read(&host, FRIEND, &terminal).unwrap().blocks.is_empty());
    host.pause(OWNER, &SharePause::new(id(), terminal.clone(), false))
        .unwrap();

    // After the resume the command run during the pause stays hidden, and
    // a snapshot, which would show the screen, is refused.
    let page = read(&host, FRIEND, &terminal).unwrap();
    assert!(
        page.blocks.iter().all(|block| block.command != "make"),
        "{page:?}"
    );
    assert_eq!(snapshot(), Err(Reason::NotAdmitted));
    // The owner still reads both.
    assert_eq!(read(&host, OWNER, &terminal).unwrap().blocks.len(), 2);
}
