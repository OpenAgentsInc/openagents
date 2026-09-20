//! What a job owns: its descendants, its deadline, its cancellation, and
//! its reaping.
//!
//! Every test here uses the audit's shape — a harmless marker file written
//! after the job was supposed to be over. A marker that exists afterwards
//! is a process that outlived the bound it was given.

use std::path::Path;
use std::time::Duration;

use supervise::{Ending, Job, Limits};
use tempfile::TempDir;

/// Long enough for a killed tree to have written its marker if it was
/// still alive to write one.
const AFTERWARDS: Duration = Duration::from_millis(2000);

/// A job that starts a background child and waits for it.
fn with_grandchild(marker: &Path, wall: Duration) -> Job {
    Job::new("sh")
        .arg("-c")
        .arg(format!(
            "(sleep 1; printf harmless > '{}') & wait",
            marker.display()
        ))
        .bounded(Limits::within(wall))
}

#[tokio::test]
async fn a_deadline_ends_the_grandchild_too() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("after-timeout");
    let ended = with_grandchild(&marker, Duration::from_millis(200))
        .run()
        .await;

    assert_eq!(ended.ending, Ending::TimedOut);
    tokio::time::sleep(AFTERWARDS).await;
    assert!(
        !marker.exists(),
        "a descendant wrote its marker after the job reported a completed timeout"
    );
}

#[tokio::test]
async fn a_cancelled_job_takes_its_tree_with_it() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("after-cancel");
    // A bound far away, so nothing here is the deadline doing the work.
    let running = tokio::spawn(with_grandchild(&marker, Duration::from_secs(60)).run());
    tokio::time::sleep(Duration::from_millis(100)).await;
    running.abort();

    tokio::time::sleep(AFTERWARDS).await;
    assert!(
        !marker.exists(),
        "a descendant wrote its marker after its caller was cancelled"
    );
}

#[tokio::test]
async fn a_caller_that_stops_waiting_still_ends_the_job() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("after-outer-timeout");
    let job = with_grandchild(&marker, Duration::from_secs(60));
    let waited = tokio::time::timeout(Duration::from_millis(100), job.run()).await;
    assert!(waited.is_err());

    tokio::time::sleep(AFTERWARDS).await;
    assert!(
        !marker.exists(),
        "a descendant outlived the future that owned it"
    );
}

#[tokio::test]
async fn a_descendant_does_not_outlive_a_child_that_exits() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("after-exit");
    // The shell backgrounds the writer and exits at once, so the job has
    // its result while a descendant is still holding the pipe.
    let ended = Job::new("sh")
        .arg("-c")
        .arg(format!(
            "(sleep 1; printf harmless > '{}') & printf gone",
            marker.display()
        ))
        .bounded(Limits::within(Duration::from_secs(60)))
        .run()
        .await;

    assert_eq!(ended.ending, Ending::Exited(Some(0)));
    assert_eq!(ended.stdout.text, "gone");
    assert!(
        ended.elapsed < Duration::from_secs(2),
        "the job waited on a pipe its descendant was holding: {:?}",
        ended.elapsed
    );
    tokio::time::sleep(AFTERWARDS).await;
    assert!(
        !marker.exists(),
        "a descendant outlived the job that started it"
    );
}

#[tokio::test]
async fn one_job_ending_leaves_the_other_alone() {
    let dir = TempDir::new().unwrap();
    let doomed = dir.path().join("doomed");
    let allowed = dir.path().join("allowed");
    let keeps_going = Job::new("sh")
        .arg("-c")
        .arg(format!(
            "sleep 1; printf harmless > '{}'; printf done",
            allowed.display()
        ))
        .bounded(Limits::within(Duration::from_secs(60)));

    let (cut, kept) = tokio::join!(
        with_grandchild(&doomed, Duration::from_millis(200)).run(),
        keeps_going.run(),
    );

    assert_eq!(cut.ending, Ending::TimedOut);
    assert_eq!(kept.ending, Ending::Exited(Some(0)));
    assert_eq!(kept.stdout.text, "done");
    assert!(allowed.exists(), "the job that kept its bound was killed");
    tokio::time::sleep(AFTERWARDS).await;
    assert!(!doomed.exists(), "the job that lost its bound survived");
}

#[tokio::test]
async fn a_child_that_ignores_the_ask_is_ended_anyway() {
    let started = std::time::Instant::now();
    let ended = Job::new("sh")
        .arg("-c")
        .arg("trap '' TERM; while :; do sleep 0.05; done")
        .bounded(Limits::within(Duration::from_millis(100)))
        .run()
        .await;

    assert_eq!(ended.ending, Ending::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the supervisor waited on a child that never answers SIGTERM"
    );
}

#[tokio::test]
async fn nothing_is_left_unreaped() {
    for _ in 0..4 {
        let ended = Job::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .bounded(Limits::within(Duration::from_millis(50)))
            .run()
            .await;
        assert_eq!(ended.ending, Ending::TimedOut);
    }
    let zombies = zombie_children();
    assert_eq!(zombies, 0, "{zombies} timed-out children were never reaped");
}

#[tokio::test]
async fn a_job_reports_its_own_ending() {
    let ended = Job::new("sh")
        .arg("-c")
        .arg("printf out; printf err >&2; exit 3")
        .bounded(Limits::within(Duration::from_secs(10)))
        .run()
        .await;
    assert_eq!(ended.ending, Ending::Exited(Some(3)));
    assert_eq!(ended.stdout.text, "out");
    assert_eq!(ended.stderr.text, "err");
    assert!(!ended.truncated());
    assert_eq!(ended.bytes(), 6);

    let ended = Job::new("sh")
        .arg("-c")
        .arg("kill -9 $$")
        .bounded(Limits::within(Duration::from_secs(10)))
        .run()
        .await;
    assert_eq!(ended.ending, Ending::Exited(None));
}

#[tokio::test]
async fn a_program_that_will_not_start_is_the_harness() {
    let ended = Job::new("/nonexistent/executor")
        .bounded(Limits::within(Duration::from_secs(10)))
        .run()
        .await;
    assert!(matches!(ended.ending, Ending::Failed(_)));
    assert!(ended.stdout.is_empty());
}

#[tokio::test]
async fn a_job_runs_where_it_was_told_to() {
    let dir = TempDir::new().unwrap();
    let ended = Job::new("sh")
        .arg("-c")
        .arg("pwd")
        .in_directory(dir.path())
        .bounded(Limits::within(Duration::from_secs(10)))
        .run()
        .await;
    let here = std::fs::canonicalize(dir.path()).unwrap();
    assert_eq!(ended.stdout.text.trim(), here.to_string_lossy());
}

/// How many of this process's children are zombies, as `ps` sees them.
fn zombie_children() -> usize {
    let mine = std::process::id().to_string();
    let listed = std::process::Command::new("ps")
        .args(["-A", "-o", "ppid=,stat="])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let parent = fields.next()?;
            let state = fields.next()?;
            Some((parent == mine && state.starts_with('Z')) as usize)
        })
        .sum()
}
