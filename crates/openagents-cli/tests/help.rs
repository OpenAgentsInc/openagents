//! Process-level help regression tests with an isolated home.

use std::process::{Command, Stdio};

fn assert_help(command: &[&str], flag: &str) {
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(command)
        .arg(flag)
        .env("HOME", home.path())
        .env("TMPDIR", home.path())
        .env("OPENAGENTS_CHAT_HOME", home.path().join("chat"))
        .env("OPENAGENTS_SETTINGS", home.path().join("settings.json"))
        .env_remove("XDG_RUNTIME_DIR")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{command:?} {flag}: {stdout}\n{stderr}"
    );
    // Delegated host/task usage uses the coder name; pair's usage has no prefix.
    assert!(
        stdout.to_lowercase().contains("usage") || stdout.starts_with("openagents pair "),
        "{command:?} {flag}: {stdout}"
    );
    assert!(stderr.is_empty(), "{command:?} {flag}: {stderr}");
}

#[test]
fn every_top_level_command_accepts_help() {
    let mut commands = vec![
        "host",
        "pair",
        "computer",
        "computers",
        "study",
        "reach",
        "session",
        "sessions",
        "chat",
        "terminal",
        "task",
        "settings",
        "verse",
        "xp",
        "zone",
        "sov",
        "eval",
        "gym",
        "key",
        "kb",
        "relay",
        "playtest",
        "plugin",
        "plugins",
        "ext",
        "cap",
        "prg",
        "discover",
        "mcp",
        "completions",
        "doctor",
        "version",
    ];
    if cfg!(unix) {
        commands.extend(["connect", "service", "ssh", "labor", "wallet", "x402"]);
    }
    for command in commands {
        for flag in ["--help", "-h", "help"] {
            assert_help(&[command], flag);
        }
    }
    assert_help(&[], "--help");
    assert_help(&[], "-h");
    assert_help(&[], "help");
}

#[test]
fn subcommand_help_exits_successfully() {
    for command in [
        vec!["chat", "work"],
        vec!["chat", "send"],
        vec!["settings", "show"],
        vec!["plugin", "list"],
        vec!["plugin", "test", "run"],
        vec!["plugin", "defaults", "show"],
        vec!["plugin", "run"],
        vec!["terminal", "--thread", "invalid"],
        vec!["task", "list"],
        vec!["host", "serve"],
        vec!["host", "spend", "list"],
    ] {
        for flag in ["--help", "-h"] {
            assert_help(&command, flag);
        }
    }
}
