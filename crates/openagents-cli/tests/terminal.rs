//! `openagents terminal` and bare `openagents`, run as a process with a
//! temporary HOME and no terminal, so no screen opens and no real identity,
//! store, or host is touched.

use std::path::Path;
use std::process::{Command, Output, Stdio};

fn openagents(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env("HOME", home)
        .env("TMPDIR", home)
        .env_remove("OPENAGENTS_CHAT_HOME")
        .env_remove("OPENAGENTS_SETTINGS")
        .env_remove("XDG_RUNTIME_DIR")
        // Pipes, not a terminal.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("openagents runs")
}

#[test]
fn bare_openagents_without_a_terminal_prints_usage() {
    let home = tempfile::tempdir().unwrap();
    let output = openagents(home.path(), &[]);
    assert_eq!(output.status.code(), Some(64));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("usage: openagents [--json] COMMAND"),
        "{stderr}"
    );
    assert!(
        stderr.contains("  terminal     OpenAgents Terminal"),
        "{stderr}"
    );
}

#[test]
fn terminal_help_names_its_options() {
    let home = tempfile::tempdir().unwrap();
    let output = openagents(home.path(), &["terminal", "--help"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("usage: openagents terminal"), "{stdout}");
    assert!(stdout.contains("--thread"), "{stdout}");
    assert!(stdout.contains("--scratch"), "{stdout}");
}

#[test]
fn a_bad_thread_is_a_usage_error() {
    let home = tempfile::tempdir().unwrap();
    let output = openagents(home.path(), &["terminal", "--thread", "not-a-thread"]);
    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("32 lowercase hex"), "{stderr}");
    let output = openagents(
        home.path(),
        &[
            "terminal",
            "--thread",
            "0123456789abcdef0123456789abcdef",
            "--continue",
        ],
    );
    assert_eq!(output.status.code(), Some(64));
    let output = openagents(home.path(), &["terminal", "--new", "--continue"]);
    assert_eq!(output.status.code(), Some(64));
    let output = openagents(home.path(), &["terminal", "--colour"]);
    assert_eq!(output.status.code(), Some(64));
}
