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
fn real_sales_cli_records_replays_exports_and_feeds_the_verified_operating_report() {
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
    let admission = sources::admission(
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
    let record = json!({"schema":"openagents.sales.pipeline-command.v1","id":"admit","lead":lead,"expected_revision":1,
        "operation":{"kind":"record_service_sale","admission":admission}});
    fs::write(
        root.join("admit.json"),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    let args = [
        "apply",
        "--root",
        host,
        "--credential",
        credential,
        "--input",
        "admit.json",
        "--evidence-root",
        ".",
    ];
    let first = run(root, &args);
    assert_eq!(run(root, &args), first);
    let proof = sources::retain(
        root,
        "verified-payment",
        b"synthetic owner checked exact external collection",
    );
    let paid = json!({"schema":"openagents.sales.pipeline-command.v1","id":"paid","lead":lead,"expected_revision":2,
        "operation":{"kind":"reconcile_service_payment","sale":"synthetic-sale","payment":{"disposition":"paid",
            "external_reference":"synthetic-external-payment","paid_minor":25000,"reversed_minor":null,"evidence":proof}}});
    fs::write(root.join("paid.json"), serde_json::to_vec(&paid).unwrap()).unwrap();
    let args = [
        "apply",
        "--root",
        host,
        "--credential",
        credential,
        "--input",
        "paid.json",
        "--evidence-root",
        ".",
    ];
    let reconciled = run(root, &args);
    assert_eq!(run(root, &args), reconciled);
    let shown = run(
        root,
        &[
            "show",
            "--root",
            host,
            "--credential",
            credential,
            "--lead",
            lead,
            "--sale",
            "synthetic-sale",
        ],
    );
    assert_eq!(shown["payments"].as_array().unwrap().len(), 1);
    run(
        root,
        &[
            "export",
            "--root",
            host,
            "--credential",
            credential,
            "--lead",
            lead,
            "--sale",
            "synthetic-sale",
            "--output",
            "service-export.json",
        ],
    );
    let exported = sources::retain(
        root,
        "service-export.json",
        &fs::read(root.join("service-export.json")).unwrap(),
    );
    assert_eq!(
        fs::metadata(root.join("service-export.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let inventory = sources::doc(
        root,
        "finance-inventory.json",
        json!({"schema":"openagents.gym.sales-finance-inventory.v1","entries":["service"],"complete":true}),
    );
    let allocation = sources::retain(
        root,
        "payer-allocation",
        b"synthetic buyer paid provider/compute/support; baseline excluded",
    );
    let no_cost = sources::retain(
        root,
        "no-additional-costs",
        b"synthetic owner declared remaining cost classes absent",
    );
    let m = finance::Manifest {
        schema: finance::SCHEMA.into(),
        owner: "operator".into(),
        period_start: now - 10,
        period_end: now + 100,
        inventory: reference(&inventory),
        comparisons: BTreeMap::from([("comparison".into(), reference(&study))]),
        offers: vec![finance::Offer {
            id: "service-offer".into(),
            version: "offer-v1".into(),
            account: "private-account".into(),
            cohort: "synthetic".into(),
            entries: vec![finance::Entry {
                id: "service".into(),
                at: shown["payments"][0]["verified_at"].as_u64().unwrap(),
                terms: finance::Terms {
                    version: "terms-v1".into(),
                    evidence: reference(&admission.sources.agreement),
                    unit: "USD_millionths".into(),
                    contractual_charge: 250_000_000,
                    billable_failure: false,
                },
                source: finance::Source::ServiceSale {
                    export: reference(&exported),
                },
                delivery: finance::Delivery::Accepted,
                delivery_evidence: reference(&admission.sources.customer_acceptance),
                task: Some(finance::TaskLink {
                    comparison: "comparison".into(),
                    task: "task".into(),
                    payer: finance::Payer::Customer,
                    include_baseline_costs: false,
                    allocation_evidence: reference(&allocation),
                }),
                expenses: vec![],
                adjustments: vec![],
                incidents: vec![],
            }],
            no_cost: [
                finance::ExpenseClass::Payment,
                finance::ExpenseClass::Setup,
                finance::ExpenseClass::Onboarding,
                finance::ExpenseClass::Repair,
                finance::ExpenseClass::Promotion,
            ]
            .into_iter()
            .map(|class| (class, reference(&no_cost)))
            .collect(),
            assumptions: vec![],
        }],
        gaps: vec![],
    };
    let r = finance::rebuild(root, &serde_json::to_vec(&m).unwrap()).unwrap();
    assert_eq!(
        r.offers[0].revenue["USD_millionths"].earned_openagents,
        250_000_000
    );
    assert_eq!(r.offers[0].profitable, Some(true));
    let serialized = serde_json::to_string(&r).unwrap();
    assert!(!serialized.contains(&fs::read_to_string(credential).unwrap()));
    assert!(!root.join("host/money").exists());
}

static FUNNEL_CLOCK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
fn funnel_clock() -> u64 {
    FUNNEL_CLOCK.load(std::sync::atomic::Ordering::SeqCst)
}
fn private_json(root: &Path, name: &str, value: &Value) {
    fs::write(root.join(name), serde_json::to_vec(value).unwrap()).unwrap();
    fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn real_sales_cli_weekly_review_rechecks_canonical_consent_and_keeps_unpaid_work_private() {
    use coder::task::sales::Store;
    use gym::sales_weekly as weekly;
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let root = dir.path();
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
    let at = coder::task::sales::unix_now() - 100;
    FUNNEL_CLOCK.store(at, std::sync::atomic::Ordering::SeqCst);
    let host = root.join("host");
    let credential = root.join("owner-token");
    let mut store = Store::open_with_clock(&host, funnel_clock).unwrap();
    store.initialize("operator", &credential).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&credential).unwrap())
        .unwrap();
    let writer_credential = root.join("writer-token");
    store
        .issue(
            &owner,
            "writer",
            coder::task::sales::Role::Writer,
            &writer_credential,
        )
        .unwrap();
    let input = json!({"schema":"openagents.sales.pipeline-command.v1","id":"weekly-create","lead":null,"expected_revision":0,
        "operation":{"kind":"create","ownership_acceptance":"synthetic operator responsibility","input":{
        "contact":"email:private-weekly@fixture.invalid","source":"synthetic private introduction","source_at":at,
        "details":{"account":"private-account","jurisdiction":"synthetic","permission":{"state":"granted","reference":"synthetic-consent",
        "recorded_at":at,"expires_at":at+7200,"channels":["email"]},"workflow":"synthetic accepted task","baseline_reference":"synthetic baseline",
        "data":{"recipients":["human:operator"],"permitted_use":"private consented weekly review","retain_until":at+7200},"stage":"qualified",
        "next":{"description":"review private evidence","due_at":at+600},"customer_decision":null,"readers":[]}}}});
    let lead = store
        .apply(&owner, &serde_json::to_vec(&input).unwrap())
        .unwrap()
        .lead;
    let consent = sources::retain(
        root,
        "weekly-consent",
        b"synthetic separate telemetry and delayed count permission",
    );
    let mut revision = 1;
    let mut apply = |store: &mut Store, id: &str, operation: Value| {
        let command = json!({"schema":"openagents.sales.pipeline-command.v1","id":id,"lead":lead,
            "expected_revision":revision,"operation":operation});
        let receipt = store
            .apply_with_evidence_root(&owner, &serde_json::to_vec(&command).unwrap(), Some(root))
            .unwrap();
        revision = receipt.revision;
    };
    apply(
        &mut store,
        "weekly-enroll",
        json!({"kind":"record_funnel_journey","admission":{
        "id":"journey","offer_version":"offer-v1","cohort":"private-cohort","lane":"self_serve","classification":"fixture",
        "consent":{"evidence":consent,"at":at,"expires_at":at+7200,"aggregate_counts":true}}}),
    );
    let baseline = attempt(root, "weekly-baseline", false);
    let candidate = attempt(root, "weekly-candidate", true);
    let frozen = sources::retain(root, "frozen-command", b"synthetic frozen check command");
    let inventory = sources::doc(
        root,
        "weekly-task-inventory.json",
        json!({"schema":"openagents.gym.sales-inventory.v1",
        "attempts":{"task/baseline":["weekly-baseline"],"task/candidate":["weekly-candidate"]}}),
    );
    let study = sources::doc(
        root,
        "weekly-comparison.json",
        json!({"schema":evidence::SCHEMA,"offer_version":"offer-v1",
        "source_revision":"a".repeat(40),"baseline_method":"manual","candidate_method":"Coder","retrospective_selection":false,
        "inventory":inventory,"gym_store":null,"tasks":[{"id":"task","task_digest":"b".repeat(64),
        "check_digests":{"check":frozen.sha256},"baseline":[baseline],"candidate":[candidate.clone()]}],"skipped_evidence":[]}),
    );
    let checked = evidence::rebuild(root, &fs::read(root.join(&study.path)).unwrap()).unwrap();
    let report = sources::retain(
        root,
        "weekly-checked-report.json",
        &serde_json::to_vec(&checked).unwrap(),
    );
    let attribution = sources::doc(
        root,
        "weekly-task-attribution.json",
        json!({
        "schema":"openagents.sales.task-account-attribution.v1","account":"private-account","offer_version":"offer-v1",
        "cohort":"private-cohort","manifest_digest":study.sha256,"task":"task",
        "customer_decision":candidate["acceptance"]["customer_decision"]}),
    );
    let observation = sources::retain(
        root,
        "weekly-observation",
        b"synthetic owner observation; no paid activation claim",
    );
    for (id, kind) in [
        (
            "acquisition",
            json!({"kind":"acquisition","source":"private introduction","evidence":observation}),
        ),
        (
            "install",
            json!({"kind":"install","client":"synthetic-cli","evidence":observation}),
        ),
        (
            "provider",
            json!({"kind":"provider_activation","provider":"synthetic-provider","evidence":observation}),
        ),
        (
            "task",
            json!({"kind":"task","manifest":study,"report":report,"attribution":attribution,"task":"task"}),
        ),
        (
            "purchase",
            json!({"kind":"purchase","financial_offer":"private-offer","entry":"unverified-entry",
            "source":{"kind":"settlement","key":"unverified-payment"},"evidence":observation}),
        ),
    ] {
        apply(
            &mut store,
            &format!("weekly-{id}"),
            json!({"kind":"record_funnel_event","journey":"journey",
            "event":{"id":id,"at":at,"kind":kind}}),
        );
    }
    apply(
        &mut store,
        "weekly-failure",
        json!({"kind":"record_conversion_failure","journey":"journey","failure":{
        "event":"purchase","reason":"payment_unknown","responsible_human":"operator","next_action":"privately reconcile payment evidence",
        "due_at":at+600,"evidence":observation,"resolved":false}}),
    );
    FUNNEL_CLOCK.store(at + 10, std::sync::atomic::Ordering::SeqCst);
    store
        .funnel_export(&owner, &lead, "journey", &root.join("journey.json"))
        .unwrap();
    let snapshot = sources::retain(
        root,
        "journey.json",
        &fs::read(root.join("journey.json")).unwrap(),
    );
    let manifest = json!({"schema":weekly::SCHEMA,"owner":"operator","period_start":at+10-weekly::WEEK,
        "period_end":at+10,"generated_at":at+10,"journeys":[snapshot],"finance":null,
        "gaps":["synthetic fixture has no qualified payment evidence"]});
    private_json(root, "weekly.json", &manifest);
    drop(store);
    let command = |name: &str, token: &Path, args: &[&str]| {
        let output = Command::new(sales_binary::path())
            .current_dir(root)
            .env("HOME", root)
            .env("OPENAGENTS_TASKS", root.join("tasks"))
            .args(["--json", "sales", name, "--root"])
            .arg(&host)
            .arg("--credential")
            .arg(token)
            .args(args)
            .output()
            .unwrap();
        output
    };
    let options = [
        "--input",
        "weekly.json",
        "--evidence-root",
        root.to_str().unwrap(),
        "--output",
        "private-weekly.json",
    ];
    assert_eq!(command("weekly", &credential, &[]).status.code(), Some(64));
    assert_eq!(
        command("review", &credential, &options).status.code(),
        Some(64)
    );
    let output = command("weekly", &credential, &options);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["published"], false);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private-account"));
    assert_eq!(
        fs::metadata(root.join("private-weekly.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let report_bytes = fs::read(root.join("private-weekly.json")).unwrap();
    let result: weekly::Report = serde_json::from_slice(&report_bytes).unwrap();
    assert_eq!(result.cohorts[0].cumulative.accepted_tasks, 1);
    assert_eq!(result.cohorts[0].cumulative.settled_purchases, 0);
    assert_eq!(result.cohorts[0].cumulative.unknown_purchases, 1);
    assert!(!result.commercial_activation_attested);
    assert!(!command("weekly", &credential, &options).status.success());
    let denied = [
        "--input",
        "weekly.json",
        "--evidence-root",
        root.to_str().unwrap(),
        "--output",
        "forbidden.json",
    ];
    assert!(
        !command("weekly", &writer_credential, &denied)
            .status
            .success()
    );
    assert!(!root.join("forbidden.json").exists());
    let export = command(
        "export",
        &credential,
        &[
            "--lead",
            &lead,
            "--journey",
            "journey",
            "--output",
            "fresh-journey.json",
        ],
    );
    assert!(export.status.success());
    private_json(
        root,
        "review.json",
        &json!({"schema":"openagents.gym.sales-weekly-review.v1","owner":"operator",
        "report_digest":evidence::digest(&report_bytes),"approved":true,"reviewed_at":at+11,"release_at":at+12}),
    );
    let review_options = [
        "--input",
        "weekly.json",
        "--evidence-root",
        root.to_str().unwrap(),
        "--report",
        "private-weekly.json",
        "--review",
        "review.json",
        "--output",
        "counts.json",
    ];
    let output = command("review", &credential, &review_options);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let public = fs::read_to_string(root.join("counts.json")).unwrap();
    assert!(!public.contains("private-account"));
    assert!(!public.contains("private-cohort"));
    assert!(!public.contains("operator"));
    assert!(public.contains("fixture"));
    let mut store = Store::open(&host).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&credential).unwrap())
        .unwrap();
    let revision = store.show(&owner, &lead).unwrap().revision;
    let revoke = json!({"schema":"openagents.sales.pipeline-command.v1","id":"weekly-withdraw","lead":lead,
        "expected_revision":revision,"operation":{"kind":"revoke_funnel_consent","journey":"journey","reference":"synthetic withdrawal"}});
    store
        .apply(&owner, &serde_json::to_vec(&revoke).unwrap())
        .unwrap();
    drop(store);
    let stale = [
        "--input",
        "weekly.json",
        "--evidence-root",
        root.to_str().unwrap(),
        "--output",
        "stale.json",
    ];
    assert!(!command("weekly", &credential, &stale).status.success());
    assert!(!root.join("stale.json").exists());
    assert!(!root.join("host/money").exists());
}
