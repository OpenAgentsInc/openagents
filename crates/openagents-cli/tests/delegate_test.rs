//! Delegation, against real child processes.
//!
//! Every claim here is made by running something. The children are stand-in
//! harnesses — shell scripts that speak the wire format of the real ones —
//! reached through the same `OA_CHILD_*` seam the TypeScript harnesses expose
//! for the same reason: a test that cannot substitute the agent can only
//! assert against a real one, which means it costs money or does not run.
//!
//! What the stand-in cannot fake is the part under test: the process is a real
//! process with a real pid in a real directory, its output is read as it is
//! written, and killing it is a real signal to a real process group.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use openagents_cli::delegate::{ChildEvent, ChildLane, DelegationSupervisor};
use openagents_cli::workspace::{Isolation, WorkspacePlan};
use tokio::sync::{mpsc, watch};

/// The environment is one variable per lane and the tests share a process, so
/// two of them setting `OA_CHILD_CLAUDE` at once run each other's stand-in.
/// Held for as long as the variable is set rather than only while setting it.
static ENVIRONMENT: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    // A test that panics while holding it poisons it; the next test still
    // wants the lock, and what it protects has no invariant to have broken.
    ENVIRONMENT.lock().unwrap_or_else(|held| held.into_inner())
}

/// Write an executable stand-in and return its path.
fn stand_in(name: &str, body: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oa-delegate-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

/// Run a fan-out and collect both the outcomes and everything it reported.
async fn run(
    supervisor: &DelegationSupervisor,
    prompt: &str,
    stop_after: Option<Duration>,
) -> (
    Vec<openagents_cli::delegate::ChildWorkerResult>,
    Vec<(Instant, ChildEvent)>,
) {
    let (events, mut incoming) = mpsc::unbounded_channel();
    let collected = tokio::spawn(async move {
        let mut seen = Vec::new();
        while let Some(event) = incoming.recv().await {
            seen.push((Instant::now(), event));
        }
        seen
    });

    let (stop, cancel) = watch::channel(false);
    if let Some(after) = stop_after {
        tokio::spawn(async move {
            tokio::time::sleep(after).await;
            let _ = stop.send(true);
        });
    } else {
        // Held so the channel stays open for the life of the run.
        std::mem::forget(stop);
    }

    let results = supervisor
        .dispatch_streaming(prompt, events, cancel)
        .await
        .expect("no child could be started");
    (results, collected.await.unwrap())
}

/// Every child gets a git worktree of its own, and giving it back leaves
/// nothing behind.
#[tokio::test]
async fn each_child_gets_a_worktree_of_its_own() {
    let here = std::env::current_dir().unwrap();
    let plan = WorkspacePlan::resolve(here.clone(), Isolation::Worktree).await;
    assert_eq!(
        plan.isolation(),
        Isolation::Worktree,
        "the test runs inside a checkout, so a worktree is available"
    );

    let workspaces = plan.prepare(2).await.expect("the worktrees were not made");
    assert_eq!(workspaces.len(), 2);
    assert_ne!(
        workspaces[0].path, workspaces[1].path,
        "two children in one directory is the defect this replaces"
    );

    for workspace in &workspaces {
        assert!(workspace.path.is_dir(), "{}", workspace.path.display());
        // A worktree's `.git` is a file pointing back at the repository, not a
        // directory: this is a worktree and not a copied tree.
        assert!(
            workspace.path.join(".git").is_file(),
            "{} is not a git worktree",
            workspace.path.display()
        );
        // Each child has its own index, so what one writes the other does not
        // see.
        std::fs::write(workspace.path.join("only-mine.txt"), format!("{}", workspace.id)).unwrap();
    }
    assert!(!workspaces[0].path.join("only-mine.txt").is_symlink());
    assert_eq!(
        std::fs::read_to_string(workspaces[0].path.join("only-mine.txt")).unwrap(),
        "1"
    );
    assert_eq!(
        std::fs::read_to_string(workspaces[1].path.join("only-mine.txt")).unwrap(),
        "2"
    );

    for workspace in &workspaces {
        assert_eq!(workspace.release().await, None);
        assert!(
            !workspace.path.exists(),
            "{} outlived the fan-out",
            workspace.path.display()
        );
    }
}

/// A child's output reaches the caller while the child is still running.
///
/// The version this replaces called `Command::output()`, which returns when
/// the child is finished. The assertion is a clock: the first line has to
/// arrive well before the last one, and the child does not exit until it has
/// written both.
#[tokio::test]
async fn a_child_streams_while_it_is_still_running() {
    let harness = stand_in(
        "slow-claude",
        r#"#!/bin/sh
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"first"}]}}'
sleep 2
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"last"}]}}'
echo '{"type":"result","is_error":false,"result":"first last"}'
"#,
    );
    let _exclusive = exclusive();
    std::env::set_var("OA_CHILD_CLAUDE", &harness);

    let supervisor = DelegationSupervisor::new(1, "claude", None)
        .with_isolation(Isolation::Directory);
    let started = Instant::now();
    let (results, events) = run(&supervisor, "ignored by the stand-in", None).await;
    std::env::remove_var("OA_CHILD_CLAUDE");

    assert!(results[0].success, "{}", results[0].output);
    let total = started.elapsed();
    assert!(
        total >= Duration::from_secs(2),
        "the stand-in sleeps for two seconds; this run took {total:?}"
    );

    let first_said = events
        .iter()
        .find_map(|(at, event)| match event {
            ChildEvent::Output { text, .. } if text.contains("first") => Some(*at),
            _ => None,
        })
        .expect("nothing was streamed at all");
    let gap = first_said.duration_since(started);
    assert!(
        gap < Duration::from_secs(1),
        "the first line arrived after {gap:?}, which is the end of the run rather than the start of it"
    );
}

/// Children run at once, and the cap is what decides how many.
///
/// Three children that each sleep for a second finish in about a second when
/// they run together and about three when they are made to queue.
#[tokio::test]
async fn count_is_real_concurrency_and_the_cap_is_real_too() {
    let harness = stand_in(
        "sleepy-claude",
        r#"#!/bin/sh
sleep 1
echo "{\"type\":\"result\",\"is_error\":false,\"result\":\"pid $$\"}"
"#,
    );
    let _exclusive = exclusive();
    std::env::set_var("OA_CHILD_CLAUDE", &harness);

    let together = DelegationSupervisor::new(3, "claude", None)
        .with_isolation(Isolation::Directory);
    let at = Instant::now();
    let (parallel, events) = run(&together, "ignored", None).await;
    let parallel_took = at.elapsed();

    let queued = DelegationSupervisor::new(3, "claude", None)
        .with_isolation(Isolation::Directory)
        .with_max_parallel(1);
    let at = Instant::now();
    let (serial, _) = run(&queued, "ignored", None).await;
    let serial_took = at.elapsed();
    std::env::remove_var("OA_CHILD_CLAUDE");

    assert_eq!(parallel.iter().filter(|r| r.success).count(), 3);
    assert_eq!(serial.iter().filter(|r| r.success).count(), 3);
    assert!(
        parallel_took < Duration::from_millis(2_500),
        "three one-second children took {parallel_took:?} together, which is not together"
    );
    assert!(
        serial_took > Duration::from_millis(2_500),
        "three one-second children capped at one at a time took {serial_took:?}, which is not a cap"
    );

    // Three real processes, three different pids, three different directories.
    let pids: Vec<u32> = parallel.iter().filter_map(|result| result.pid).collect();
    assert_eq!(pids.len(), 3, "a child with no pid is not a child process");
    assert_eq!(
        pids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        3,
        "{pids:?} are not three separate processes"
    );
    let homes: std::collections::BTreeSet<&Path> = parallel
        .iter()
        .filter_map(|result| result.workspace.as_deref())
        .collect();
    assert_eq!(homes.len(), 3, "{homes:?} is not one directory per child");

    let announced = events
        .iter()
        .filter(|(_, event)| matches!(event, ChildEvent::Started { .. }))
        .count();
    assert_eq!(announced, 3);
}

/// A child that fails is reported as failed.
///
/// Both shapes: one that runs and exits non-zero, and one whose binary is not
/// there at all. The version this replaces reported `2/2 children succeeded`
/// in both cases.
#[tokio::test]
async fn a_failing_child_is_reported_as_failed() {
    let harness = stand_in(
        "doomed-claude",
        r#"#!/bin/sh
echo "the tests did not pass" >&2
exit 3
"#,
    );
    let _exclusive = exclusive();
    std::env::set_var("OA_CHILD_CLAUDE", &harness);
    let supervisor = DelegationSupervisor::new(1, "claude", None)
        .with_isolation(Isolation::Directory);
    let (results, _) = run(&supervisor, "ignored", None).await;
    std::env::remove_var("OA_CHILD_CLAUDE");

    assert!(!results[0].success, "a child that exited 3 reported success");
    assert!(results[0].failure.is_some());
    let why = results[0].failure.clone().unwrap();
    assert!(why.contains("code 3"), "{why}");
    assert!(why.contains("the tests did not pass"), "{why}");

    std::env::set_var("OA_CHILD_CLAUDE", "/nonexistent/no-such-agent");
    let supervisor = DelegationSupervisor::new(1, "claude", None)
        .with_isolation(Isolation::Directory);
    let (missing, _) = run(&supervisor, "ignored", None).await;
    std::env::remove_var("OA_CHILD_CLAUDE");

    assert!(!missing[0].success);
    assert!(
        missing[0].failure.clone().unwrap().contains("not on PATH"),
        "{:?}",
        missing[0].failure
    );
}

/// Stopping a fan-out stops the children, and the children's own children.
///
/// The stand-in starts a background process that writes a file after five
/// seconds. If the group was signalled, the file never appears; if only the
/// direct child was killed, it does.
#[tokio::test]
async fn stopping_a_fanout_kills_the_whole_group() {
    let witness = std::env::temp_dir().join(format!("oa-orphan-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&witness);

    let harness = stand_in(
        "runaway-claude",
        &format!(
            r#"#!/bin/sh
sh -c 'sleep 5; echo orphaned > {}' &
sleep 30
"#,
            witness.display()
        ),
    );
    let _exclusive = exclusive();
    std::env::set_var("OA_CHILD_CLAUDE", &harness);

    let supervisor = DelegationSupervisor::new(1, "claude", None)
        .with_isolation(Isolation::Directory);
    let at = Instant::now();
    let (results, _) = run(&supervisor, "ignored", Some(Duration::from_millis(700))).await;
    let took = at.elapsed();
    std::env::remove_var("OA_CHILD_CLAUDE");

    assert!(!results[0].success);
    assert_eq!(results[0].failure.as_deref(), Some("stopped before finishing"));
    assert!(
        took < Duration::from_secs(20),
        "a stopped child ran for {took:?}; the stand-in sleeps for thirty seconds"
    );

    // Long enough for the grandchild to have fired if it were still alive.
    tokio::time::sleep(Duration::from_secs(6)).await;
    assert!(
        !witness.exists(),
        "the child's own subprocess outlived the fan-out that started it"
    );
}

/// A lane name that is not a lane is refused rather than quietly redirected.
#[test]
fn an_unknown_lane_is_not_silently_ox_alpha() {
    assert!(ChildLane::known("claude"));
    assert!(ChildLane::known("opencode/x-preview-f-free"));
    assert!(!ChildLane::known("gemni"));
    assert!(!ChildLane::known(""));
}
