//! Installed Paul controls use a scratch owner, native file key, and private
//! credentials. No provider, wallet, host service, or outbound call is made.
#![cfg(unix)]
use coder::task::{
    agent,
    agent_key::FileKeys,
    sales::{Store, paul},
};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};
fn run(base: &Path, group: &str, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
    command.args(["--json", group]);
    if group == "agent" {
        command.arg("sales");
    }
    command.args(args).args([
        "--root",
        base.join("host").to_str().unwrap(),
        "--credential",
        base.join("owner").to_str().unwrap(),
    ]);
    command
        .env("HOME", base)
        .env("OPENAGENTS_SCRATCH", base.join("scratch"))
        .env_remove("OPENAGENTS_API_KEY")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("OPENROUTER_API_KEY")
        .current_dir(base)
        .output()
        .unwrap()
}
fn ok(base: &Path, group: &str, args: &[&str]) -> Value {
    let output = run(base, group, args);
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn installed_agent_controls_require_exact_owner_binding_and_show_true_idle_without_model_capacity()
{
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();
    fs::set_permissions(base, fs::Permissions::from_mode(0o700)).unwrap();
    ok(base, "sales", &["init", "--owner", "operator"]);
    let root = base.join("host");
    let now = coder::task::sales::unix_now();
    let native = agent::Store::with_keys(&root, "paul", std::sync::Arc::new(FileKeys)).unwrap();
    let record = native.open_as(base, now, agent::preset("paul")).unwrap();
    let record = native.ensure_key(record, now).unwrap();
    native
        .attest(
            record,
            &secp256k1::SecretKey::from_byte_array([17; 32]).unwrap(),
            now + 5000,
            now,
        )
        .unwrap();
    let mut store = Store::open(&root).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&base.join("owner")).unwrap())
        .unwrap();
    let binding = paul::Binding {
        schema: paul::SCHEMA.into(),
        revision: 1,
        anchor: store.sales_agent_anchor(&owner, "paul").unwrap(),
        owner_credential: base.join("owner"),
        assignments: vec![],
        permitted_requesters: vec!["owner".into()],
    };
    drop(store);
    let input = base.join("binding.json");
    fs::write(&input, serde_json::to_vec(&binding).unwrap()).unwrap();
    fs::set_permissions(&input, fs::Permissions::from_mode(0o600)).unwrap();
    let checked = ok(
        base,
        "agent",
        &["binding-check", "--input", input.to_str().unwrap()],
    );
    assert_eq!(checked["sha256"], binding.sha256().unwrap());
    assert!(
        !run(
            base,
            "agent",
            &[
                "configure",
                "--input",
                input.to_str().unwrap(),
                "--approve",
                &"b".repeat(64)
            ]
        )
        .status
        .success()
    );
    ok(
        base,
        "agent",
        &[
            "configure",
            "--input",
            input.to_str().unwrap(),
            "--approve",
            checked["sha256"].as_str().unwrap(),
        ],
    );
    let idle = ok(
        base,
        "agent",
        &[
            "pipeline",
            "--requester",
            "owner",
            "--request",
            "installed-idle",
        ],
    );
    assert_eq!(idle["pipeline"]["idle"], true);
    assert_eq!(idle["pipeline"]["model_available"], false);
    assert_eq!(idle["pipeline"]["external_effects"], false);
    assert_eq!(idle["pipeline"]["rows"], json!([]));
    assert!(idle["expense"].is_null());
    assert!(
        !run(
            base,
            "agent",
            &["pipeline", "--requester", "other", "--request", "foreign"]
        )
        .status
        .success()
    );
    assert_eq!(
        ok(base, "agent", &["practice", "--requester", "owner"]),
        json!([])
    );
    let owner_view = ok(base, "agent", &["owner-view"]);
    assert_eq!(owner_view["idle"], true);
    assert_eq!(owner_view["rows"], json!([]));
    let research = base.join("research-input");
    fs::write(
        &research,
        &serde_json::to_vec(&json!({
            "lead":"missing-assignment",
            "helper":{"query":"cited_answer","release":"a".repeat(40),"claims":[]}
        }))
        .unwrap(),
    )
    .unwrap();
    fs::set_permissions(&research, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        !run(
            base,
            "agent",
            &[
                "research",
                "--requester",
                "owner",
                "--request",
                "not-a-pipeline",
                "--input",
                research.to_str().unwrap()
            ]
        )
        .status
        .success()
    );
    assert_eq!(ok(base, "sales", &["models", "history"]), json!([]));
    assert!(!root.join("agents/paul/coder-state").exists());
    let source = ok(base, "agent", &["source"]);
    assert_eq!(source["model_available"], false);
    assert_eq!(source["source"]["basis"], "local_deterministic");
    let record = native.load().unwrap().unwrap();
    let mut paused = record;
    paused.state = agent::State::Paused;
    native.save(&paused).unwrap();
    assert!(
        !run(
            base,
            "agent",
            &["pipeline", "--requester", "owner", "--request", "paused"]
        )
        .status
        .success()
    );
}
