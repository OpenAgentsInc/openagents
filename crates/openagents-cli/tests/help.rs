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
    assert!(
        stdout.to_lowercase().contains("usage"),
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

fn run(args: &[&str]) -> (Option<i32>, String, String) {
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env("HOME", home.path())
        .env("TMPDIR", home.path())
        .env("OPENAGENTS_CHAT_HOME", home.path().join("chat"))
        .env("OPENAGENTS_SETTINGS", home.path().join("settings.json"))
        // No gh on PATH: a command that asks gh first fails differently.
        .env("PATH", home.path())
        .env_remove("XDG_RUNTIME_DIR")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn help_group_prints_the_groups_own_help() {
    for group in [
        "chat",
        "wallet",
        "plugin",
        "issue",
        "xp",
        "mcp",
        "completions",
    ] {
        let (code, by_word, _) = run(&["help", group]);
        assert_eq!(code, Some(0), "help {group}");
        let (_, by_flag, _) = run(&[group, "--help"]);
        assert_eq!(by_word, by_flag, "help {group} differs from {group} --help");
    }
}

#[test]
fn each_group_prints_only_its_own_usage() {
    let (_, mcp, _) = run(&["mcp", "--help"]);
    let (_, completions, _) = run(&["completions", "--help"]);
    assert!(mcp.contains("openagents mcp serve") && !mcp.contains("completions SHELL"));
    assert!(
        completions.contains("openagents completions SHELL") && !completions.contains("mcp serve")
    );
    let (_, xp, _) = run(&["xp", "--help"]);
    assert!(xp.starts_with("usage: openagents xp"), "{xp}");
    assert!(
        !xp.contains("gesture"),
        "xp help lists verse commands: {xp}"
    );
}

#[test]
fn an_unknown_issue_command_is_refused_before_gh_runs() {
    let (code, _, stderr) = run(&["issue", "list"]);
    assert_eq!(code, Some(64), "{stderr}");
    assert!(stderr.contains("unknown command `list`"), "{stderr}");
    assert!(!stderr.contains("gh"), "{stderr}");
}
