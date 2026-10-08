#[path = "support/sales_binary.rs"]
mod sales_binary;
use gym::{sales_evidence as evidence, sales_finance as finance};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
#[allow(dead_code)]
mod sources {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../receipts/tests/support/service_sale.rs"
    ));
}

fn reference(r: &receipts::service_sale::Reference) -> evidence::Reference {
    evidence::Reference {
        path: r.path.clone(),
        sha256: r.sha256.clone(),
    }
}
fn run(root: &Path, args: &[&str]) -> Value {
    let result = Command::new(sales_binary::path())
        .current_dir(root)
        .env("HOME", root)
        .args(["sales"])
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}
fn attempt(root: &Path, id: &str, accepted: bool) -> Value {
    let session = atif::Session::opening(
        id,
        "synthetic-model",
        "synthetic-provider",
        "synthetic-fixture",
        "revision",
    );
    let path = root.join(format!("{id}.jsonl"));
    let mut log = atif::Log::create_at(&path, &session).unwrap();
    log.append(&atif::Step::said(atif::Source::User, "synthetic task"))
        .unwrap();
    log.finish(atif::log::ENDED).unwrap();
    drop(log);
    let trace = sources::retain(root, &format!("{id}.jsonl"), &fs::read(&path).unwrap());
    let artifact = sources::retain(root, &format!("{id}.patch"), id.as_bytes());
    let check = sources::retain(
        root,
        &format!("{id}.check"),
        b"synthetic passing frozen check",
    );
    let frozen = sources::retain(root, "frozen-command", b"synthetic frozen check command");
    let acceptance=accepted.then(||json!({"candidate_digest":artifact.sha256,"independent_checker":"checker",
        "check_review":sources::retain(root,"independent-review",b"synthetic checker accepted exact candidate"),
        "customer_decision":sources::retain(root,"customer-decision",b"synthetic buyer accepted exact candidate")}));
    let costs = ["provider", "compute", "support"]
        .into_iter()
        .map(|component| {
            let name = format!("{id}-{component}-bill");
            json!({"component":component,"basis":"billed","unit":"USD_millionths","amount":10,
            "evidence":sources::retain(root,&name,name.as_bytes()),"price":null})
        })
        .collect::<Vec<_>>();
    json!({"id":id,"kind":"primary","parent":null,"executor":"executor","trace":trace,"artifact":artifact,
        "checks":{"check":{"check_digest":frozen.sha256,"status":"passed","evidence":check}},
        "setup_ms":1,"queue_ms":1,"check_ms":1,"support_ms":1,"costs":costs,"compute":null,"acceptance":acceptance})
}
#[test]
fn earned_sales_ring_once_and_share_only_a_reviewed_aggregate() {
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let root = dir.path();
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
    let host = root.join("host");
    let credential = root.join("owner-token");
    let host = host.to_str().unwrap();
    let credential = credential.to_str().unwrap();
    run(
        root,
        &[
            "init",
            "--root",
            host,
            "--owner",
            "operator",
            "--credential",
            credential,
        ],
    );
    let now = coder::task::sales::unix_now();
    let input = json!({"schema":"openagents.sales.pipeline-command.v1","id":"create","lead":null,"expected_revision":0,
        "operation":{"kind":"create","ownership_acceptance":"synthetic operator acceptance","input":{
            "contact":"email:synthetic@fixture.invalid","source":"synthetic private introduction","source_at":now-6,
            "details":{"account":"private-account","jurisdiction":"synthetic","permission":{"state":"granted","reference":"synthetic consent",
                "recorded_at":now-6,"expires_at":now+500,"channels":["email"]},"workflow":"one synthetic task",
                "baseline_reference":"synthetic baseline","data":{"recipients":["human:operator"],"permitted_use":"one synthetic service","retain_until":now+2000},
                "stage":"qualified","next":{"description":"review accepted synthetic result","due_at":now+20},"customer_decision":null,"readers":[]}}}});
    fs::write(
        root.join("create.json"),
        serde_json::to_vec(&input).unwrap(),
    )
    .unwrap();
    let created = run(
        root,
        &[
            "apply",
            "--root",
            host,
            "--credential",
            credential,
            "--input",
            "create.json",
        ],
    );
    let lead = created["lead"].as_str().unwrap();
    let baseline = attempt(root, "baseline", false);
    let candidate = attempt(root, "candidate", true);
    let frozen = sources::retain(root, "frozen-command", b"synthetic frozen check command");
    let inventory = sources::doc(
        root,
        "task-inventory.json",
        json!({"schema":"openagents.gym.sales-inventory.v1",
        "attempts":{"task/baseline":["baseline"],"task/candidate":["candidate"]}}),
    );
    let study = sources::doc(
        root,
        "comparison.json",
        json!({"schema":evidence::SCHEMA,"offer_version":"offer-v1","source_revision":"a".repeat(40),
        "baseline_method":"manual","candidate_method":"Coder","retrospective_selection":false,"inventory":inventory,"gym_store":null,
        "tasks":[{"id":"task","task_digest":"b".repeat(64),"check_digests":{"check":frozen.sha256},
            "baseline":[baseline],"candidate":[candidate.clone()]}],"skipped_evidence":[]}),
    );
    let checked = evidence::rebuild(root, &fs::read(root.join(&study.path)).unwrap()).unwrap();
    let report = sources::retain(
        root,
        "checked-report.json",
        &serde_json::to_vec(&checked).unwrap(),
    );
    let mut admission = sources::admission(
        root,
        now,
        lead,
        "private-account",
        "offer-v1",
        sources::Comparison {
            manifest: study.clone(),
            report,
            candidate: serde_json::from_value(candidate["artifact"].clone()).unwrap(),
            check: serde_json::from_value(candidate["acceptance"]["check_review"].clone()).unwrap(),
            decision: serde_json::from_value(candidate["acceptance"]["customer_decision"].clone())
                .unwrap(),
            frozen_checks: vec![frozen],
        },
    );
    admission.fulfillment = Some(sources::fulfillment(root, now, &admission));
    let apply = |name: &str, revision: u64, operation: Value| {
        let command = json!({"schema":"openagents.sales.pipeline-command.v1","id":name,"lead":lead,
            "expected_revision":revision,"operation":operation});
        fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec(&command).unwrap(),
        )
        .unwrap();
        run(
            root,
            &[
                "apply",
                "--root",
                host,
                "--credential",
                credential,
                "--input",
                &format!("{name}.json"),
                "--evidence-root",
                ".",
            ],
        )
    };
    apply(
        "admit",
        1,
        json!({"kind":"record_service_sale","admission":admission}),
    );
    let earned = |words: &[&str]| {
        let mut args = vec!["earned"];
        args.extend_from_slice(words);
        args.extend_from_slice(&["--root", host, "--credential", credential]);
        run(root, &args)
    };
    assert_eq!(earned(&["ring"]), json!([]));
    let ledger = earned(&["ledger"]);
    assert_eq!(ledger["totals"]["earned_sales"], 0);
    assert!(
        ledger["rows"][0]["ineligible_because"]
            .as_array()
            .unwrap()
            .len()
            >= 1
    );
    let proof = sources::retain(
        root,
        "verified-payment",
        b"synthetic owner checked exact external collection",
    );
    apply(
        "paid",
        2,
        json!({"kind":"reconcile_service_payment","sale":"synthetic-sale","payment":{"disposition":"paid",
        "external_reference":"synthetic-external-payment","paid_minor":25000,"reversed_minor":null,"evidence":proof}}),
    );
    assert_eq!(
        earned(&["ring"]),
        json!([]),
        "paid but undelivered earns nothing"
    );
    let fulfillment = sources::fulfillment_input(root, now, true);
    apply(
        "delivered",
        3,
        json!({"kind":"reconcile_service_fulfillment","sale":"synthetic-sale","fulfillment":fulfillment}),
    );
    let rings = earned(&["ring"]);
    assert_eq!(rings.as_array().unwrap().len(), 1, "{rings}");
    assert_eq!(earned(&["ring"]), json!([]), "a replay rings nothing");
    let ledger = earned(&["ledger"]);
    assert_eq!(ledger["totals"]["earned_sales"], 1);
    assert_eq!(ledger["totals"]["rung"], 1);
    assert_eq!(ledger["totals"]["net_usd_millionths"], 250_000_000);
    let shared = run(root, &["earned", "shared", "--root", host]);
    assert_eq!(shared["state"], "unavailable");
    let text = serde_json::to_string(&shared).unwrap();
    assert!(!text.contains(lead) && !text.contains("synthetic-sale") && !text.contains("25000"));
    let too_soon = Command::new(sales_binary::path())
        .current_dir(root)
        .env("HOME", root)
        .args([
            "sales",
            "earned",
            "draft",
            "--through",
            &now.to_string(),
            "--root",
            host,
            "--credential",
            credential,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        !too_soon.status.success(),
        "a draft through today is refused"
    );
}
