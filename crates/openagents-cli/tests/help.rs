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
    let (code, _, stderr) = run(&["issue", "frobnicate"]);
    assert_eq!(code, Some(64), "{stderr}");
    assert!(stderr.contains("unknown command `frobnicate`"), "{stderr}");
    assert!(!stderr.contains("gh"), "{stderr}");
}

#[test]
#[cfg(unix)]
fn money_subcommands_show_their_own_usage() {
    for (command, usage, detail) in [
        (
            vec!["x402", "node"],
            "usage: openagents x402 node COMMAND",
            "init [--network NET]",
        ),
        (
            vec!["x402", "fetch"],
            "usage: openagents x402 fetch URL",
            "--pay-with wallet|node|phone",
        ),
        (
            vec!["wallet", "send"],
            "usage: openagents wallet send TO",
            "--yes",
        ),
    ] {
        for flag in ["--help", "-h"] {
            let mut args = command.clone();
            args.push(flag);
            let (code, text, stderr) = run(&args);
            assert_eq!(code, Some(0), "{stderr}");
            assert!(text.starts_with(usage), "{text}");
            assert!(text.contains(detail), "{text}");
        }
        let mut args = vec!["help"];
        args.extend(command.clone());
        let (code, text, stderr) = run(&args);
        assert_eq!(code, Some(0), "{stderr}");
        assert!(text.starts_with(usage), "{text}");
    }
}

#[test]
fn reported_subcommands_show_usage_flags_and_an_example_before_parsing() {
    for (group, command, syntax, flag, sibling) in [
        ("background", "show", "show ID", "--tasks DIR", "resume ID"),
        (
            "plugin",
            "install",
            "install DIR | NAME | ID",
            "--blossom URL",
            "enable PLUGIN",
        ),
        (
            "chat",
            "send",
            "send MESSAGE",
            "--no-run",
            "follow --thread ID",
        ),
        (
            "eval",
            "run",
            "run --door NAME=URL",
            "--timeout SECONDS",
            "report --store FILE",
        ),
    ] {
        for help in ["--help", "-h"] {
            // Help wins over missing values and invalid options, without opening a service.
            let (code, text, stderr) = run(&[group, command, "--invalid", help]);
            assert_eq!(code, Some(0), "{stderr}");
            assert!(stderr.is_empty(), "{stderr}");
            assert!(
                text.starts_with(&format!("usage: openagents {group} {syntax}")),
                "{text}"
            );
            assert!(text.contains(flag), "{text}");
            assert!(
                text.contains(&format!("Example:\n  openagents {group} {command} ")),
                "{text}"
            );
            assert!(!text.contains(sibling), "{text}");
        }
        let (code, by_word, stderr) = run(&["help", group, command]);
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(by_word, run(&[group, command, "--help"]).1);
    }
    for alias in ["plugins", "ext"] {
        assert_eq!(
            run(&[alias, "install", "--help"]).1,
            run(&["plugin", "install", "--help"]).1
        );
    }
}

#[test]
fn gce_work_help_explains_recovery_and_on_demand_fallback() {
    let (code, text, stderr) = run(&["chat", "work", "--help"]);
    assert_eq!(code, Some(0), "{stderr}");
    for detail in [
        "lost host resumes",
        "progress or stranded branch",
        "from scratch",
        "fall back to on demand",
        "Preemptions",
        "run record",
    ] {
        assert!(text.contains(detail), "Missing {detail}: {text}");
    }
}
