#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use coder::workflow_template::{self as template, Report};
use serde_json::{Value, json};

fn command(home: &Path, words: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .current_dir(home)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env_remove("OPENAGENTS_API_KEY")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args(["plugin"])
        .args(words)
        .arg("--json")
        .output()
        .unwrap()
}
fn run(home: &Path, words: &[&str]) -> Value {
    let output = command(home, words);
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn retain(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn actual_template_cli_runs_checks_and_rejects_stale_private_context_without_a_provider() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let package = root.join("package");
    fs::create_dir_all(package.join("programs")).unwrap();
    retain(&package.join("package.json"), template::PACKAGE.as_bytes());
    retain(
        &package.join("programs/meeting-followup.json"),
        template::PROGRAM.as_bytes(),
    );
    // Installation is off and confers no input approval. It touches only scratch HOME.
    let installed = run(&root, &["install", text(&package)]);
    assert_eq!(installed["plugin"]["enabled"], false);
    let source = root.join("meeting.md");
    let snapshot = root.join("snapshot.json");
    let report = root.join("report.json");
    let checker = root.join("checker");
    fs::create_dir(&checker).unwrap();
    fs::set_permissions(&checker, fs::Permissions::from_mode(0o700)).unwrap();
    let protected = checker.join("protected.json");
    retain(
        &checker.join("private-labels"),
        b"PROTECTED_LABEL_NEVER_READABLE",
    );
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../coder/fixtures/workflow-template");
    let notes = fs::read(fixtures.join("meeting.md")).unwrap();
    let expected = fs::read(fixtures.join("protected.json")).unwrap();
    retain(&source, &notes);
    let generic = command(&root, &["run", text(&package), "--in", text(&root)]);
    assert!(!generic.status.success());
    assert!(
        format!(
            "{}{}",
            String::from_utf8_lossy(&generic.stdout),
            String::from_utf8_lossy(&generic.stderr)
        )
        .contains("operator-approved captured input")
    );

    retain(&protected, &expected);
    retain(
        &root.join("prior-customer.md"),
        b"ACTION: Preserve FIRST_CUSTOMER_PRIVATE\n",
    );
    retain(
        &root.join("credential"),
        b"fixture-credential-never-readable",
    );
    let prepared = run(
        &root,
        &[
            "template",
            "prepare",
            "--package",
            text(&package),
            "--source",
            text(&source),
            "--task",
            "second-customer-task",
            "--customer",
            "synthetic-new-customer",
            "--owner",
            "local-human",
            "--recipient",
            "local-reviewer",
            "--permission-epoch",
            "2",
        ],
    );
    retain(
        &snapshot,
        &serde_json::to_vec(&prepared["snapshot"]).unwrap(),
    );
    let approval = prepared["snapshot_digest"].as_str().unwrap();
    let run_words = [
        "template",
        "run",
        "--package",
        text(&package),
        "--source",
        text(&source),
        "--snapshot",
        text(&snapshot),
        "--approve",
        approval,
        "--recipient",
        "local-reviewer",
        "--permission-epoch",
        "2",
    ];
    let output = run(&root, &run_words);
    assert_eq!(output["report"]["with_template"]["finished"], true);
    for forbidden in [
        "FIRST_CUSTOMER_PRIVATE",
        "PROTECTED_LABEL_NEVER_READABLE",
        "fixture-credential",
        "protected.json",
    ] {
        assert!(!output.to_string().contains(forbidden));
    }
    retain(&report, &serde_json::to_vec(&output["report"]).unwrap());
    let protected_digest = template::sha256(&expected);
    let check_words = [
        "template",
        "check",
        "--source",
        text(&source),
        "--snapshot",
        text(&snapshot),
        "--report",
        text(&report),
        "--protected",
        text(&protected),
        "--protected-sha256",
        &protected_digest,
    ];
    let checked = run(&root, &check_words);
    assert_eq!(checked["check"]["passed"], true);
    assert_eq!(checked["check"]["items_with_template"], 2);
    assert_eq!(checked["check"]["items_without_file_access"], 0);
    assert_eq!(checked["check"]["customer_accepted"], false);
    let mut changed = run_words;
    changed[11] = "another-recipient";
    assert!(!command(&root, &changed).status.success());
    retain(&source, b"ACTION: New input requires approval\n");
    assert!(!command(&root, &run_words).status.success());
    retain(&source, &notes);
    let mut failed: Report = serde_json::from_value(output["report"].clone()).unwrap();
    failed.with_template.finished = false;
    failed.with_template.stopped = Some("cancelled".into());
    retain(&report, &serde_json::to_vec(&failed).unwrap());
    let rejected = command(&root, &check_words);
    assert!(!rejected.status.success());
    let rejected: Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert_eq!(rejected["check"]["passed"], false);
    retain(&report, &serde_json::to_vec(&output["report"]).unwrap());
    retain(
        &protected,
        &serde_json::to_vec(&json!({"changed":"protected input"})).unwrap(),
    );
    assert!(!command(&root, &check_words).status.success());
    if let Some(directory) = std::env::var_os("OPENAGENTS_TEMPLATE_EVIDENCE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        fs::create_dir_all(&directory).unwrap();
        for (name, value) in [
            ("snapshot.json", &prepared),
            ("comparison.json", &output),
            ("check.json", &checked),
        ] {
            retain(
                &directory.join(name),
                &serde_json::to_vec_pretty(value).unwrap(),
            );
        }
    }
    retain(&package.join("package.json"), b"{}");
    assert!(!command(&root, &run_words).status.success());
}
