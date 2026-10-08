//! Installed native privacy controls, with isolated files and no contact transport.
#![cfg(unix)]
#[path = "support/sales_binary.rs"]
mod sales_binary;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn hash(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}
fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn run(base: &Path, credential: &str, args: &[&str]) -> Output {
    Command::new(sales_binary::path())
        .args(["--json", "sales"])
        .args(args)
        .arg("--root")
        .arg(base.join("host"))
        .arg("--credential")
        .arg(base.join(credential))
        .env("HOME", base)
        .env("OPENAGENTS_SCRATCH", base.join("scratch"))
        .env("OPENAGENTS_TASKS", base.join("tasks"))
        .env_remove("OPENAGENTS_API_KEY")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("OPENROUTER_API_KEY")
        .current_dir(base)
        .output()
        .unwrap()
}
fn ok(base: &Path, credential: &str, args: &[&str]) -> Value {
    let o = run(base, credential, args);
    assert!(
        o.status.success(),
        "{} {}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    serde_json::from_slice(&o.stdout).unwrap()
}
fn apply(base: &Path, name: &str, value: Value) -> Output {
    let path = base.join(format!("{name}.json"));
    write(&path, &value);
    run(
        base,
        "owner",
        &["privacy", "apply", "--input", path.to_str().unwrap()],
    )
}
#[test]
fn installed_contact_admission_copies_restart_ambiguous_opt_out_and_reimport() {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let base = temp.path();
    fs::create_dir(base.join("scratch")).unwrap();
    fs::set_permissions(base.join("scratch"), fs::Permissions::from_mode(0o700)).unwrap();
    ok(base, "owner", &["init", "--owner", "operator"]);
    let now = coder::task::sales::unix_now();
    let root = base.join("host");
    let alice = coder::task::agent::Store::with_keys(
        &root,
        "alice",
        std::sync::Arc::new(coder::task::agent_key::FileKeys),
    )
    .unwrap();
    let owner_key = secp256k1::SecretKey::from_byte_array([42; 32]).unwrap();
    let record = alice
        .open_as(base, now, coder::task::agent::preset("alice"))
        .unwrap();
    let record = alice.ensure_key(record, now).unwrap();
    alice
        .attest(record, &owner_key, now + 200 * 86400, now)
        .unwrap();
    let original_key = alice.key().unwrap();
    let owner_path = base.join("memory-owner.key");
    fs::write(&owner_path, "2a".repeat(32)).unwrap();
    fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600)).unwrap();
    let mut memory = match coder::task::agent_engrams::EngramStore::open(
        &alice,
        &secret_screen::Screen::shapes(),
        now,
    ) {
        coder::task::agent_engrams::Opened::Ready(s) => s,
        _ => panic!("isolated fixture memory unavailable"),
    };
    memory
        .put(
            nostr::engram::Body::core("original-customer-id requested an old workflow"),
            now,
        )
        .unwrap();
    drop(memory);
    let agent = |args: &[&str]| {
        Command::new(sales_binary::path())
            .env_clear()
            .env("HOME", base)
            .env("PATH", "/usr/bin:/bin")
            .env("OPENAGENTS_SCRATCH", base.join("scratch"))
            .current_dir(base)
            .args(["--json", "agent"])
            .args(args)
            .arg("--root")
            .arg(&root)
            .arg("--control-socket")
            .arg(base.join("absent.sock"))
            .output()
            .unwrap()
    };
    assert!(
        agent(&[
            "memory",
            "alice",
            "engrams",
            "--owner-key",
            owner_path.to_str().unwrap()
        ])
        .status
        .success()
    );
    let path = base.join("lead.json");
    let create = json!({"schema":"openagents.sales.pipeline-command.v1","id":"original-lead","lead":null,"expected_revision":0,"operation":{"kind":"create","ownership_acceptance":"operator accepted private business responsibility","input":{"contact":"email:SeededBuyer@BUSINESS.invalid","source":"Seeded private business source","source_at":now,"details":{"account":"original-customer-id","jurisdiction":"US","permission":{"state":"granted","reference":"owner verified requested business contact","recorded_at":now,"expires_at":now+3600,"channels":["email"]},"workflow":"Seeded private workflow","baseline_reference":"private-baseline","data":{"recipients":["human:operator"],"permitted_use":"one private workflow","retain_until":now+7200},"stage":"qualified","next":{"description":"review private scope","due_at":now+600},"customer_decision":null,"readers":[]}}}});
    write(&path, &create);
    let lead = ok(base, "owner", &["apply", "--input", path.to_str().unwrap()])["lead"]
        .as_str()
        .unwrap()
        .to_owned();
    let legacy_read = agent(&[
        "memory",
        "alice",
        "engrams",
        "--owner-key",
        owner_path.to_str().unwrap(),
    ]);
    assert!(!legacy_read.status.success());
    assert!(!String::from_utf8_lossy(&legacy_read.stdout).contains("original-customer-id"));
    // These exact selected native paths refuse before a Live relay connector.
    for args in [
        vec![
            "memory",
            "alice",
            "engrams",
            "--owner-key",
            owner_path.to_str().unwrap(),
            "--from-relay",
            "--agent",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--relay",
            "ws://127.0.0.1:9",
        ],
        vec![
            "memory",
            "unknown",
            "engrams",
            "--owner-key",
            owner_path.to_str().unwrap(),
            "--from-relay",
            "--agent",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--relay",
            "ws://127.0.0.1:9",
        ],
    ] {
        let o = agent(&args);
        assert!(!o.status.success());
        let output = format!(
            "{} {}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(
            output.contains("unavailable") || output.contains("unqualified"),
            "{output}"
        );
        assert!(!output.contains("original-customer-id"));
    }
    let paul = coder::task::agent::Store::with_keys(
        &root,
        "paul",
        std::sync::Arc::new(coder::task::agent_key::FileKeys),
    )
    .unwrap();
    let record = paul
        .open_as(base, now, coder::task::agent::preset("paul"))
        .unwrap();
    let record = paul.ensure_key(record, now).unwrap();
    paul.attest(record, &owner_key, now + 200 * 86400, now)
        .unwrap();
    let o = agent(&[
        "memory",
        "paul",
        "sync",
        "on",
        "--relay",
        "ws://127.0.0.1:9",
    ]);
    assert!(!o.status.success());
    assert!(!paul.dir().join("sync.json").exists());
    let selected = paul.load().unwrap().unwrap().pubkey.unwrap();
    let sales_relay = agent(&[
        "memory",
        "paul",
        "engrams",
        "--owner-key",
        owner_path.to_str().unwrap(),
        "--from-relay",
        "--agent",
        &selected,
        "--relay",
        "ws://127.0.0.1:9",
    ]);
    assert!(!sales_relay.status.success());
    let refused_text = format!(
        "{} {}",
        String::from_utf8_lossy(&sales_relay.stdout),
        String::from_utf8_lossy(&sales_relay.stderr)
    );
    assert!(refused_text.contains("unavailable"), "{refused_text}");

    let refused = run(
        base,
        "owner",
        &["privacy", "check", "--lead", &lead, "--channel", "email"],
    );
    assert!(!refused.status.success());
    assert!(!String::from_utf8_lossy(&refused.stdout).contains("SeededBuyer"));
    let admission = json!({"schema":"openagents.sales-contact-command.v1","id":"owner-admission","expected_revision":0,"operation":{"kind":"admit","admission":{"lead":lead,"expected_lead_revision":1,"customer":"original-customer-id","jurisdiction":"US","source_kind":"given_business_role","permission_kind":"requested_contact","source_sha256":hash("Seeded private business source"),"permission_reference_sha256":hash("owner verified requested business contact"),"owner_reference":"operator reviewed actual requested contact","aliases":["email:seededbuyer@business.invalid",format!("nostr:{}","a".repeat(64))]}}});
    assert!(apply(base, "admission", admission.clone()).status.success());
    assert!(apply(base, "admission", admission).status.success());
    let gate = ok(
        base,
        "owner",
        &["privacy", "check", "--lead", &lead, "--channel", "email"],
    );
    assert_eq!(gate["contact_admitted"], true);
    assert_eq!(gate["send_authority"], false);
    for name in ["scratch-capture.json", "cache-export.json"] {
        let export = base.join("scratch").join(name);
        ok(
            base,
            "owner",
            &[
                "export",
                "--lead",
                &lead,
                "--output",
                export.to_str().unwrap(),
            ],
        );
        assert!(fs::read_to_string(export).unwrap().contains("SeededBuyer"));
    }
    let before = ok(base, "owner", &["privacy", "view"]);
    assert_eq!(before["copies"].as_array().unwrap().len(), 2);
    assert_eq!(before["model_disclosure_available"], false);
    assert_eq!(before["relay_disclosure_available"], false);
    let stop = json!({"schema":"openagents.sales-contact-command.v1","id":"ambiguous-stop","expected_revision":1,"operation":{"kind":"opt_out","contact":format!("nostr:{}","a".repeat(64)),"customer":null,"reference":"ambiguous actual opt-out reply","ambiguous":true}});
    assert!(apply(base, "stop", stop.clone()).status.success());
    assert!(apply(base, "stop", stop).status.success());
    for name in ["scratch-capture.json", "cache-export.json"] {
        assert!(!base.join("scratch").join(name).exists());
    }
    assert_eq!(alice.key().unwrap(), original_key);
    let minimized = agent(&[
        "memory",
        "alice",
        "engrams",
        "--owner-key",
        owner_path.to_str().unwrap(),
    ]);
    assert!(
        minimized.status.success(),
        "{} {}",
        String::from_utf8_lossy(&minimized.stdout),
        String::from_utf8_lossy(&minimized.stderr)
    );
    assert!(!String::from_utf8_lossy(&minimized.stdout).contains("original-customer-id"));
    let after = ok(base, "owner", &["privacy", "view"]);
    assert!(
        after["copies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["state"] == "removed")
    );
    assert_eq!(after["historical_remote_erasure_verified"], false);
    let stdout = serde_json::to_string(&after).unwrap();
    let state = fs::read_to_string(base.join("host/sales/state.json")).unwrap();
    for seed in [
        "SeededBuyer",
        "seededbuyer@",
        "Seeded private business source",
        "Seeded private workflow",
        "original-customer-id",
    ] {
        assert!(!stdout.contains(seed));
        assert!(!state.contains(seed));
    }
    let show = run(base, "owner", &["show", "--lead", &lead]);
    assert!(!show.status.success());
    assert!(!String::from_utf8_lossy(&show.stdout).contains("SeededBuyer"));
    let mut reimport = create;
    reimport["id"] = "renamed-source-import".into();
    reimport["operation"]["input"]["contact"] = "email:new-address@business.invalid".into();
    write(&path, &reimport);
    assert!(
        !run(base, "owner", &["apply", "--input", path.to_str().unwrap()])
            .status
            .success()
    );
    reimport["id"] = "renamed-customer-import".into();
    reimport["operation"]["input"]["details"]["account"] = "different-display-name".into();
    reimport["operation"]["input"]["contact"] = "email:seededbuyer@business.invalid".into();
    write(&path, &reimport);
    assert!(
        !run(base, "owner", &["apply", "--input", path.to_str().unwrap()])
            .status
            .success()
    );
    assert_eq!(
        ok(
            base,
            "owner",
            &[
                "suppressed",
                "--contact",
                "EMAIL:SEEDEDBUYER@BUSINESS.INVALID"
            ]
        )["suppressed"],
        true
    );
}
