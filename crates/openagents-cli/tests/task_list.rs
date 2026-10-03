//! Task lists stay compact when stdout is a pipe.
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Output},
};

fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .current_dir(home)
        .args(args)
        .output()
        .unwrap()
}
fn success(output: Output) -> Vec<u8> {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    output.stdout
}
#[test]
fn piped_list_defaults_to_lines_and_json_is_explicit() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(success(run(home.path(), &["task", "list"])), b"No tasks.\n");
    assert_eq!(
        serde_json::from_slice::<Value>(&success(run(home.path(), &["--json", "task", "list"])))
            .unwrap(),
        json!([])
    );
    let prompt = "large private prompt ".repeat(1_000);
    let mut expected = Vec::new();
    for (id, title) in [
        ("list-test-one", "First task"),
        ("list-test-two", "Second task"),
    ] {
        let command = json!({
            "schema": "openagents.coder.task-command.v1", "command_id": format!("submit-{id}"),
            "task_id": id, "expected_revision": null,
            "action": {"type": "submit", "intent": {
                "title": title, "prompt": prompt,
                "workspace": {"path": home.path(), "source_revision": null},
                "configuration": {"adapter": "test", "model": null}
            }}
        });
        let file = home.path().join("submit.json");
        std::fs::write(&file, serde_json::to_vec(&command).unwrap()).unwrap();
        success(run(
            home.path(),
            &["task", "submit", "--file", file.to_str().unwrap()],
        ));
        expected.push(
            serde_json::from_slice::<Value>(&success(run(home.path(), &["task", "show", id])))
                .unwrap(),
        );
    }
    let output = String::from_utf8(success(run(home.path(), &["task", "list"]))).unwrap();
    assert_eq!(output.lines().count(), 3, "{output}");
    for title in ["First task", "Second task"] {
        assert!(
            output.lines().skip(1).any(|line| line.contains(title)
                && line.contains("queued")
                && line.contains("not_started")),
            "{output}"
        );
    }
    assert!(output.len() < 400, "{output}");
    assert!(!output.contains("large private prompt"));
    for args in [
        vec!["--json", "task", "list"],
        vec!["task", "list", "--json"],
    ] {
        let records: Vec<Value> =
            serde_json::from_slice(&success(run(home.path(), &args))).unwrap();
        assert_eq!(records.len(), 2);
        for record in &expected {
            assert!(records.contains(record));
        }
    }
}
