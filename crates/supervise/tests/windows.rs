//! The same ownership contract on Windows, where a job's tree is a job
//! object: a descendant does not outlive its job's deadline, its exit, its
//! caller's cancellation, a watched job's stop, or a blocking wait.
//!
//! Every process here is this test binary run again with one test selected
//! and a role to play, so the cases need no shell. A grandchild writes a
//! harmless marker a second after it starts; a marker that exists afterwards
//! is a process that outlived the bound it was given.

#![cfg(windows)]

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use supervise::{Ending, Input, Job, Limits};

const ROLE: &str = "SUPERVISE_TEST_ROLE";
const MARKER: &str = "SUPERVISE_TEST_MARKER";

/// Long enough for a grandchild that was still alive to have written its
/// marker.
const AFTERWARDS: Duration = Duration::from_millis(2500);

/// This binary, selecting only [`play_a_role_when_asked`], as `role`.
fn this(role: &str, marker: &Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "play_a_role_when_asked", "--nocapture"])
        .env(ROLE, role)
        .env(MARKER, marker);
    command
}

/// Plays the role the variable names. Does nothing when it is unset, which
/// is every run but the ones the cases below start.
#[test]
fn play_a_role_when_asked() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    let marker = std::path::PathBuf::from(std::env::var_os(MARKER).unwrap());
    match role.as_str() {
        // Starts a grandchild the plain way, with no job flags, then waits
        // or exits.
        "parent" | "parent-exits" => {
            // Left running on purpose: outliving this process is what the
            // grandchild tries to do.
            #[allow(clippy::zombie_processes)]
            let _grandchild = this("grandchild", &marker).spawn().unwrap();
            println!("started");
            if role == "parent" {
                std::thread::sleep(Duration::from_secs(60));
            }
        }
        "grandchild" => {
            std::thread::sleep(Duration::from_secs(1));
            std::fs::write(&marker, "harmless").unwrap();
        }
        "echo" => {
            println!("hello");
            eprintln!("to stderr");
        }
        other => panic!("no role {other}"),
    }
}

fn job(role: &str, marker: &Path, wall: Duration) -> Job {
    Job::from_command(this(role, marker)).bounded(Limits::within(wall))
}

#[tokio::test]
async fn output_is_captured_and_the_exit_code_kept() {
    let dir = tempfile::tempdir().unwrap();
    let ended = job("echo", &dir.path().join("unused"), Duration::from_secs(20))
        .run()
        .await;
    assert_eq!(
        ended.ending,
        Ending::Exited(Some(0)),
        "{}",
        ended.stderr.text
    );
    assert!(ended.stdout.text.contains("hello"), "{}", ended.stdout.text);
    assert!(ended.stderr.text.contains("to stderr"));
    let memory = ended.memory.expect("a job has the default cap");
    assert_eq!(memory.enforcement, supervise::Enforcement::JobObject);
}

#[tokio::test]
async fn a_deadline_ends_the_grandchild_too() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("after-timeout");
    let ended = job("parent", &marker, Duration::from_millis(500))
        .run()
        .await;
    assert_eq!(ended.ending, Ending::TimedOut);
    tokio::time::sleep(AFTERWARDS).await;
    assert!(!marker.exists(), "a descendant outlived the deadline");
}

#[tokio::test]
async fn a_descendant_does_not_outlive_a_child_that_exits() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("after-exit");
    let ended = job("parent-exits", &marker, Duration::from_secs(20))
        .run()
        .await;
    assert_eq!(
        ended.ending,
        Ending::Exited(Some(0)),
        "{}",
        ended.stderr.text
    );
    tokio::time::sleep(AFTERWARDS).await;
    assert!(
        !marker.exists(),
        "a descendant outlived the job that exited"
    );
}

#[tokio::test]
async fn a_cancelled_job_takes_its_tree_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("after-cancel");
    let running = tokio::spawn(job("parent", &marker, Duration::from_secs(60)).run());
    tokio::time::sleep(Duration::from_millis(300)).await;
    running.abort();
    tokio::time::sleep(AFTERWARDS).await;
    assert!(
        !marker.exists(),
        "a descendant outlived its cancelled caller"
    );
}

#[tokio::test]
async fn a_stopped_live_job_leaves_its_job_object_empty() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("after-stop");
    let live = job("parent", &marker, Duration::from_secs(60))
        .start(Input::Null)
        .unwrap();
    assert!(live.pid().is_some());
    tokio::time::sleep(Duration::from_millis(300)).await;
    let stopped = live.stop().await;
    assert!(stopped.requested);
    assert!(stopped.group_clear, "{stopped:?}");
    assert_eq!(stopped.group, stopped.group.filter(|id| *id > 0));
    tokio::time::sleep(AFTERWARDS).await;
    assert!(!marker.exists(), "a descendant outlived the stop");
}

#[test]
fn a_blocking_wait_ends_the_tree_at_its_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("after-blocking");
    let mut command = this("parent", &marker);
    supervise::blocking::own_group(&mut command);
    let mut child = command.spawn().unwrap();
    // The job object is made as the wait starts; the parent starts its
    // grandchild after that.
    let ending = supervise::blocking::wait(&mut child, Duration::from_millis(500));
    assert_eq!(ending, Ending::TimedOut);
    std::thread::sleep(AFTERWARDS);
    assert!(!marker.exists(), "a descendant outlived the blocking wait");
}
