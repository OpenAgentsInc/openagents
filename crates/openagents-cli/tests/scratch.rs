//! `openagents scratch` end to end, under a temporary HOME.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

fn openagents(home: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home)
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn scratch_makes_the_sessions_private_directory_under_the_home() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().canonicalize().unwrap();
    let output = openagents(&home, &[("OPENAGENTS_SESSION", "codex:4242")], &["scratch"]);
    assert!(output.status.success(), "{output:?}");
    let printed = String::from_utf8(output.stdout).unwrap();
    let path = home.join(".openagents/scratch/codex-4242");
    assert_eq!(printed.trim(), path.display().to_string());
    assert!(path.is_dir());
    assert_eq!(mode(&path), 0o700);
    assert_eq!(mode(&home.join(".openagents/scratch")), 0o700);
    assert_eq!(
        std::fs::read_to_string(path.join(".session")).unwrap(),
        "codex:4242"
    );

    // --session wins, and an unsafe name becomes a safe directory name.
    let output = openagents(
        &home,
        &[("OPENAGENTS_SESSION", "codex:4242")],
        &["scratch", "--session", "../seat 3", "--json"],
    );
    assert!(output.status.success(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["session"], "../seat 3");
    assert_eq!(value["from"], "--session");
    let path = home.join(".openagents/scratch/_seat_3");
    assert_eq!(value["path"], path.display().to_string());
    assert!(path.is_dir());
}

#[test]
fn scratch_keeps_a_directory_it_was_given_and_honours_the_root_variable() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().canonicalize().unwrap();
    let given = home.join("given/scratch-dir");
    let output = openagents(
        &home,
        &[("OPENAGENTS_SCRATCH", given.to_str().unwrap())],
        &["scratch", "--json"],
    );
    assert!(output.status.success(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["path"], given.display().to_string());
    assert_eq!(value["from"], "OPENAGENTS_SCRATCH");
    assert_eq!(mode(&given), 0o700);

    let root = home.join("elsewhere");
    let output = openagents(
        &home,
        &[
            ("OPENAGENTS_SCRATCH_ROOT", root.to_str().unwrap()),
            ("OPENAGENTS_SESSION", "studio-seat-3"),
        ],
        &["scratch"],
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        root.join("studio-seat-3").display().to_string()
    );
    assert!(!home.join(".openagents/scratch").exists());

    let output = openagents(&home, &[], &["scratch", "extra"]);
    assert_eq!(output.status.code(), Some(64), "{output:?}");
}

#[test]
fn a_lease_gives_its_command_the_sessions_scratch() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().canonicalize().unwrap();
    let leases = home.join("leases");
    let output = openagents(
        &home,
        &[
            ("OPENAGENTS_LEASE_ROOT", leases.to_str().unwrap()),
            ("OPENAGENTS_SESSION", "lease-test"),
        ],
        &[
            "lease",
            "gpu",
            "--",
            "sh",
            "-c",
            "echo \"$OPENAGENTS_SCRATCH\"",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let scratch = home.join("scratch/lease-test");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        scratch.display().to_string()
    );
    assert!(scratch.is_dir());
}
