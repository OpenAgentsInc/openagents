use super::*;
use openagents_chat::{
    client::Ran,
    plugin_flow::{Flow, Step, Test},
    plugin_workbench::{Declarations, Engine, Request, Source},
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use xp_ledger::eval::fixture::{self, Run};
#[derive(Clone)]
struct Fake {
    failed: Arc<AtomicBool>,
}
impl Engine for Fake {
    fn tests(&self, _: &Path) -> Result<Vec<Test>, String> {
        Ok(vec![Test {
            name: "quote".into(),
            kind: "should-fire".into(),
            task: "Quote Ada".into(),
        }])
    }
    fn run(&self, args: &[String], _: std::time::Duration) -> Result<Ran, String> {
        if let Some(at) = args.iter().position(|a| a == "--output-dir") {
            let into = PathBuf::from(&args[at + 1]).join("run1");
            std::fs::create_dir_all(&into).unwrap();
            let mut report: serde_json::Value = serde_json::from_str(include_str!(
                "../../../ext-eval/tests/fixtures/expected/better.report.json"
            ))
            .unwrap();
            let bytes = std::fs::read(Path::new(&args[3]).join("package.json")).unwrap();
            report["subject"]["definition"]["id"] =
                json!(format!("{}:tool/main", fixture::pubkey("author")));
            report["subject"]["definition"]["artifact"]["digest"] =
                json!(route_contract::digest::Digest::of_bytes(&bytes).as_str());
            std::fs::write(
                into.join("report.json"),
                serde_json::to_vec(&report).unwrap(),
            )
            .unwrap();
        }
        let output = if args.get(1).is_some_and(|a| a == "use") {
            json!({"v":"openagents.plugin-use.v1","pin":{"id":args[2],"version":args[4],"digest":args[6]},"thread":args[10],"request":"use-distinct","dispatched":"ran","outputs":[{"digest":"sha256:fixture"}]}).to_string()
        } else {
            "comparison fixture".into()
        };
        Ok(Ran {
            ok: !self.failed.load(Ordering::Relaxed),
            output,
        })
    }
}
fn private(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}
fn fixture() -> (
    tempfile::TempDir,
    Config,
    Owner<Fake>,
    openagents_chat::plugin_workbench::Record,
    Fake,
) {
    let tmp = tempfile::tempdir().unwrap();
    let draft = tmp.path().join("draft");
    std::fs::create_dir_all(draft.join("skills")).unwrap();
    private(&draft.join("skills/tool.md"), b"Quote each argument.");
    private(&draft.join("README.md"), b"Quote safely.");
    private(
        &draft.join("package.json"),
        json!({"v":1,"slug":"tool","version":"1","publisher":fixture::pubkey("author")})
            .to_string()
            .as_bytes(),
    );
    let fake = Fake {
        failed: Arc::new(AtomicBool::new(false)),
    };
    let root = tmp.path().join("owner");
    let owner = Owner::open(root.clone(), fake.clone());
    let record = owner
        .freeze(
            Source {
                flow: "flow1".into(),
                thread: "author-thread".into(),
                task: "author-task".into(),
            },
            Flow::at(Step::Tests, Some("tool".into())),
            &draft,
            Declarations {
                author: fixture::pubkey("author"),
                fee_msat: None,
                payout: None,
            },
        )
        .unwrap();
    let config = Config {
        plugins: vec![root],
        knowledge: vec![],
        reviews: vec![],
        events: vec![],
        documents: vec![],
        operators: BTreeSet::new(),
        evaluators: BTreeSet::new(),
        referees: BTreeSet::new(),
        ledger: None,
    };
    (tmp, config, owner, record, fake)
}
fn apply(
    owner: &Owner<Fake>,
    r: &openagents_chat::plugin_workbench::Record,
    id: &str,
    action: Action,
) {
    owner
        .apply(Request {
            id: id.into(),
            source: r.source.clone(),
            release: r.release.clone(),
            tree: r.tree.clone(),
            action,
        })
        .unwrap();
}
#[test]
fn retained_owner_keeps_failures_and_actual_reuse_without_inventing_credit() {
    let (tmp, config, owner, r, fake) = fixture();
    fake.failed.store(true, Ordering::Relaxed);
    apply(&owner, &r, "failed", Action::Compare);
    fake.failed.store(false, Ordering::Relaxed);
    apply(&owner, &r, "better", Action::Compare);
    apply(&owner, &r, "publish", Action::Publish);
    apply(&owner, &r, "install", Action::Install);
    apply(&owner, &r, "enable", Action::Enable);
    apply(
        &owner,
        &r,
        "reuse",
        Action::Reuse {
            source_task: "distinct-work".into(),
            thread: "reuse-thread".into(),
            request: "reuse-question".into(),
            workspace: tmp.path().into(),
        },
    );
    let first = config.read(5000).unwrap();
    assert!(first[0].authored.available);
    assert!(first[0].published.available);
    assert!(first[0].installed.available);
    assert!(first[0].invoked.available);
    assert_eq!(first[0].invoked.references, ["use-distinct"]);
    assert!(!first[0].validated.available);
    assert!(!first[0].adopted.available);
    assert!(!first[0].credited.available);
    assert!(!first[0].settled.available);
    assert!(first[0].attempts.iter().any(|a| a.contains("failed")));
    assert_eq!(first, config.read(5000).unwrap());
    private(
        &config.plugins[0].join("draft/skills/tool.md"),
        b"Changed version",
    );
    assert!(config.read(5000).is_err());
}
#[test]
fn independent_signed_validation_binds_exact_frozen_release_and_keeps_inconclusive() {
    let (tmp, mut config, owner, r, fake) = fixture();
    fake.failed.store(true, Ordering::Relaxed);
    apply(&owner, &r, "failed", Action::Compare);
    fake.failed.store(false, Ordering::Relaxed);
    for (id, action) in [
        ("better", Action::Compare),
        ("publish", Action::Publish),
        ("install", Action::Install),
        ("enable", Action::Enable),
    ] {
        apply(&owner, &r, id, action);
    }
    apply(
        &owner,
        &r,
        "reuse",
        Action::Reuse {
            source_task: "distinct-work".into(),
            thread: "reuse-thread".into(),
            request: "reuse-question".into(),
            workspace: tmp.path().into(),
        },
    );
    let author = fixture::signer("author");
    let files = owner.reviewed_files().unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(
        &xp_ledger::adopt::manifest(&format!("{}:tool", author.pubkey()), "1", &[], &[]).unwrap(),
    )
    .unwrap();
    manifest["files"]=json!(files.iter().map(|(name,b)|json!({"path":name,"digest":nostr::contracts::digest_bytes(b),"size":b.len(),"media_type":"text/plain"})).collect::<Vec<_>>());
    let manifest = nostr::contracts::jcs(&manifest).unwrap();
    let parts = xp_ledger::adopt::release(&manifest).unwrap();
    let subject = author.sign(100, parts.kind, parts.tags, parts.content);
    let suite = fixture::release(&fixture::signer("suite-author"), "suite", 50);
    let second_suite = fixture::release(&fixture::signer("second-suite"), "second", 200);
    let result = fixture::published(
        &fixture::signer("evaluator"),
        &Run::better(&suite, &subject),
        None,
        2100,
    );
    let validation = fixture::published_citing(
        &fixture::signer("checker"),
        &Run::better(&second_suite, &subject),
        Some(eval_ext::Cites::Validates(&result.id)),
        2200,
    );
    let mut run = Run::better(&suite, &subject);
    run.verdict = "inconclusive";
    let inconclusive = fixture::published(&fixture::signer("evaluator"), &run, None, 2300);
    for (i, event) in [
        subject.clone(),
        suite.clone(),
        second_suite,
        result.clone(),
        validation.clone(),
        inconclusive,
    ]
    .into_iter()
    .enumerate()
    {
        let path = tmp.path().join(format!("event{i}.json"));
        private(&path, &serde_json::to_vec(&event).unwrap());
        config.events.push(path);
    }
    let path = tmp.path().join("manifest.json");
    private(&path, &manifest);
    config.documents.push(path);
    let rows = config.read(5000).unwrap();
    assert!(rows[0].validated.available);
    assert!(!config.read(2000).unwrap()[0].validated.available);
    assert!(
        config.read(2000).unwrap()[0]
            .limitations
            .iter()
            .any(|s| s.contains("future-dated"))
    );
    assert!(!rows[0].credited.available);
    assert!(rows[0].attempts.iter().any(|s| s.contains("Inconclusive")));
    let operator = fixture::signer("operator");
    let referee = fixture::signer("referee");
    let admission =
        xp_ledger::adopt::admission(operator.pubkey(), &[&result], &[&validation], 9000).unwrap();
    let defaults_manifest = xp_ledger::adopt::manifest(
        &xp_ledger::adopt::package_of(operator.pubkey()),
        "1",
        &[subject.id.clone()],
        &[xp_ledger::adopt::receipt(&admission)],
    )
    .unwrap();
    let parts = xp_ledger::adopt::release(&defaults_manifest).unwrap();
    let defaults = operator.sign(2400, parts.kind, parts.tags, parts.content);
    let parts = xp::quest(&json!({"id":"adopt-tool","version":1,"season":{"id":"fixture","opens_at":0,"closes_at":10000},"title":"Adopt tool","objective":"Adopt checked tool","acceptance":{"rule":"eval-adopt","defaults":xp_ledger::adopt::package_of(operator.pubkey()),"subject":{"id":subject.id,"pubkey":subject.pubkey,"kind":subject.kind}},"reference":null,"award":{"extension-author":200,"suite-author":100,"evaluator":50}})).unwrap();
    let quest = referee.sign(1, parts.kind, parts.tags, parts.content);
    let mut checks: Vec<Event> = ["check-one", "check-two", "check-three"]
        .into_iter()
        .map(|label| {
            fixture::published(
                &fixture::signer(label),
                &Run::better(&suite, &subject),
                Some(&result.id),
                2300,
            )
        })
        .collect();
    let mut publications = checks.clone();
    publications.extend([result.clone(), validation.clone()]);
    let adoption = xp::Adoption {
        release: &defaults,
        manifest: &defaults_manifest,
        admission: &admission,
        results: &publications,
        checks: &publications,
        requests: &[],
    };
    let awards: Vec<Event> = xp::eval_adopt_awards(&quest, &adoption, 2500)
        .unwrap()
        .into_iter()
        .map(|p| referee.sign(2500, p.kind, p.tags, p.content))
        .collect();
    checks.extend([defaults.clone(), quest]);
    checks.extend(awards.clone());
    for (i, event) in checks.into_iter().enumerate() {
        let path = tmp.path().join(format!("adopt{i}.json"));
        private(&path, &serde_json::to_vec(&event).unwrap());
        config.events.push(path);
    }
    for (i, doc) in [admission, defaults_manifest].into_iter().enumerate() {
        let path = tmp.path().join(format!("adopt-document{i}.json"));
        private(&path, &doc);
        config.documents.push(path);
    }
    config.operators.insert(operator.pubkey().into());
    config.referees.insert(referee.pubkey().into());
    let rows = config.read(5000).unwrap();
    assert!(rows[0].adopted.available);
    assert!(
        rows[0].authored.available
            && rows[0].published.available
            && rows[0].installed.available
            && rows[0].invoked.available
            && rows[0].validated.available
    );
    assert!(rows[0].attempts.iter().any(|a| a.contains("failed")));
    assert!(rows[0].credited.available);
    assert!(
        rows[0]
            .credited
            .references
            .iter()
            .any(|s| s.contains("extension-author"))
    );
    assert!(!rows[0].settled.available);
    assert!(!config.read(9500).unwrap()[0].adopted.available);
    let author_award = awards
        .iter()
        .find(|event| {
            xp::parse_award(event)
                .unwrap()
                .awardees
                .iter()
                .any(|awardee| awardee.role == "extension-author")
        })
        .unwrap();
    let parts = xp::revocation(author_award, "Scratch revocation").unwrap();
    let event = referee.sign(2600, parts.kind, parts.tags, parts.content);
    let path = tmp.path().join("revocation.json");
    private(&path, &serde_json::to_vec(&event).unwrap());
    config.events.push(path);
    let rows = config.read(5000).unwrap();
    assert!(
        !rows[0]
            .credited
            .references
            .iter()
            .any(|r| r.contains(&author_award.id))
    );
    assert!(
        rows[0]
            .limitations
            .iter()
            .any(|r| r.contains(&author_award.id))
    );
    use pay_ledger::{Ledger, Payee, PayoutState, Rail, SettlementInput, Split};
    let path = tmp.path().join("ledger.sqlite");
    let mut payments = Ledger::open(&path).unwrap();
    let party = author.pubkey().to_owned();
    payments
        .register_payee(Payee {
            party: party.clone(),
            destination_kind: "spark".into(),
            destination_value: "scratch-destination".into(),
            source: "fixture".into(),
            verified_at: 2600,
        })
        .unwrap();
    payments
        .record_settlement(SettlementInput {
            key: "actual-call".into(),
            resource: "route-use-distinct".into(),
            plugin_id: Some(rows[0].identity.clone()),
            release_id: Some(subject.id.clone()),
            price_msat: 2000,
            received_msat: 2000,
            rail: Rail::Lightning,
            payer_alias: Some("PRIVATE-PAYER".into()),
            settled_at: 1_792_022_400,
            split: Split::Plugin {
                author: party.clone(),
                fee_msat: 1000,
            },
        })
        .unwrap();
    let share = payments
        .available_shares(&party)
        .unwrap()
        .into_iter()
        .find(|s| s.role == "author")
        .unwrap();
    payments
        .reserve_payout("payout-exact", &party, &[share], 2601)
        .unwrap();
    payments
        .begin_send("payout-exact", "wallet-ref", None, 1000, 2602)
        .unwrap();
    payments
        .finish_payout("payout-exact", PayoutState::Sent, Some(0), None, 2603)
        .unwrap();
    drop(payments);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    config.ledger = Some(path.clone());
    let before = std::fs::read(&path).unwrap();
    let rows = config.read(5000).unwrap();
    assert!(rows[0].settled.available);
    for label in [
        "Authored:",
        "Published:",
        "Installed:",
        "Invoked:",
        "Externally validated:",
        "Adopted:",
        "Credited:",
        "Settled:",
        "Attempt: failed",
        "acquisition_usd: unknown",
    ] {
        assert!(rows[0].lines().contains(label), "missing {label}");
    }
    assert!(rows[0].settled.references[0].contains("payout-exact"));
    assert!(!rows[0].lines().contains("PRIVATE-PAYER"));
    assert_eq!(before, std::fs::read(path).unwrap());

    let withdrawn = xp_ledger::adopt::manifest(
        &xp_ledger::adopt::package_of(operator.pubkey()),
        "2",
        &[],
        &[],
    )
    .unwrap();
    let p = xp_ledger::adopt::release(&withdrawn).unwrap();
    let event = operator.sign(2600, p.kind, p.tags, p.content);
    let path = tmp.path().join("withdrawn.json");
    private(&path, &serde_json::to_vec(&event).unwrap());
    config.events.push(path);
    let path = tmp.path().join("withdrawn-manifest.json");
    private(&path, &withdrawn);
    config.documents.push(path);
    assert!(!config.read(5000).unwrap()[0].adopted.available);
    let mut changed: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    changed["version"] = json!("2");
    private(&config.documents[0], &serde_json::to_vec(&changed).unwrap());
    assert!(!config.read(5000).unwrap()[0].validated.available);
}
#[test]
fn malformed_and_public_sources_are_refused_and_display_is_bounded() {
    let (tmp, mut config, _, _, _) = fixture();
    let path = tmp.path().join("bad.json");
    private(&path, b"{}");
    config.events.push(path);
    assert!(config.read(5000).is_err());
    config.events.clear();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            config.plugins[0].join("record.json"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(config.read(5000).is_err());
    }
    let mut row = empty("a".into(), "1".into(), "digest".into(), "author".into());
    row.attempts.push("x".repeat(10000));
    assert!(row.lines().len() <= 2048);
    assert!(row.lines().contains("truncated"));
}

use knowledge::digest;
use knowledge::prospective::{
    Bundle, Frozen, POLICY, PROFILE, Report, Signed, Trial, review_draft,
};
use knowledge::study::{Arm, Case, Partition};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde_json::Value;
use std::collections::BTreeMap;
fn keypair(byte: u8) -> Keypair {
    Keypair::from_secret_key(
        &Secp256k1::new(),
        &SecretKey::from_byte_array([byte; 32]).unwrap(),
    )
}
fn knowledge_doc() -> String {
    "---\nid: shell.quoting\nversion: 1\nkind: method\ntitle: Shell quoting\nsummary: Quote arguments.\ntags: [shell]\napplies_when: Passing arguments.\nstatus: candidate\nauthor: scratch\nprovenance:\n  written_from: [source-task, source-family]\n  cites: [\"Shell manual\"]\nevidence: []\n---\n\n## Details\n\nQuote each argument.\n".into()
}
fn knowledge_bundle(document: &str) -> Bundle {
    let operator = keypair(1);
    let evaluator = keypair(2);
    let author = keypair(3);
    let plan = Signed::create(
        Frozen {
            v: PROFILE.into(),
            candidate_digest: digest(document.as_bytes()),
            candidate_id: "shell.quoting".into(),
            candidate_version: 1,
            author: author.x_only_public_key().0.to_string(),
            operator: operator.x_only_public_key().0.to_string(),
            evaluator: evaluator.x_only_public_key().0.to_string(),
            configuration_digest: digest(b"configuration"),
            source_tasks: vec!["source-task".into(), "run-source".into()],
            source_groups: vec!["source-family".into()],
            cases: vec![
                Case {
                    task: "development-task".into(),
                    group: "development-family".into(),
                    partition: Partition::Development,
                    workload_digest: digest(b"development"),
                    environment_digest: digest(b"environment"),
                },
                Case {
                    task: "confirmation-task".into(),
                    group: "confirmation-family".into(),
                    partition: Partition::Confirmation,
                    workload_digest: digest(b"confirmation"),
                    environment_digest: digest(b"environment"),
                },
            ],
            committed_at: 10,
            expires_at: 100,
            max_total_usd: 1.0,
            scope: "scratch-shell-only".into(),
            policy_digest: digest(POLICY.as_bytes()),
        },
        &operator,
    )
    .unwrap();
    let mut trials = Vec::new();
    for case in &plan.record.cases {
        for arm in [Arm::Subject, Arm::Baseline] {
            trials.push(Trial {
                task: case.task.clone(),
                arm,
                started_at: 20,
                finished_at: 30,
                loaded_candidate: match arm {
                    Arm::Subject => Some(plan.record.candidate_digest.clone()),
                    Arm::Baseline => None,
                },
                configuration_digest: plan.record.configuration_digest.clone(),
                workload_digest: case.workload_digest.clone(),
                environment_digest: case.environment_digest.clone(),
                receipt: Value::Null,
                receipt_digest: digest(format!("{}-{arm:?}", case.task).as_bytes()),
                passed: Some(arm == Arm::Subject),
                charged_usd: Some(0.01),
            });
        }
    }
    for trial in &mut trials {
        trial.seal_receipt().unwrap();
    }
    let report = Signed::create(
        Report {
            v: "openagents.knowledge-paired-report.v1".into(),
            plan_digest: plan.digest().unwrap(),
            completed_at: 30,
            trials,
            costs: BTreeMap::from([
                ("acquisition_usd".into(), Some(0.01)),
                ("setup_usd".into(), Some(0.01)),
                ("checks_usd".into(), Some(0.01)),
                ("runtime_usd".into(), Some(0.04)),
            ]),
            limitations: vec![
                "Synthetic fixed cases establish no population inference or live transfer".into(),
            ],
        },
        &evaluator,
    )
    .unwrap();
    let review = Signed::create(
        review_draft(&plan, &report, document, 40).unwrap(),
        &operator,
    )
    .unwrap();
    Bundle {
        plan,
        report,
        review: Some(review),
        retirements: Vec::new(),
    }
}

#[test]
fn retained_knowledge_keeps_costs_and_signed_retirement_separate_from_check() {
    let tmp = tempfile::tempdir().unwrap();
    let selection = knowledge::workbench::Selection {
        sources: vec![knowledge::workbench::Source {
            task: "source-task".into(),
            run: "run-source".into(),
            group: "source-family".into(),
            artifact: digest(b"source"),
            citation: "Shell manual".into(),
            disclosed: "private selected input".into(),
        }],
        forbidden: vec!["DO-NOT-DISCLOSE".into()],
        costs: BTreeMap::from([
            ("acquisition_usd".into(), None),
            ("setup_usd".into(), Some(0.01)),
            ("checks_usd".into(), None),
        ]),
    };
    let mut session = knowledge::workbench::Session::new(selection).unwrap();
    session
        .edit(&knowledge_doc(), &knowledge::lint::Corpus::default())
        .unwrap();
    let candidate = session.candidates[0].bytes.clone();
    let mut bundle = knowledge_bundle(&candidate);
    let session_path = tmp.path().join("session.json");
    session.save(&session_path).unwrap();
    let review_path = tmp.path().join("review.json");
    bundle.save(&review_path).unwrap();
    let mut config = Config {
        plugins: vec![],
        knowledge: vec![session_path],
        reviews: vec![review_path.clone()],
        events: vec![],
        documents: vec![],
        operators: BTreeSet::from([bundle.plan.record.operator.clone()]),
        evaluators: BTreeSet::from([bundle.plan.record.evaluator.clone()]),
        referees: BTreeSet::new(),
        ledger: None,
    };
    let rows = config.read(50).unwrap();
    assert!(rows[0].adopted.available);
    assert!(rows[0].validated.available);
    assert!(!rows[0].credited.available);
    assert!(!rows[0].settled.available);
    assert!(rows[0].costs.iter().any(|s| s.contains("unknown")));
    assert!(!rows[0].lines().contains("private selected input"));
    assert!(!rows[0].lines().contains("Shell manual"));
    let mut check_only = bundle.clone();
    check_only.review = None;
    let check_path = tmp.path().join("check-only.json");
    check_only.save(&check_path).unwrap();
    config.reviews = vec![check_path];
    let checked = config.read(50).unwrap();
    assert!(checked[0].validated.available);
    assert!(!checked[0].adopted.available);
    config.reviews = vec![review_path.clone()];
    bundle.retirements.push(
        Signed::create(
            knowledge::prospective::Retirement {
                v: "openagents.knowledge-retirement.v1".into(),
                admission_digest: bundle.review.as_ref().unwrap().digest().unwrap(),
                retired_at: 60,
                reason: "Scratch retirement".into(),
            },
            &keypair(1),
        )
        .unwrap(),
    );
    let retired_path = tmp.path().join("retired-review.json");
    bundle.save(&retired_path).unwrap();
    config.reviews = vec![review_path.clone(), retired_path.clone()];
    let rows = config.read(70).unwrap();
    assert!(!rows[0].adopted.available);
    assert!(rows[0].validated.available);
    assert!(rows[0].limitations.iter().any(|s| s.contains("Retired")));
    config.reviews.reverse();
    assert!(!config.read(70).unwrap()[0].adopted.available);
    config.operators.clear();
    assert!(!config.read(70).unwrap()[0].adopted.available);
}

#[test]
fn exact_version_panes_bind_each_retained_source_and_revision() {
    let (_first, mut config, _, _, _) = fixture();
    let (_second, other, _, _, _) = fixture();
    config.plugins.extend(other.plugins);
    let rows = config.read(5000).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].identity, rows[1].identity);
    assert_ne!(id(&rows[0]), id(&rows[1]));
    let host = Host::Local {
        instance: "ab".repeat(32),
    };
    let adapter = Adapter {
        config,
        host: host.clone(),
    };
    let subject = Subject::Record {
        host: host.clone(),
        id: id(&rows[1]),
        revision: None,
    };
    assert_eq!(adapter.describe(&subject).state, PaneState::Ready);
    let stale = Subject::Record {
        host,
        id: id(&rows[1]),
        revision: Some(Revision::Sha256("cd".repeat(32))),
    };
    assert!(matches!(
        adapter.describe(&stale).state,
        PaneState::Stale { .. }
    ));
}

#[test]
fn copied_source_map_key_and_traversal_attempts_are_refused() {
    let (_tmp, config, owner, record, _) = fixture();
    apply(&owner, &record, "compare", Action::Compare);
    let path = config.plugins[0].join("record.json");
    let retained = owner.read().unwrap();
    let mut changed = retained.clone();
    changed
        .attempts
        .get_mut("compare")
        .unwrap()
        .request
        .source
        .task = "other-task".into();
    private(&path, &serde_json::to_vec(&changed).unwrap());
    assert!(config.read(5000).is_err());
    let mut changed = retained.clone();
    changed.attempts.get_mut("compare").unwrap().request.id = "other-id".into();
    private(&path, &serde_json::to_vec(&changed).unwrap());
    assert!(config.read(5000).is_err());
    let mut changed = retained;
    let mut attempt = changed.attempts.remove("compare").unwrap();
    attempt.request.id = "../escape".into();
    changed.attempts.insert(attempt.request.id.clone(), attempt);
    private(&path, &serde_json::to_vec(&changed).unwrap());
    assert!(config.read(5000).is_err());
}
