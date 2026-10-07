//! The shipped CLI checks only explicit private fixtures and installs inert guidance.

#![cfg(unix)]

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn private(path: &Path) {
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if path.is_dir() { 0o700 } else { 0o600 }),
    )
    .unwrap();
}
fn copy_tree(source: &Path, into: &Path) {
    fs::create_dir_all(into).unwrap();
    private(into);
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = into.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
            private(&target);
        }
    }
}
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(["--json"])
        .args(args)
        .env("HOME", root)
        .env("OPENAGENTS_SCRATCH", root.join("scratch"))
        .env("OPENAGENTS_TASKS", root.join("tasks"))
        .env_remove("OPENAGENTS_CODER_MODEL_INPUT")
        .env_remove("OPENROUTER_API_KEY")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("AI_GATEWAY_API_KEY")
        .current_dir(root)
        .output()
        .unwrap()
}
fn ok(root: &Path, args: &[&str]) -> Value {
    let out = run(root, args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn package() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/brainstorm")
}

#[test]
fn shipped_guidance_install_and_enable_do_not_enable_native_reads_or_publish() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path();
    let source = root.join("source");
    copy_tree(&package(), &source);
    let value = ok(root, &["plugin", "install", source.to_str().unwrap()]);
    assert_eq!(value["plugin"]["enabled"], false);
    assert!(value["plugin"]["background"].as_array().unwrap().is_empty());
    assert_eq!(value["plugin"]["slug"], "brainstorm-guidance");
    assert!(!root.join(".openagents/coder-new").exists());
    let on = ok(root, &["plugin", "enable", "brainstorm-guidance"]);
    assert_eq!(on["plugin"]["enabled"], true);
    assert!(!root.join(".openagents/coder-new").exists());
    assert!(!root.join(".openagents/keys").exists());
    let state = root.join("coder-state");
    let out = run(
        root,
        &[
            "coder",
            "--state",
            state.to_str().unwrap(),
            "chat",
            "-p",
            "/brainstorm rank 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        ],
    );
    // The explicit lookup refuses before any provider or Brainstorm request.
    assert!(!out.status.success());
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("Enable Brainstorm"),
        "{result:?}"
    );
    assert!(!root.join(".openagents/keys").exists());
}

#[test]
fn shipped_private_checker_preserves_coverage_and_refuses_tampering_without_home_state() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path();
    private(root);
    let sources = root.join("sources");
    copy_tree(&package().join("examples"), &sources);
    let input = sources.join("pilot-fixture.json");
    let args = [
        "plugin",
        "brainstorm-pilot",
        "check",
        "--input",
        input.to_str().unwrap(),
        "--sources",
        sources.to_str().unwrap(),
    ];
    let value = ok(root, &args);
    assert_eq!(value["summary"]["basis"], "fixture");
    assert_eq!(value["summary"]["lookup_coverage"][0]["coverage"], "absent");
    assert_eq!(
        value["summary"]["lookup_coverage"][1]["coverage"],
        "unknown"
    );
    assert_eq!(value["summary"]["settled_payment_claims"], 0);
    assert_eq!(
        value["summary"]["independently_verified_paid_conversion"],
        false
    );
    assert!(!root.join(".openagents").exists());
    fs::write(sources.join("rank-zero.json"), "changed").unwrap();
    let out = run(root, &args);
    assert!(!out.status.success());
    assert!(!String::from_utf8_lossy(&out.stdout).contains("synthetic publisher"));
    assert!(!root.join(".openagents").exists());
}
