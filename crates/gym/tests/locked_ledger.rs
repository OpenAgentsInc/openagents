//! The locked-partition ledger's transaction, exercised across processes
//! and against interrupted writes.
//!
//! `LockedLedger` spends a suite's locked partition once per digest, and the
//! spend is a transaction: the ledger's `*.lock` file is taken, the file is
//! read, eligibility is decided, the record is appended, and the file is
//! synced before the items are handed back. These tests run that
//! transaction in real child processes — each one is this test binary
//! re-executed on a hidden entry point — released together on a filesystem
//! barrier so they reach the ledger at the same time. The fault tests
//! reproduce the on-disk state a writer killed mid-append leaves — the
//! lock file it held and a torn last line — and check the ledger's
//! behavior against that state.
//!
//! Everything runs against a private temporary ledger and the committed
//! `support-v2-three-way` suite. No measurement store, model, or network
//! is involved, and no real ledger is spent.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, exit};
use std::time::{Duration, Instant};

use gym::suite::{LockedLedger, Spend, Suite, SuiteError, support_v2_three_way};

/// The env var a child process reads to find its role.
const CHILD_ROLE: &str = "GYM_LOCKED_LEDGER_CHILD_ROLE";
/// The env var a child process reads to find the shared ledger.
const CHILD_LEDGER: &str = "GYM_LOCKED_LEDGER_PATH";
/// The env var a child process reads to find which suite to spend.
const CHILD_SUITE: &str = "GYM_LOCKED_LEDGER_SUITE";
/// The env var a child process reads to find the release barrier's
/// directory, when a test wants all readers to arrive at once.
const CHILD_BARRIER: &str = "GYM_LOCKED_LEDGER_BARRIER";
/// The env var a child process reads to find the working directory it
/// should adopt before touching a relative ledger path.
const CHILD_CWD: &str = "GYM_LOCKED_LEDGER_CWD";

/// The child's exit code when its read committed and the items came back.
const EXIT_READ: i32 = 0;
/// The child's exit code when it was refused as a second read.
const EXIT_ALREADY_READ: i32 = 3;
/// The child's exit code when an override found nothing to override.
const EXIT_NOTHING_TO_OVERRIDE: i32 = 4;
/// The child's exit code for any other refusal or failure.
const EXIT_FAILED: i32 = 1;

const SPEND: Spend<'static> = Spend {
    subject: "locked-ledger-integration-test",
    reason: "proving the read is a transaction",
    at: "2026-09-20T00:00:00Z",
};

/// The suite a child spends: the committed one, or `b` — the same items
/// minus six under a different name and digest, so it holds a read budget
/// of its own.
fn suite(which: &str) -> Suite {
    let suite = support_v2_three_way().expect("the committed suite loads");
    if which != "b" {
        return suite;
    }
    let mut other = suite;
    other.name = "support-v3".to_owned();
    other.items.truncate(190);
    other.digest = other.compute_digest().expect("a digest");
    other
}

/// The path the ledger's lock is derived from, replicating the crate's
/// resolution independently: the deepest existing ancestor is
/// canonicalized and the missing tail reattached. If this ever disagrees
/// with the implementation, the stale-lock tests fail and say so.
fn resolved(ledger: &Path) -> PathBuf {
    let absolute = std::path::absolute(ledger).unwrap_or_else(|_| ledger.to_path_buf());
    let mut cursor = absolute.clone();
    let mut tail = Vec::new();
    while !cursor.exists() {
        match (cursor.file_name(), cursor.parent()) {
            (Some(name), Some(parent)) => {
                tail.push(name.to_os_string());
                cursor = parent.to_path_buf();
            }
            _ => break,
        }
    }
    let mut resolved = fs::canonicalize(&cursor).unwrap_or(cursor);
    for name in tail.iter().rev() {
        resolved.push(name);
    }
    resolved
}

/// The path a writer's lock file takes: the ledger's canonical path plus
/// `.lock`, by the store's convention.
fn lock_path(ledger: &Path) -> PathBuf {
    PathBuf::from(format!("{}.lock", resolved(ledger).display()))
}

/// Spawns this test binary as a child process in `role` against the shared
/// ledger, returning the running child so several can race. A `Some`
/// barrier tells a `read` child to wait for release before reading.
fn spawn_child(role: &str, ledger: &Path, suite: &str, barrier: Option<&Path>) -> Child {
    let mut command = Command::new(env::current_exe().expect("the test binary's path"));
    command
        .arg("--exact")
        .arg("child_process")
        .arg("--include-ignored")
        .arg("--nocapture")
        .env(CHILD_ROLE, role)
        .env(CHILD_LEDGER, ledger)
        .env(CHILD_SUITE, suite);
    if let Some(barrier) = barrier {
        command.env(CHILD_BARRIER, barrier);
    }
    command.spawn().expect("the child spawns")
}

/// Runs one child to completion and returns its exit code.
fn child(role: &str, ledger: &Path, suite: &str) -> i32 {
    spawn_child(role, ledger, suite, None)
        .wait()
        .expect("the child finishes")
        .code()
        .unwrap_or(-1)
}

/// Collects the exit codes of running children, killing stragglers rather
/// than waiting on them forever.
fn wait_codes(children: Vec<Child>, deadline: Instant) -> Vec<i32> {
    let mut pending = children;
    let mut codes = Vec::new();
    loop {
        pending.retain_mut(|child| match child.try_wait() {
            Ok(Some(status)) => {
                codes.push(status.code().unwrap_or(-1));
                false
            }
            Ok(None) => true,
            Err(_) => {
                codes.push(-1);
                false
            }
        });
        if pending.is_empty() {
            return codes;
        }
        if Instant::now() >= deadline {
            for mut child in pending {
                let _ = child.kill();
                let _ = child.wait();
            }
            panic!("children did not finish; codes so far: {codes:?}");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Spawns one child per reader behind a filesystem barrier, releases them
/// all at once once every child has arrived, and collects their exit
/// codes. Releasing together is what makes the race a race: a child that
/// spawns late would meet the winner's committed record and prove nothing
/// about the lock. A child that never arrives or never finishes is killed
/// rather than waited on forever.
fn run_race(readers: &[(PathBuf, &str)]) -> Vec<i32> {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let barrier = dir.path().join("barrier");
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut children: Vec<Child> = readers
        .iter()
        .map(|(ledger, suite)| spawn_child("read", ledger, suite, Some(&barrier)))
        .collect();
    let ready = barrier.join("ready");
    loop {
        let arrived = fs::read_dir(&ready)
            .map(|entries| entries.count())
            .unwrap_or(0);
        if arrived >= readers.len() {
            break;
        }
        if Instant::now() >= deadline {
            for child in &mut children {
                let _ = child.kill();
                let _ = child.wait();
            }
            panic!(
                "{arrived} of {} children reached the barrier",
                readers.len()
            );
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    fs::write(barrier.join("go"), "go").expect("the release lands");
    wait_codes(children, deadline)
}

/// The barrier wait a `read` child performs: it drops its own marker under
/// the barrier's `ready/` directory and spins until the parent's `go`
/// file appears, so every reader reaches the ledger at the same time. A
/// parent that never releases fails the child rather than hanging it.
fn wait_at_barrier() {
    let Ok(dir) = env::var(CHILD_BARRIER) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let ready = dir.join("ready");
    fs::create_dir_all(&ready).expect("the barrier directory");
    fs::write(ready.join(std::process::id().to_string()), "ready").expect("the marker lands");
    let go = dir.join("go");
    let started = Instant::now();
    while !go.exists() {
        if started.elapsed() > Duration::from_secs(55) {
            eprintln!("the parent never released the barrier");
            exit(EXIT_FAILED);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The entry point the parent tests re-execute. It does nothing under an
/// ordinary `cargo test`; a spawned child finds its role in
/// `GYM_LOCKED_LEDGER_CHILD_ROLE` and reports its outcome as an exit code.
#[test]
#[ignore]
fn child_process() {
    let Ok(role) = env::var(CHILD_ROLE) else {
        return;
    };
    let suite = suite(&env::var(CHILD_SUITE).unwrap_or_else(|_| "a".to_owned()));
    match role.as_str() {
        "read" => {
            wait_at_barrier();
            let path = PathBuf::from(env::var(CHILD_LEDGER).expect("the ledger path"));
            match LockedLedger::at(&path).read_locked(&suite, &SPEND) {
                Ok(_) => exit(EXIT_READ),
                Err(SuiteError::AlreadyRead { .. }) => exit(EXIT_ALREADY_READ),
                Err(error) => {
                    eprintln!("{error}");
                    exit(EXIT_FAILED);
                }
            }
        }
        "again" => {
            let path = PathBuf::from(env::var(CHILD_LEDGER).expect("the ledger path"));
            match LockedLedger::at(&path).read_locked_again(
                &suite,
                &SPEND,
                "chris",
                "the first process's read answered a different question",
            ) {
                Ok(_) => exit(EXIT_READ),
                Err(SuiteError::NothingToOverride { .. }) => exit(EXIT_NOTHING_TO_OVERRIDE),
                Err(SuiteError::AlreadyRead { .. }) => exit(EXIT_ALREADY_READ),
                Err(error) => {
                    eprintln!("{error}");
                    exit(EXIT_FAILED);
                }
            }
        }
        "relative" => {
            // A ledger spelled relative to the working directory: the
            // commit creates and syncs the directory chain under it.
            let cwd = PathBuf::from(env::var(CHILD_CWD).expect("the working directory"));
            env::set_current_dir(&cwd).expect("the child changes directory");
            match LockedLedger::at("a/b/c/ledger.jsonl").read_locked(&suite, &SPEND) {
                Ok(_) => exit(EXIT_READ),
                Err(error) => {
                    eprintln!("{error}");
                    exit(EXIT_FAILED);
                }
            }
        }
        "die-holding" => {
            // The state a writer killed while it held the lock leaves
            // behind: the lock file — beside the ledger's canonical name,
            // which is where the lock is taken — and a torn record from a
            // write that died halfway. This child writes the artifacts and
            // exits normally, so the parent tests the ledger's behavior
            // against the same state a kill leaves, rather than trying to
            // signal a process inside its critical section.
            let path = PathBuf::from(env::var(CHILD_LEDGER).expect("the ledger path"));
            fs::write(lock_path(&path), format!("{}\n", std::process::id()))
                .expect("the lock file lands");
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .expect("the ledger opens");
            file.write_all(b"{\"schema\":\"openagents.gym.locked_read.v1\",\"suite\":\"sup")
                .expect("the torn write lands");
            file.sync_all().expect("the torn write is durable");
            exit(EXIT_READ);
        }
        other => {
            eprintln!("unknown child role {other}");
            exit(EXIT_FAILED);
        }
    }
}

#[test]
fn racing_processes_get_exactly_one_first_read() {
    // The audit's A06 probe, run across real processes released together:
    // callers that once each read an empty ledger and appended as the
    // first reader now meet a transaction — exactly one commits, and the
    // rest find the committed record and are refused.
    let dir = tempfile::tempdir().expect("a temporary directory");
    let ledger = dir.path().join("ledger.jsonl");
    let readers: Vec<(PathBuf, &str)> = (0..8).map(|_| (ledger.clone(), "a")).collect();
    let codes = run_race(&readers);
    assert_eq!(
        codes.iter().filter(|code| **code == EXIT_READ).count(),
        1,
        "exactly one first reader: {codes:?}"
    );
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == EXIT_ALREADY_READ)
            .count(),
        7,
        "everyone else was refused as a second read: {codes:?}"
    );
    assert_eq!(
        LockedLedger::at(&ledger)
            .reads()
            .expect("the ledger reads back")
            .len(),
        1
    );
}

#[test]
fn aliased_spellings_of_one_ledger_share_one_first_read() {
    // The lock lives beside the ledger's canonical name, so readers that
    // spell the same file differently still serialize: the lexical path,
    // the canonical path, a `..` segment, and on Unix a symlinked
    // directory.
    let dir = tempfile::tempdir().expect("a temporary directory");
    let lexical = dir.path().join("ledger.jsonl");
    let canonical = fs::canonicalize(dir.path())
        .expect("the directory resolves")
        .join("ledger.jsonl");
    let dotdot = dir.path().join("subdir/../ledger.jsonl");
    let mut spellings = vec![lexical.clone(), canonical, dotdot];
    #[cfg(unix)]
    {
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(dir.path(), &alias).expect("the symlink lands");
        spellings.push(alias.join("ledger.jsonl"));
    }
    let readers: Vec<(PathBuf, &str)> = (0..8)
        .map(|index| (spellings[index % spellings.len()].clone(), "a"))
        .collect();
    let codes = run_race(&readers);
    assert_eq!(
        codes.iter().filter(|code| **code == EXIT_READ).count(),
        1,
        "one ledger, one first read, however it is spelled: {codes:?}"
    );
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == EXIT_ALREADY_READ)
            .count(),
        7,
        "every other spelling met the committed record: {codes:?}"
    );
}

#[test]
fn each_digest_spends_its_own_first_read() {
    // Reads are counted per digest, so two suites racing in one ledger each
    // get exactly one first read and neither is charged for the other's.
    let dir = tempfile::tempdir().expect("a temporary directory");
    let ledger = dir.path().join("ledger.jsonl");
    let readers: Vec<(PathBuf, &str)> = (0..8)
        .map(|index| (ledger.clone(), if index % 2 == 0 { "a" } else { "b" }))
        .collect();
    let codes = run_race(&readers);
    assert_eq!(
        codes.iter().filter(|code| **code == EXIT_READ).count(),
        2,
        "one first read per digest: {codes:?}"
    );
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == EXIT_ALREADY_READ)
            .count(),
        6,
        "the rest were refused: {codes:?}"
    );
    let reads = LockedLedger::at(&ledger)
        .reads()
        .expect("the ledger reads back");
    assert_eq!(reads.len(), 2);
    assert_ne!(reads[0].digest, reads[1].digest, "one read per suite");
}

#[test]
fn an_override_keeps_its_authority_and_reason_across_processes() {
    // The deliberate second read is the loud door, and it stays loud across
    // process boundaries: the record keeps who authorized it and against
    // what argument.
    let dir = tempfile::tempdir().expect("a temporary directory");
    let ledger = dir.path().join("ledger.jsonl");

    assert_eq!(child("read", &ledger, "a"), EXIT_READ, "the first read");
    assert_eq!(
        child("again", &ledger, "a"),
        EXIT_READ,
        "the override read committed"
    );
    let reads = LockedLedger::at(&ledger)
        .reads()
        .expect("the ledger reads back");
    assert_eq!(reads.len(), 2);
    let recorded = reads[1].overrides.as_ref().expect("the override is on it");
    assert_eq!(recorded.read, 1);
    assert_eq!(recorded.authority, "chris");
    assert!(
        recorded.because.contains("a different question"),
        "the override keeps its argument: {}",
        recorded.because
    );

    // The budget is still spent: a third plain read is refused, and an
    // override on an unread digest has nothing to override.
    assert_eq!(child("read", &ledger, "a"), EXIT_ALREADY_READ);
    assert_eq!(child("again", &ledger, "b"), EXIT_NOTHING_TO_OVERRIDE);
}

#[test]
fn a_relative_ledger_path_is_created_and_committed() {
    // A child works in a directory of its own and names its ledger
    // relative to it — `a/b/c/ledger.jsonl` — so the commit creates the
    // directory chain and syncs each new name in it.
    let dir = tempfile::tempdir().expect("a temporary directory");
    let mut child = Command::new(env::current_exe().expect("the test binary's path"))
        .arg("--exact")
        .arg("child_process")
        .arg("--include-ignored")
        .arg("--nocapture")
        .env(CHILD_ROLE, "relative")
        .env(CHILD_CWD, dir.path())
        .spawn()
        .expect("the child spawns");
    let code = child
        .wait()
        .expect("the child finishes")
        .code()
        .unwrap_or(-1);
    assert_eq!(code, EXIT_READ, "the relative-path read committed");

    let ledger = dir.path().join("a/b/c/ledger.jsonl");
    assert!(ledger.exists(), "the chain was created end to end");
    let reads = LockedLedger::at(&ledger)
        .reads()
        .expect("the ledger reads back");
    assert_eq!(reads.len(), 1);
    let text = fs::read_to_string(&ledger).expect("the ledger reads");
    assert!(text.ends_with('\n'), "the record is a complete line");
}

#[test]
fn the_state_a_killed_writer_leaves_fails_closed_and_recovers() {
    // A `die-holding` child writes the artifacts a writer killed
    // mid-append leaves — the lock file and a torn record — then exits
    // normally. The next reader reports the lock, then the torn line, and
    // neither is read past: the committed read underneath still counts.
    let dir = tempfile::tempdir().expect("a temporary directory");
    let ledger_path = dir.path().join("ledger.jsonl");
    assert_eq!(child("read", &ledger_path, "a"), EXIT_READ);
    assert_eq!(child("die-holding", &ledger_path, "a"), EXIT_READ);

    let ledger = LockedLedger::at(&ledger_path).lock_wait(Duration::from_millis(50));
    let suite = suite("a");
    let error = ledger.read_locked(&suite, &SPEND).unwrap_err();
    assert!(
        matches!(error, SuiteError::Locked { .. }),
        "the held lock is reported: {error}"
    );

    fs::remove_file(lock_path(&ledger_path)).expect("the repair removes the lock");
    let error = ledger.read_locked(&suite, &SPEND).unwrap_err();
    assert!(
        matches!(error, SuiteError::Interrupted { .. }),
        "the torn record fails closed: {error}"
    );

    // Repair is a person's: truncate the file to the last committed line,
    // and the read that committed still counts.
    let bytes = fs::read(&ledger_path).expect("the ledger reads");
    let committed = bytes.iter().rposition(|byte| *byte == b'\n').unwrap() + 1;
    fs::write(&ledger_path, &bytes[..committed]).expect("the repair writes");
    assert!(matches!(
        ledger.read_locked(&suite, &SPEND),
        Err(SuiteError::AlreadyRead { .. })
    ));
}
