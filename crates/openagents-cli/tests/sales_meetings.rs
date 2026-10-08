//! Installed private meeting controls over a scratch host.
#![cfg(unix)]
#[path = "support/sales_binary.rs"]
mod sales_binary;
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};
fn run(base: &Path, credential: &str, args: &[&str]) -> std::process::Output {
    Command::new(sales_binary::path())
        .args(["--json", "sales"])
        .args(args)
        .args([
            "--root",
            base.join("host").to_str().unwrap(),
            "--credential",
            base.join(credential).to_str().unwrap(),
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
fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
#[test]
fn installed_meetings_require_current_owner_and_never_invent_a_brief_or_confirmation() {
    let d = tempfile::tempdir().unwrap();
    let b = d.path();
    fs::set_permissions(b, fs::Permissions::from_mode(0o700)).unwrap();
    ok(b, "owner", &["init", "--owner", "operator"]);
    ok(
        b,
        "owner",
        &[
            "issue",
            "--human",
            "alex",
            "--role",
            "reader",
            "--new-credential",
            b.join("human").to_str().unwrap(),
        ],
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let slot = b.join("slot.json");
    write(
        &slot,
        &json!({"id":"slot","version":1,"human":"alex","start_at":now+1000,"end_at":now+2000,"expires_at":now+900,"availability_reference":"synthetic owner availability declaration"}),
    );
    let v = ok(
        b,
        "owner",
        &[
            "meetings",
            "slot",
            "--input",
            slot.to_str().unwrap(),
            "--expected-version",
            "0",
        ],
    );
    assert_eq!(v["availability_reference"].as_str().unwrap().len(), 64);
    assert!(
        !run(
            b,
            "human",
            &[
                "meetings",
                "slot",
                "--input",
                slot.to_str().unwrap(),
                "--expected-version",
                "0"
            ]
        )
        .status
        .success()
    );
    assert!(!run(b, "human", &["meetings", "slots"]).status.success());
    let create = b.join("lead.json");
    write(
        &create,
        &json!({"schema":"openagents.sales.pipeline-command.v1","id":"meeting-customer","lead":null,"expected_revision":0,"operation":{"kind":"create","ownership_acceptance":"operator accepted responsibility","input":{"contact":"email:synthetic@fixture.invalid","source":"requested introduction","source_at":now,"details":{"account":"synthetic customer","jurisdiction":"US","permission":{"state":"granted","reference":"synthetic accepted introduction","recorded_at":now,"expires_at":now+4000,"channels":["email"]},"workflow":"synthetic scoped workflow","baseline_reference":"unmeasured","data":{"recipients":["human:operator","human:alex"],"permitted_use":"requested demo only","retain_until":now+5000},"stage":"qualified","next":{"description":"human reviews requested demo","due_at":now+3000},"customer_decision":null,"readers":[]}}}}),
    );
    let lead = ok(b, "owner", &["apply", "--input", create.to_str().unwrap()])["lead"]
        .as_str()
        .unwrap()
        .to_owned();
    let admission = b.join("admission.json");
    use sha2::{Digest, Sha256};
    let hash = |s: &str| {
        Sha256::digest(s.as_bytes())
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<String>()
    };
    write(
        &admission,
        &json!({"schema":"openagents.sales-contact-command.v1","id":"meeting-business-contact","expected_revision":0,"operation":{"kind":"admit","admission":{"lead":lead,"expected_lead_revision":1,"customer":"synthetic customer","jurisdiction":"US","source_kind":"given_business_role","permission_kind":"accepted_introduction","source_sha256":hash("requested introduction"),"permission_reference_sha256":hash("synthetic accepted introduction"),"owner_reference":"operator checked requested business introduction","aliases":["email:synthetic@fixture.invalid"]}}}),
    );
    ok(
        b,
        "owner",
        &["privacy", "apply", "--input", admission.to_str().unwrap()],
    );
    let proposal = b.join("proposal.json");
    write(
        &proposal,
        &json!({"id":"proposal","expected_revision":0,"lead":lead,"expected_lead_revision":1,"slot":"slot","slot_version":1,"target":"alex","brief":null}),
    );
    let m = ok(
        b,
        "owner",
        &["meetings", "propose", "--input", proposal.to_str().unwrap()],
    );
    let view = ok(b, "human", &["meetings", "show", "--meeting", "proposal"]);
    assert!(view["meeting"]["brief"].is_null());
    for field in [
        "calendar_authority",
        "mailbox_authority",
        "payment_authority",
        "outbound_authority",
        "earned_revenue",
    ] {
        assert_eq!(view[field], false);
    }
    let reference = b.join("reference.json");
    write(&reference, &json!("synthetic requested demo"));
    assert!(
        !run(
            b,
            "owner",
            &[
                "meetings",
                "confirm",
                "--meeting",
                "proposal",
                "--revision",
                "1",
                "--approve",
                m["proposal_sha256"].as_str().unwrap(),
                "--input",
                reference.to_str().unwrap()
            ]
        )
        .status
        .success()
    );
    assert!(
        !run(
            b,
            "human",
            &["meetings", "propose", "--input", proposal.to_str().unwrap()]
        )
        .status
        .success()
    );
    assert!(!run(b, "human", &["show", "--lead", &lead]).status.success());
}
