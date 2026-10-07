//! `openagents lease` end to end, under a temporary lease root and HOME.
#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

fn openagents(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("OPENAGENTS_LEASE_ROOT", home.join("leases"))
        .env("OPENAGENTS_SESSION", "lease-test")
        .env("OPENAGENTS_BUILD_LEASES", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn lease_runs_the_command_under_the_lease_and_leaves_a_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let receipt = dir.path().join("receipt.json");
    let output = openagents(
        dir.path(),
        &[
            "lease",
            "quiet",
            "--receipt",
            receipt.to_str().unwrap(),
            "--json",
            "--",
            "sh",
            "-c",
            "echo \"$OPENAGENTS_LEASES $OPENAGENTS_SESSION\"; echo --json; exit 7",
        ],
    );
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut lines = stdout.lines();
    assert_eq!(lines.next(), Some("quiet lease-test"));
    // `--json` after `--` belongs to the command.
    assert_eq!(lines.next(), Some("--json"));
    let printed: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    assert_eq!(printed["resource"], "quiet");
    assert_eq!(printed["exit"], 7);
    assert_eq!(printed["held_whole_run"], true);
    assert_eq!(printed["holder"]["command"], "sh");
    let written: Value = serde_json::from_slice(&std::fs::read(&receipt).unwrap()).unwrap();
    assert_eq!(written, printed);
    let id = written["id"].as_str().unwrap();
    assert!(
        dir.path()
            .join(format!("leases/receipts/{id}.json"))
            .exists()
    );
}

#[test]
fn nesting_passes_through_and_no_wait_refuses_a_busy_resource() {
    let dir = tempfile::tempdir().unwrap();
    let me = env!("CARGO_BIN_EXE_openagents");
    // The only build slot is held by the outer command; the inner one runs
    // inside it without waiting, and a plain --no-wait request is refused.
    let script = format!(
        "{me} lease build --no-wait -- true && \
         env -u OPENAGENTS_LEASES {me} lease build --no-wait -- true; echo inner=$?"
    );
    let output = openagents(dir.path(), &["lease", "build", "--", "sh", "-c", &script]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("inner=1"), "{stdout}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("build is not free"), "{stderr}");

    let listed = openagents(dir.path(), &["lease", "list", "--json"]);
    assert!(listed.status.success(), "{listed:?}");
    let value: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(value["leases"], Value::Array(Vec::new()));
    assert_eq!(value["limits"]["build"], 1);
}

#[test]
fn the_screen_is_refused_without_a_grant_and_a_grant_needs_a_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let output = openagents(dir.path(), &["lease", "screen", "--", "true"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("grant"),
        "{output:?}"
    );
    let output = openagents(dir.path(), &["lease", "grant", "screen", "--for", "10m"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not a terminal"),
        "{output:?}"
    );
    let output = openagents(dir.path(), &["lease", "revoke", "screen", "--json"]);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["revoked"], false);
    let output = openagents(dir.path(), &["lease", "grant", "gpu"]);
    assert_eq!(output.status.code(), Some(64));
}
