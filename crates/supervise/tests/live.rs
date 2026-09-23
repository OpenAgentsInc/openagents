//! A watched job: output as it arrives, input while it runs, and a stop
//! that owns the whole tree.

use std::time::{Duration, Instant};

use supervise::{Ending, Input, Job, Limits};
use tempfile::TempDir;

/// Long enough for a killed tree to have written its marker if it was
/// still alive to write one.
const AFTERWARDS: Duration = Duration::from_millis(1500);

#[tokio::test]
async fn output_arrives_before_the_job_ends() {
    let live = Job::new("sh")
        .arg("-c")
        .arg("echo first; sleep 1; echo second")
        .bounded(Limits::within(Duration::from_secs(10)))
        .start(Input::Null)
        .unwrap();
    let started = Instant::now();
    let mut seen = Vec::new();
    while started.elapsed() < Duration::from_secs(5) {
        let delivery = live.take();
        seen.extend(delivery.bytes);
        if !seen.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(seen, b"first\n");
    assert!(!live.finished(), "the first line arrived while the job ran");
    let stopped = live.wait().await;
    assert_eq!(stopped.ending, Ending::Exited(Some(0)));
    assert_eq!(stopped.rest.offset, 6);
    assert_eq!(stopped.rest.bytes, b"second\n");
    assert_eq!(stopped.stdout_bytes, 13);
    assert!(stopped.group_clear);
}

#[tokio::test]
async fn input_written_while_it_runs_is_read() {
    let mut live = Job::new("sh")
        .arg("-c")
        .arg("while read -r line; do echo \"heard $line\"; done")
        .bounded(Limits::within(Duration::from_secs(10)))
        .start(Input::Piped)
        .unwrap();
    live.send(b"one\n").await.unwrap();
    live.send(b"two\n").await.unwrap();
    live.close_input();
    assert!(!live.input_open());
    let stopped = live.wait().await;
    assert_eq!(stopped.ending, Ending::Exited(Some(0)));
    assert_eq!(stopped.rest.bytes, b"heard one\nheard two\n");
}

#[tokio::test]
async fn a_requested_stop_ends_the_tree_and_acknowledges_it() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("after-stop");
    let live = Job::new("sh")
        .arg("-c")
        .arg(format!(
            "(sleep 1; printf harmless > '{}') & echo started; wait",
            marker.display()
        ))
        .bounded(Limits::within(Duration::from_secs(60)))
        .start(Input::Null)
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let stopped = live.stop().await;
    assert!(stopped.requested);
    assert!(
        stopped.group_clear,
        "the acknowledgement says the group is empty"
    );
    assert_ne!(stopped.ending, Ending::TimedOut);
    assert!(stopped.elapsed < Duration::from_secs(5));
    tokio::time::sleep(AFTERWARDS).await;
    assert!(
        !marker.exists(),
        "a descendant outlived an acknowledged stop"
    );
}

#[tokio::test]
async fn a_dropped_handle_still_ends_the_tree() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("after-drop");
    let live = Job::new("sh")
        .arg("-c")
        .arg(format!(
            "(sleep 1; printf harmless > '{}') & wait",
            marker.display()
        ))
        .bounded(Limits::within(Duration::from_secs(60)))
        .start(Input::Null)
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    drop(live);
    tokio::time::sleep(AFTERWARDS).await;
    assert!(!marker.exists(), "a descendant outlived its dropped handle");
}

#[tokio::test]
async fn the_deadline_is_a_timeout() {
    let live = Job::new("sh")
        .arg("-c")
        .arg("sleep 30")
        .bounded(Limits::within(Duration::from_millis(200)))
        .start(Input::Null)
        .unwrap();
    let stopped = live.wait().await;
    assert_eq!(stopped.ending, Ending::TimedOut);
    assert!(!stopped.requested);
    assert!(stopped.group_clear);
}

#[tokio::test]
async fn output_nobody_takes_is_dropped_as_a_gap_and_counted() {
    let live = Job::new("sh")
        .arg("-c")
        .arg("head -c 100000 /dev/zero")
        .bounded(Limits::within(Duration::from_secs(10)).keeping(1000))
        .start(Input::Null)
        .unwrap();
    let stopped = live.wait().await;
    assert_eq!(stopped.stdout_bytes, 100_000);
    assert_eq!(stopped.rest.bytes.len(), 1000);
    let dropped: u64 = stopped.rest.gaps.iter().map(|gap| gap.bytes).sum();
    assert_eq!(dropped, 99_000);
    assert_eq!(stopped.rest.gaps[0].offset, 1000);
}
