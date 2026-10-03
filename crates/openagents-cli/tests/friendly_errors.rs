//! Missing-resource diagnostics must not expose internal storage paths.
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], json: bool) -> Output {
    let home = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
    if json {
        command.arg("--json");
    }
    command
        .args(args)
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("VERSE_HOME", home.path().join("verse"))
        .env("LABOR_HOME", home.path().join("labor"))
        .env("SOV_HOME", home.path().join("sov"))
        .env("OPENAGENTS_GYM_HOME", home.path().join("gym"))
        .env("OPENAGENTS_SETTINGS", home.path().join("settings.json"))
        .env_remove("OPENAGENTS_PROFILE")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn missing_resources_have_actionable_errors() {
    let mut cases = vec![
        (
            vec!["gym", "status"],
            "Not connected to a gym yet. Run `openagents gym connect --code CODE`.",
        ),
        (
            vec!["gym", "observe"],
            "Not connected to a gym yet. Run `openagents gym connect --code CODE`.",
        ),
        (
            vec!["sov", "profile", "validate", "/nonexistent"],
            "No file at /nonexistent.",
        ),
        (
            vec!["sov", "spawn", "nobody", "--seconds", "5"],
            "No profile draft named nobody.",
        ),
    ];
    if cfg!(unix) {
        cases.push((
            vec!["labor", "check", "nobook"],
            "No book named nobook. See `openagents labor list`.",
        ));
    }
    for (args, expected) in cases {
        for json in [false, true] {
            let output = run(&args, json);
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.status.code(), Some(1), "{args:?}: {text}");
            assert!(text.contains(expected), "{args:?}: {text}");
            for forbidden in [
                "os error",
                ".openagents/",
                "setup.json",
                "default.connection",
                "nobody.profile.json",
            ] {
                assert!(!text.contains(forbidden), "{args:?}: {text}");
            }
            if json {
                let _: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            }
        }
    }
}

#[test]
fn issue_number_is_checked_before_repository_lookup() {
    for subcommand in ["claim", "release", "done", "status"] {
        let output = run(&["issue", subcommand], false);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "{stderr}");
        assert!(
            stderr.starts_with("openagents issue: N (issue number) is required"),
            "{stderr}"
        );
        assert!(!stderr.contains("checkout of the repository"), "{stderr}");
    }
}

#[test]
fn unknown_commands_are_short_and_suggest_nearby_commands() {
    for (word, suggestion) in [
        ("models", Some("settings")),
        ("gmy", Some("gym")),
        ("zzzzzzzzzz", None),
    ] {
        let output = run(&[word], false);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64));
        assert!(
            stderr.contains(&format!("unknown command `{word}`")),
            "{stderr}"
        );
        assert!(stderr.contains("openagents --help"), "{stderr}");
        assert_eq!(stderr.lines().count(), 2, "{stderr}");
        if let Some(name) = suggestion {
            assert!(
                stderr.contains(&format!("did you mean `openagents {name}`?")),
                "{stderr}"
            );
        } else {
            assert!(!stderr.contains("did you mean"), "{stderr}");
        }
    }
}
