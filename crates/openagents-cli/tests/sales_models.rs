//! Actual expense controls over a scratch native host, without providers.
#![cfg(unix)]
#[path = "support/sales_binary.rs"]
mod sales_binary;
use coder::task::{
    agent,
    agent_key::FileKeys,
    sales::{Store, expenses as e},
};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};
fn run(base: &Path, args: &[&str]) -> std::process::Output {
    Command::new(sales_binary::path())
        .args(["--json", "sales"])
        .args(args)
        .args([
            "--root",
            base.join("host").to_str().unwrap(),
            "--credential",
            base.join("owner").to_str().unwrap(),
        ])
        .env("HOME", base)
        .env("OPENAGENTS_SCRATCH", base.join("scratch"))
        .env_remove("OPENAGENTS_API_KEY")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("OPENROUTER_API_KEY")
        .current_dir(base)
        .output()
        .unwrap()
}
fn ok(base: &Path, args: &[&str]) -> Value {
    let out = run(base, args);
    assert!(
        out.status.success(),
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn write(path: &Path, value: &impl serde::Serialize) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
#[test]
fn installed_owner_controls_preserve_native_originals_unknown_and_distinct_bills() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();
    fs::set_permissions(base, fs::Permissions::from_mode(0o700)).unwrap();
    ok(base, &["init", "--owner", "operator"]);
    let source = e::Source {
        basis: e::Basis::ListPrice,
        kind: e::Kind::Training,
        source_revision: "a".repeat(64),
        price_revision: "b".repeat(64),
        recipient: "human:operator".into(),
        max_input_bytes: 100,
        max_output_tokens: 100,
        max_attempts: 1,
        max_elapsed_secs: 100,
        input_usd_millionths_per_million: 5_000_000,
        output_usd_millionths_per_million: 5_000_000,
    };
    let policy = e::Policy {
        schema: "openagents.sales-model-policy.v1".into(),
        revision: 1,
        floor_daily_usd_millionths: 2_000,
        agent_daily_usd_millionths: 2_000,
        request_usd_millionths: 2_000,
        sources: vec![source.clone()],
    };
    let file = base.join("policy.json");
    write(&file, &policy);
    let checked = ok(
        base,
        &["models", "policy-check", "--input", file.to_str().unwrap()],
    );
    assert!(
        !run(
            base,
            &[
                "models",
                "policy",
                "--input",
                file.to_str().unwrap(),
                "--approve",
                &"f".repeat(64)
            ]
        )
        .status
        .success()
    );
    ok(
        base,
        &[
            "models",
            "policy",
            "--input",
            file.to_str().unwrap(),
            "--approve",
            checked["sha256"].as_str().unwrap(),
        ],
    );
    assert_eq!(ok(base, &["models", "history"]), json!([]));
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
    let context = e::TrainingContext {
        agent: "paul".into(),
        anchor: store.sales_agent_anchor(&owner, "paul").unwrap(),
        persona_sha256: "c".repeat(64),
        run: "synthetic-cli-run".into(),
        synthetic_source_sha256: "d".repeat(64),
    };
    use sha2::{Digest, Sha256};
    let input = e::Input {
        request: "installed-practice".into(),
        attempt: 1,
        source,
        input_bytes: 100,
        input_sha256: Sha256::digest(vec![b'x'; 100])
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    };
    let hold = store
        .reserve_sales_training(&owner, &context, &input)
        .unwrap();
    let original = hold.receipt().clone();
    drop(store);
    drop(hold);
    let shown = ok(base, &["models", "show", "--reservation", &original.id]);
    assert_eq!(shown["status"], "unknown");
    assert_eq!(shown["maximum_usd_millionths"], 1_000);
    assert_eq!(shown["training"]["run"], "synthetic-cli-run");
    assert_eq!(shown["input"]["source"]["price_revision"], "b".repeat(64));
    let settlement = base.join("settle.json");
    write(
        &settlement,
        &e::Settlement {
            request: "provider-usage".into(),
            estimated_usd_millionths: Some(1_000),
            billed_usd_millionths: Some(900),
            evidence_sha256: "e".repeat(64),
        },
    );
    let known = ok(
        base,
        &[
            "models",
            "settle",
            "--reservation",
            &original.id,
            "--input",
            settlement.to_str().unwrap(),
        ],
    );
    assert_eq!(known["status"], "known");
    assert_eq!(known["settlements"][0]["estimated_usd_millionths"], 1_000);
    assert_eq!(known["settlements"][0]["billed_usd_millionths"], 900);
    assert_eq!(ok(base, &["models", "history", "--limit", "1"])[0], known);
    assert!(
        !run(base, &["models", "history", "--limit", "101"])
            .status
            .success()
    );
    assert!(!run(base, &["models", "reserve"]).status.success());
}
