//! Real CLI dispatch over isolated private partner records; no service or funds.
#![cfg(unix)]
#[path = "support/sales_binary.rs"]
mod sales_binary;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

fn run(work: &Path, args: &[&str]) -> Output {
    Command::new(sales_binary::path())
        .args(["--json", "sales"])
        .args(args)
        .env("HOME", work)
        .env("OPENAGENTS_SCRATCH", work.join("scratch"))
        .env_remove("OPENROUTER_API_KEY")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("OPENAGENTS_API_KEY")
        .current_dir(work)
        .output()
        .unwrap()
}
fn ok(work: &Path, args: &[&str]) -> Value {
    let result = run(work, args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}
fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn source(root: &Path, name: &str, bytes: &[u8]) -> Value {
    fs::write(root.join(name), bytes).unwrap();
    fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    let digest = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    json!({"path":name,"sha256":digest})
}
#[test]
fn actual_cli_keeps_pending_briefs_private_and_accepts_only_the_exact_recipient() {
    let work = tempfile::tempdir().unwrap();
    fs::set_permissions(work.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let base = work.path();
    let host = base.join("host");
    let owner = base.join("owner");
    let partner = base.join("partner");
    let sources = base.join("sources");
    fs::create_dir(&sources).unwrap();
    fs::set_permissions(&sources, fs::Permissions::from_mode(0o700)).unwrap();
    let root = host.to_str().unwrap();
    let credential = owner.to_str().unwrap();
    let partner_credential = partner.to_str().unwrap();
    ok(
        base,
        &[
            "init",
            "--root",
            root,
            "--owner",
            "operator",
            "--credential",
            credential,
        ],
    );
    ok(
        base,
        &[
            "issue",
            "--root",
            root,
            "--credential",
            credential,
            "--human",
            "partner",
            "--role",
            "writer",
            "--new-credential",
            partner_credential,
        ],
    );
    let now = coder::task::sales::unix_now();
    let input = base.join("command.json");
    write(
        &input,
        &json!({"schema":"openagents.sales.pipeline-command.v1","id":"create","lead":null,"expected_revision":0,
        "operation":{"kind":"create","ownership_acceptance":"synthetic owner accepted",
        "input":{"contact":"email:synthetic@example.invalid","source":"synthetic permission","source_at":now,
        "details":{"account":"synthetic-private-account","jurisdiction":"synthetic jurisdiction",
        "permission":{"state":"granted","reference":"synthetic private consent","recorded_at":now,"expires_at":now+3600,"channels":["email"]},
        "workflow":"synthetic introduction","baseline_reference":"synthetic baseline",
        "data":{"recipients":["human:operator","human:partner"],"permitted_use":"one private introduction","retain_until":now+86400},
        "stage":"qualified","next":{"description":"review","due_at":now+600},"readers":[],"customer_decision":null}}}}),
    );
    let lead = ok(
        base,
        &[
            "apply",
            "--root",
            root,
            "--credential",
            credential,
            "--input",
            input.to_str().unwrap(),
        ],
    )["lead"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut proposal = json!({"id":"intro","recipient_human":"partner","expires_at":now+1800,"next":{"description":"prepare introduction","due_at":now+600},
        "terms":{"kind":"discovery","brief":source(&sources,"private-brief",b"synthetic private brief"),"permitted_use":"one private introduction"},
        "consent":source(&sources,"consent",b"synthetic scoped permission"),"provenance":source(&sources,"provenance",b"synthetic source"),
        "approval":{"path":"approval.json","sha256":"0".repeat(64)},"commission":null});
    let draft = base.join("proposal.json");
    write(&draft, &proposal);
    let digest = ok(
        base,
        &[
            "show",
            "--root",
            root,
            "--credential",
            credential,
            "--lead",
            &lead,
            "--proposal",
            draft.to_str().unwrap(),
        ],
    )["proposal_sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    let approval = json!({"schema":"openagents.sales.partner-approval.v1","pipeline_lead":lead,"assignment":"intro","proposal_sha256":digest,
        "approved_by":"operator","approved_at":now,"allow_private_assignment":true});
    proposal["approval"] = source(
        &sources,
        "approval.json",
        &serde_json::to_vec(&approval).unwrap(),
    );
    write(
        &input,
        &json!({"schema":"openagents.sales.pipeline-command.v1","id":"propose","lead":lead,"expected_revision":1,
        "operation":{"kind":"propose_partner","proposal":proposal}}),
    );
    ok(
        base,
        &[
            "apply",
            "--root",
            root,
            "--credential",
            credential,
            "--input",
            input.to_str().unwrap(),
            "--evidence-root",
            sources.to_str().unwrap(),
        ],
    );
    let pending = ok(
        base,
        &[
            "show",
            "--root",
            root,
            "--credential",
            partner_credential,
            "--lead",
            &lead,
            "--assignment",
            "intro",
        ],
    );
    assert!(pending.get("assignment").is_none());
    assert!(!pending.to_string().contains("private-brief"));
    assert!(!pending.to_string().contains("synthetic-private-account"));
    assert!(
        !run(
            base,
            &[
                "show",
                "--root",
                root,
                "--credential",
                partner_credential,
                "--lead",
                &lead
            ]
        )
        .status
        .success()
    );
    let proof = source(
        &sources,
        "recipient-ack",
        b"synthetic exact recipient acceptance",
    );
    write(
        &input,
        &json!({"schema":"openagents.sales.pipeline-command.v1","id":"accept","lead":lead,"expected_revision":2,
        "operation":{"kind":"advance_partner","assignment":"intro","action":{"kind":"accept","proposal_sha256":digest,"evidence":proof}}}),
    );
    assert!(
        !run(
            base,
            &[
                "apply",
                "--root",
                root,
                "--credential",
                credential,
                "--input",
                input.to_str().unwrap(),
                "--evidence-root",
                sources.to_str().unwrap()
            ]
        )
        .status
        .success()
    );
    ok(
        base,
        &[
            "apply",
            "--root",
            root,
            "--credential",
            partner_credential,
            "--input",
            input.to_str().unwrap(),
            "--evidence-root",
            sources.to_str().unwrap(),
        ],
    );
    let accepted = ok(
        base,
        &[
            "show",
            "--root",
            root,
            "--credential",
            partner_credential,
            "--lead",
            &lead,
            "--assignment",
            "intro",
        ],
    );
    assert_eq!(accepted["assignment"]["status"], "accepted");
    assert_eq!(accepted["authority_granted"], false);
    let exported = base.join("private-export.json");
    ok(
        base,
        &[
            "export",
            "--root",
            root,
            "--credential",
            partner_credential,
            "--lead",
            &lead,
            "--assignment",
            "intro",
            "--output",
            exported.to_str().unwrap(),
        ],
    );
    assert_eq!(
        fs::metadata(&exported).unwrap().permissions().mode() & 0o077,
        0
    );
    assert!(
        !run(
            base,
            &[
                "show",
                "--root",
                root,
                "--credential",
                credential,
                "--lead",
                &lead,
                "--assignment",
                "intro",
                "--sale",
                "other"
            ]
        )
        .status
        .success()
    );
    let state: Value =
        serde_json::from_slice(&fs::read(host.join("sales/state.json")).unwrap()).unwrap();
    assert_eq!(
        state["leads"][&lead]["partner_assignments"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    assert!(state["leads"][&lead].get("service_sales").is_none());
    assert!(!base.join(".openagents").exists());
}
