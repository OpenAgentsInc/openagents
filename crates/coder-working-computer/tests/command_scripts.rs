//! The Boat command wrapper scripts, run with the local `sh`. A separate test
//! binary, so its child processes never share a store lease descriptor.

use coder_working_computer::boat::*;
use coder_working_computer::provider::{CommandCursor, CommandProgress, CommandSpec};
use std::collections::BTreeMap;
use std::process::Command;
use std::time::Duration;

fn spec(id: &str, command: &str, names: &[&str]) -> CommandSpec {
    CommandSpec {
        id: id.into(),
        command: command.into(),
        cwd: ".".into(),
        credential_names: names.iter().map(|n| n.to_string()).collect(),
        env: BTreeMap::from([("OA_MARK".to_string(), "x y".to_string())]),
        timeout_seconds: 30,
        digest: "d".repeat(64),
    }
}
fn sh(script: &str, env: &[(&str, &str)]) -> (i32, String) {
    let out = Command::new("sh")
        .arg("-c")
        .arg(script)
        .envs(env.iter().copied())
        .output()
        .expect("sh runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
    )
}

#[test]
fn wrapper_runs_once_hides_unnamed_credentials_and_reads_by_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("cmds");
    let root = root.to_str().unwrap();
    let work = dir.path().to_str().unwrap();
    let creds = [
        ("NAMED_TOKEN", "named-value-123"),
        ("OTHER_TOKEN", "other-value-456"),
    ];
    let s = spec(
        "c1",
        "echo \"n=${NAMED_TOKEN:-} o=${OTHER_TOKEN:-} m=$OA_MARK\"; echo oops >&2; echo run >> runs; exit 3",
        &["NAMED_TOKEN"],
    );
    let script = command_script(root, work, &s, &["OTHER_TOKEN"]);
    assert_eq!(sh(&script, &creds).0, 3);
    // The same identity never runs again.
    assert_eq!(sh(&script, &creds).0, ALREADY_CLAIMED);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("runs")).unwrap(),
        "run\n"
    );

    let (_, text) = sh(
        &read_script(root, "c1", CommandCursor::default(), 1 << 20),
        &[],
    );
    let read = parse_read(&text).unwrap();
    assert_eq!(read.progress, CommandProgress::Exited { code: 3 });
    assert_eq!(read.digest.as_deref(), Some(s.digest.as_str()));
    assert_eq!(
        String::from_utf8(read.stdout).unwrap(),
        "n=named-value-123 o= m=x y\n"
    );
    assert_eq!(read.stderr, b"oops\n");

    let cursor = CommandCursor {
        stdout: 2,
        stderr: 4,
    };
    let (_, text) = sh(&read_script(root, "c1", cursor, 3), &[]);
    let read = parse_read(&text).unwrap();
    assert_eq!(read.stdout, b"nam");
    assert_eq!(read.stderr, b"\n");

    let (_, text) = sh(
        &read_script(root, "never", CommandCursor::default(), 10),
        &[],
    );
    assert_eq!(parse_read(&text).unwrap().progress, CommandProgress::Absent);
    assert_eq!(sh(&stop_script(root, "c1"), &[]).1.trim(), "exited");
}

#[test]
fn a_stopped_command_records_its_signal_exit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("cmds");
    let root = root.to_str().unwrap().to_owned();
    let work = dir.path().to_str().unwrap().to_owned();
    let s = spec("slow", "echo begun; sleep 30", &[]);
    let script = command_script(&root, &work, &s, &[]);
    let child = Command::new("sh").arg("-c").arg(&script).spawn().unwrap();
    let read = || {
        let script = read_script(&root, "slow", CommandCursor::default(), 100);
        parse_read(&sh(&script, &[]).1).unwrap()
    };
    for _ in 0..50 {
        if read().stdout == b"begun\n" {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(read().progress, CommandProgress::Running);
    assert_eq!(sh(&stop_script(&root, "slow"), &[]).1.trim(), "stopped");
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(143));
    assert_eq!(read().progress, CommandProgress::Exited { code: 143 });
}
