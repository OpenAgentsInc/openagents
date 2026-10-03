use std::{
    path::Path,
    process::{Command, Output},
};

fn run(home: &Path, args: &[&str], directory: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
    command
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .env("OPENAGENTS_KEY_STORE", "file")
        .env("OPENAGENTS_SETTINGS", home.join("settings.json"))
        .current_dir(home)
        .args(args);
    if let Some(dir) = directory {
        command.env("OPENAGENTS_KNOWLEDGE", dir);
    }
    command.output().unwrap()
}
fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}
#[test]
fn installed_defaults_and_key_guidance() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    let missing = run(home, &["kb", "withdraw", "nope"], None);
    assert_eq!(missing.status.code(), Some(1));
    assert!(text(&missing).contains(&format!(
        "no entry nope in {}",
        home.join(".openagents/knowledge/entries").display()
    )));
    let search = run(home, &["--json", "kb", "search", "docker"], None);
    assert!(search.status.success(), "{}", text(&search));
    let value: serde_json::Value = serde_json::from_slice(&search.stdout).unwrap();
    assert!(value["local"].as_u64().unwrap() > 0);
    assert!(
        value["source"]
            .as_str()
            .unwrap()
            .contains("bundled entries")
    );
    let reason = value["lexical_only"].as_str().unwrap();
    assert!(reason.contains("openagents settings provider-key set openrouter"));
    assert!(!reason.contains("put api_key"));
    assert!(!reason.contains("openai.json"));
    let id = value["hits"][0]["id"].as_str().unwrap();
    let show = run(home, &["kb", "show", id], None);
    assert!(show.status.success(), "{}", text(&show));
    assert!(text(&show).contains("entries: bundled entries"));
    let plain = run(home, &["kb", "search", "docker"], None);
    assert!(text(&plain).contains("entries: bundled entries"));
    assert!(text(&plain).contains("openagents settings provider-key set openrouter"));
    let withdrawn = run(home, &["kb", "withdraw", id], None);
    assert!(withdrawn.status.success(), "{}", text(&withdrawn));
    let show = run(home, &["--json", "kb", "show", id], None);
    let shown: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(shown["entry"]["status"], "withdrawn");
}
#[test]
fn overrides_are_not_seeded() {
    let home = tempfile::tempdir().unwrap();
    let entries = tempfile::tempdir().unwrap();
    for args in [
        vec!["--json", "kb", "search", "docker", "--lexical"],
        vec![
            "--json",
            "kb",
            "search",
            "docker",
            "--lexical",
            "--dir",
            entries.path().to_str().unwrap(),
        ],
    ] {
        let out = run(home.path(), &args, Some(entries.path()));
        assert!(out.status.success(), "{}", text(&out));
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["local"], 0);
        assert!(!value["source"].as_str().unwrap().contains("bundled"));
    }
    assert_eq!(std::fs::read_dir(entries.path()).unwrap().count(), 0);
}
