//! Installed owner-only email controls with private fixture credentials and no mailbox.
#![cfg(unix)]
use coder::task::{agent, agent_key::FileKeys, sales};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::Arc,
};
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn run(base: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .env_clear()
        .env("HOME", base)
        .env("PATH", "/usr/bin:/bin")
        .env("OPENAGENTS_SCRATCH", base.join("scratch"))
        .current_dir(base)
        .args(["--json", "sales", "email"])
        .args(args)
        .arg("--root")
        .arg(base.join("host"))
        .arg("--credential")
        .arg(base.join("owner"))
        .output()
        .unwrap()
}
fn ok(base: &Path, args: &[&str]) -> Value {
    let o = run(base, args);
    assert!(
        o.status.success(),
        "{} {}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    serde_json::from_slice(&o.stdout).unwrap()
}
#[test]
fn installed_email_configuration_checks_opaque_provider_credentials_without_sending() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path();
    fs::set_permissions(base, fs::Permissions::from_mode(0o700)).unwrap();
    let root = base.join("host");
    let now = sales::unix_now();
    let mut store = sales::Store::open(&root).unwrap();
    store.initialize("operator", &base.join("owner")).unwrap();
    let owner = store
        .authenticate(&sales::Store::read_credential(&base.join("owner")).unwrap())
        .unwrap();
    let native = agent::Store::with_keys(&root, "paul", Arc::new(FileKeys)).unwrap();
    let record = native.open_as(base, now, agent::preset("paul")).unwrap();
    let record = native.ensure_key(record, now).unwrap();
    native
        .attest(
            record,
            &secp256k1::SecretKey::from_byte_array([22; 32]).unwrap(),
            now + 10000,
            now,
        )
        .unwrap();
    let anchor = store.sales_agent_anchor(&owner, "paul").unwrap();
    let recipients = vec![
        "human:operator".to_string(),
        format!("agent:{}", anchor.pubkey),
        "provider:email:fixture".into(),
    ];
    let create = json!({"schema":sales::COMMAND_SCHEMA,"id":"email-lead","lead":null,"expected_revision":0,"operation":{"kind":"create","ownership_acceptance":"synthetic owner accepted","input":{"contact":"email:buyer@fixture.invalid","source":"original requested business contact","source_at":now,"details":{"account":"original-business","jurisdiction":"US","permission":{"state":"granted","reference":"original customer accepted requested email","recorded_at":now,"expires_at":now+5000,"channels":["email"]},"workflow":"one private requested workflow","baseline_reference":"opaque baseline","data":{"recipients":recipients,"permitted_use":"one private requested email through fixture provider","retain_until":now+8000},"stage":"qualified","next":{"description":"review private scope","due_at":now+1000},"customer_decision":null,"readers":[]}}}});
    let lead = store
        .apply(&owner, &serde_json::to_vec(&create).unwrap())
        .unwrap()
        .lead;
    let contact = sales::privacy::Command {
        schema: sales::privacy::COMMAND_SCHEMA.into(),
        id: "admit-email".into(),
        expected_revision: 0,
        operation: sales::privacy::Operation::Admit {
            admission: sales::privacy::Admission {
                lead: lead.clone(),
                expected_lead_revision: 1,
                customer: "original-business".into(),
                jurisdiction: "US".into(),
                source_kind: sales::privacy::SourceKind::GivenBusinessRole,
                permission_kind: sales::privacy::PermissionKind::RequestedContact,
                source_sha256: sha(b"original requested business contact"),
                permission_reference_sha256: sha(b"original customer accepted requested email"),
                owner_reference: "owner verified original fixture request".into(),
                aliases: vec!["email:buyer@fixture.invalid".into()],
            },
        },
    };
    store
        .apply_sales_privacy(&owner, &serde_json::to_vec(&contact).unwrap())
        .unwrap();
    let policy = sales::agents::Policy {
        schema: sales::agents::POLICY_SCHEMA.into(),
        id: "email-policy".into(),
        version: 1,
        channels: vec!["email".into()],
        jurisdictions: vec!["US".into()],
        allowed_agents: vec![anchor.pubkey.clone()],
        data_recipients: recipients,
        timezone: "America/Chicago".into(),
        daily_floor_cap: 5,
        daily_agent_cap: 5,
        execution_budget_usd_millionths: 0,
        trust: sales::agents::Trust::IndividualReview,
        read_fields: [
            sales::agents::ReadField::Stage,
            sales::agents::ReadField::NextAction,
        ]
        .into(),
        write_fields: [sales::agents::WriteField::Draft].into(),
        playbook: sales::agents::Artifact {
            reference: "email-playbook".into(),
            sha256: "a".repeat(64),
        },
        permission_evidence_required: true,
        expires_at: now + 5000,
    };
    let policy_sha = policy.sha256().unwrap();
    let command = sales::agents::OwnerCommand {
        schema: sales::agents::OWNER_COMMAND_SCHEMA.into(),
        id: "email-policy".into(),
        expected_revision: 0,
        operation: sales::agents::OwnerOperation::PublishPolicy { policy },
    };
    store
        .apply_sales_agent_owner(&owner, &serde_json::to_vec(&command).unwrap(), None)
        .unwrap();
    drop(store);
    let secret = "synthetic-provider-oauth-token-not-32bytes";
    fs::write(base.join("mailbox"), secret).unwrap();
    fs::set_permissions(base.join("mailbox"), fs::Permissions::from_mode(0o600)).unwrap();
    let config = json!({"schema":sales::email::CONFIG_SCHEMA,"id":"fixture","version":1,"provider":"fixture","sender":"operator@fixture.invalid","reply_to":"operator@fixture.invalid","company":"Synthetic Company","human_responsible":"operator","postal_address":"1 Synthetic Road, Fixture City, US","identity_reference_sha256":"a".repeat(64),"commercial_label":"Commercial advertisement","unsubscribe_url":"https://fixture.invalid/unsubscribe","unsubscribe_reference_sha256":"b".repeat(64),"unsubscribe_available_until":now+31*86400,"credential_account":"sales-mailbox:fixture","credential_sha256":sha(secret.as_bytes()),"policy_sha256":policy_sha,"templates":{"email-v1":"c".repeat(64)},"domain_evidence":{"domain":"fixture.invalid","spf":"passed","dkim":"passed","dmarc":"passed","tls":"passed","authentication":"passed","reference_sha256":"d".repeat(64),"expires_at":now+3600},"expires_at":now+3600});
    write(
        &base.join("config.json"),
        &json!({"schema":sales::email::COMMAND_SCHEMA,"id":"configure","expected_revision":0,"operation":{"kind":"configure","config":config}}),
    );
    ok(base, &["apply", "--input", "config.json"]);
    let view = ok(base, &["view"]);
    assert_eq!(view["live_sender_qualified"], false);
    let current = view["current"].as_str().unwrap();
    let mut message = json!({"schema":sales::email::MESSAGE_SCHEMA,"lead":lead,"expected_lead_revision":1,"config_sha256":current,"policy_sha256":policy_sha,"template":{"reference":"email-v1","sha256":"c".repeat(64)},"sender":{"kind":"human","principal":"operator"},"recipient":"buyer@fixture.invalid","subject":"Requested private scope","body":"Here is the requested private workflow scope.","subject_review_sha256":"e".repeat(64),"expires_at":now+1800});
    write(&base.join("message.json"), &message);
    let prepared = ok(
        base,
        &[
            "check",
            "--input",
            "message.json",
            "--mailbox-key",
            "mailbox",
        ],
    );
    assert_eq!(prepared["outbound_authority"], false);
    assert!(!prepared.to_string().contains(secret));
    assert!(
        !fs::read_to_string(root.join("sales/state.json"))
            .unwrap()
            .contains(secret)
    );
    message["body"] = json!(secret);
    write(&base.join("message.json"), &message);
    let refused = run(
        base,
        &[
            "check",
            "--input",
            "message.json",
            "--mailbox-key",
            "mailbox",
        ],
    );
    assert!(!refused.status.success());
    assert!(!String::from_utf8_lossy(&refused.stdout).contains(secret));
    assert!(!String::from_utf8_lossy(&refused.stderr).contains(secret));
    let digest = prepared["message_sha256"].as_str().unwrap();
    write(
        &base.join("provider.json"),
        &json!({"message_sha256":digest,"provider_id":"fixture-attempt","reference_sha256":"f".repeat(64),"delivery":"accepted","tls":"passed","authentication":"passed"}),
    );
    let evidence = ok(
        base,
        &[
            "evidence",
            "--input",
            "provider.json",
            "--message-sha256",
            digest,
        ],
    );
    assert_eq!(evidence["delivery"], "accepted");
    write(
        &base.join("revoke.json"),
        &json!({"schema":sales::email::COMMAND_SCHEMA,"id":"revoke","expected_revision":1,"operation":{"kind":"revoke","config_sha256":current,"reference_sha256":"f".repeat(64)}}),
    );
    ok(base, &["apply", "--input", "revoke.json"]);
    message["body"] = json!("opaque scope");
    write(&base.join("message.json"), &message);
    assert!(
        !run(
            base,
            &[
                "check",
                "--input",
                "message.json",
                "--mailbox-key",
                "mailbox"
            ]
        )
        .status
        .success()
    );
}
