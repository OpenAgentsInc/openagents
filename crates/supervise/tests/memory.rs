//! A job past its memory cap is killed alone and reported as such.
//!
//! The allocating job is this test binary run again with one test selected
//! and a variable telling that test how much to take, so the cases need no
//! allocator on the host beyond the one under test.

use std::time::Duration;

use supervise::{Ending, Enforcement, Job, Limits};

const MIB: u64 = 1024 * 1024;
const ALLOCATE: &str = "SUPERVISE_TEST_ALLOCATE_MIB";

/// Takes the number of mebibytes the variable names, touching every page so
/// the memory is in use rather than reserved. Does nothing when the variable
/// is unset, which is every run but the ones the cases below start.
#[test]
fn allocate_when_asked() {
    let Ok(mib) = std::env::var(ALLOCATE) else {
        return;
    };
    let mib: usize = mib.parse().unwrap();
    let mut held = Vec::with_capacity(mib);
    for _ in 0..mib {
        let mut chunk = vec![0u8; 1 << 20];
        for page in chunk.iter_mut().step_by(4096) {
            *page = 1;
        }
        held.push(chunk);
    }
    println!("held {} MiB", held.len());
}

fn allocating(mib: u64, cap: u64) -> Job {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "allocate_when_asked", "--nocapture"])
        .env(ALLOCATE, mib.to_string());
    Job::from_command(command).bounded(Limits::within(Duration::from_secs(60)).memory(Some(cap)))
}

#[tokio::test]
async fn a_job_past_its_cap_is_killed_and_reported() {
    let ended = allocating(1024, 64 * MIB).run().await;
    let memory = ended.memory.clone().expect("the job ran under a cap");
    assert_eq!(memory.max, 64 * MIB);
    assert!(!ended.ending.success(), "{:?}", ended.ending);
    assert!(!ended.stdout.text.contains("held 1024 MiB"));
    match memory.enforcement {
        // The kernel killed inside the scope, and the report says so.
        Enforcement::Scope(_) => {
            assert!(ended.over_memory(), "{memory:?} {:?}", ended.ending);
        }
        // Without a scope the allocation fails and the process aborts. It
        // is stopped, and nothing can tell it apart from a crash.
        Enforcement::DataLimit => assert!(!ended.over_memory()),
    }
}

#[tokio::test]
async fn a_job_under_its_cap_runs_and_is_not_reported() {
    let ended = allocating(16, 256 * MIB).run().await;
    assert_eq!(
        ended.ending,
        Ending::Exited(Some(0)),
        "{}",
        ended.stderr.text
    );
    assert!(ended.stdout.text.contains("held 16 MiB"));
    assert!(!ended.over_memory());
    assert_eq!(ended.memory.map(|memory| memory.max), Some(256 * MIB));
}

#[tokio::test]
async fn a_crash_under_a_cap_is_not_a_memory_ending() {
    let ended = Job::new("sh")
        .args(["-c", "kill -SEGV $$"])
        .bounded(Limits::within(Duration::from_secs(10)).memory(Some(256 * MIB)))
        .run()
        .await;
    assert_eq!(ended.ending, Ending::Exited(None));
    assert!(!ended.over_memory());
    assert!(ended.memory.is_some());
}

#[tokio::test]
async fn what_the_job_starts_first_is_already_inside_the_scope() {
    let ended = Job::new("sh")
        .args(["-c", "sh -c 'cat /proc/self/cgroup'"])
        .bounded(Limits::within(Duration::from_secs(10)).memory(Some(256 * MIB)))
        .run()
        .await;
    assert!(ended.ending.success(), "{:?}", ended.ending);
    if let Some(Enforcement::Scope(unit)) = ended.memory.map(|memory| memory.enforcement) {
        assert!(
            ended.stdout.text.trim_end().ends_with(&format!("/{unit}")),
            "{} is not in {unit}",
            ended.stdout.text
        );
    }
}

#[tokio::test]
async fn a_job_without_a_cap_reports_none() {
    let ended = Job::new("true")
        .bounded(Limits::within(Duration::from_secs(5)).memory(None))
        .run()
        .await;
    assert!(ended.ending.success());
    assert!(ended.memory.is_none());
}

#[tokio::test]
async fn a_watched_job_past_its_cap_is_reported_when_it_stops() {
    let live = allocating(1024, 64 * MIB)
        .start(supervise::Input::Null)
        .unwrap();
    let stopped = live.wait().await;
    let memory = stopped.memory.clone().expect("the job ran under a cap");
    assert!(!stopped.ending.success());
    if let Enforcement::Scope(_) = memory.enforcement {
        assert!(stopped.over_memory(), "{memory:?}");
    }
}

#[tokio::test]
async fn a_process_that_left_the_group_ends_with_the_scope() {
    if std::process::Command::new("setsid")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let ended = Job::new("sh")
        .args([
            "-c",
            "setsid sh -c 'echo $$ > pid; exec sleep 30' & sleep 0.2",
        ])
        .in_directory(directory.path())
        .bounded(Limits::within(Duration::from_secs(10)).memory(Some(256 * MIB)))
        .run()
        .await;
    assert!(ended.ending.success(), "{:?}", ended.ending);
    let Some(Enforcement::Scope(_)) = ended.memory.map(|memory| memory.enforcement) else {
        return;
    };
    let pid: u32 = std::fs::read_to_string(directory.path().join("pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    // The escaped process is killed with the scope; give init a moment to
    // reap it.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while supervise::process_running(pid) && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(!supervise::process_running(pid), "{pid} outlived its scope");
}
