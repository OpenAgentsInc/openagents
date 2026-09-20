//! The blocking half owns a tree the same way the asynchronous one does.

use std::process::{Command, Stdio};
use std::time::Duration;

use supervise::{Ending, blocking};
use tempfile::TempDir;

/// A shell that starts a background writer and waits for it.
fn with_grandchild(marker: &std::path::Path) -> Command {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(format!(
            "(sleep 1; printf harmless > '{}') & wait",
            marker.display()
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    blocking::own_group(&mut command);
    command
}

#[test]
fn a_deadline_ends_the_grandchild_too() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("after-timeout");
    let mut child = with_grandchild(&marker).spawn().unwrap();

    assert_eq!(
        blocking::wait(&mut child, Duration::from_millis(200)),
        Ending::TimedOut
    );
    std::thread::sleep(Duration::from_millis(2000));
    assert!(
        !marker.exists(),
        "a descendant wrote its marker after the wait reported a timeout"
    );
}

#[test]
fn a_descendant_does_not_outlive_a_child_that_exits() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("after-exit");
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(format!(
            "(sleep 1; printf harmless > '{}') &",
            marker.display()
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    blocking::own_group(&mut command);
    let mut child = command.spawn().unwrap();

    assert_eq!(
        blocking::wait(&mut child, Duration::from_secs(30)),
        Ending::Exited(Some(0))
    );
    std::thread::sleep(Duration::from_millis(2000));
    assert!(
        !marker.exists(),
        "a descendant outlived the job that started it"
    );
}

#[test]
fn an_ordinary_command_answers() {
    let mut command = Command::new("sh");
    command
        .args(["-c", "exit 2"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    blocking::own_group(&mut command);
    let mut child = command.spawn().unwrap();
    assert_eq!(
        blocking::wait(&mut child, Duration::from_secs(30)),
        Ending::Exited(Some(2))
    );
}

#[test]
fn a_child_in_this_process_s_group_is_refused() {
    // Without `own_group` the child shares this process's group, and
    // terminating that group would terminate the test.
    let mut child = Command::new("sh")
        .args(["-c", "sleep 30"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert!(matches!(
        blocking::wait(&mut child, Duration::from_millis(50)),
        Ending::Failed(_)
    ));
}
