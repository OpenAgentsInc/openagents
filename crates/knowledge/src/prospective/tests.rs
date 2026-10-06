use super::*;
use secp256k1::SecretKey;
fn keypair(byte: u8) -> Keypair {
    Keypair::from_secret_key(
        &Secp256k1::new(),
        &SecretKey::from_byte_array([byte; 32]).unwrap(),
    )
}
fn doc() -> String {
    "---\nid: shell.quoting\nversion: 1\nkind: method\ntitle: Shell quoting\nsummary: Quote arguments.\ntags: [shell]\napplies_when: Passing arguments.\nstatus: candidate\nauthor: scratch\nprovenance:\n  written_from: [source-task, source-family]\n  cites: [\"Shell manual\"]\nevidence: []\n---\n\n## Details\n\nQuote each argument.\n".into()
}
fn fixture() -> Bundle {
    let operator = keypair(1);
    let evaluator = keypair(2);
    let author = keypair(3);
    let plan = Signed::create(
        Frozen {
            v: PROFILE.into(),
            candidate_digest: digest(doc().as_bytes()),
            candidate_id: "shell.quoting".into(),
            candidate_version: 1,
            author: author.x_only_public_key().0.to_string(),
            operator: operator.x_only_public_key().0.to_string(),
            evaluator: evaluator.x_only_public_key().0.to_string(),
            configuration_digest: digest(b"configuration"),
            source_tasks: vec!["source-task".into()],
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
    let review =
        Signed::create(review_draft(&plan, &report, &doc(), 40).unwrap(), &operator).unwrap();
    Bundle {
        plan,
        report,
        review: Some(review),
        retirements: Vec::new(),
    }
}
fn trust() -> Trust {
    Trust {
        operator: keypair(1).x_only_public_key().0.to_string(),
        evaluator: keypair(2).x_only_public_key().0.to_string(),
    }
}
fn grant() -> ActivationGrant {
    ActivationGrant {
        operator: keypair(1).x_only_public_key().0.to_string(),
        scope: "scratch-shell-only".into(),
        evaluator: trust().evaluator,
        expires_at: 100,
        candidate_digest: digest(doc().as_bytes()),
        configuration_digest: digest(b"configuration"),
        admission_digest: fixture().review.unwrap().digest().unwrap(),
        retired_admissions: BTreeSet::new(),
        instructions: true,
        disclosure: true,
    }
}
fn resign_plan(bundle: &mut Bundle) {
    bundle.plan = Signed::create(bundle.plan.record.clone(), &keypair(1)).unwrap();
    bundle.report.record.plan_digest = bundle.plan.digest().unwrap();
    resign_report(bundle);
}
fn resign_report(bundle: &mut Bundle) {
    for trial in &mut bundle.report.record.trials {
        trial.seal_receipt().unwrap();
    }
    bundle.report = Signed::create(bundle.report.record.clone(), &keypair(2)).unwrap();
    bundle.review = None;
}
#[test]
fn reviewed_fixture_replays_exact_evidence_and_expires_without_rewriting_history() {
    let bundle = fixture();
    assert_eq!(bundle.state(&doc(), 50, &trust()).unwrap(), State::Admitted);
    assert_eq!(
        bundle.activate(&doc(), &grant(), 50).unwrap().guidance(),
        "## Details\n\nQuote each argument."
    );
    let path =
        std::env::temp_dir().join(format!("knowledge-prospective-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    bundle.save(&path).unwrap();
    assert!(bundle.save(&path).is_err());
    let replay = Bundle::read(&path).unwrap();
    assert_eq!(replay.state(&doc(), 50, &trust()).unwrap(), State::Admitted);
    assert_eq!(replay.state(&doc(), 100, &trust()).unwrap(), State::Retired);
    assert!(replay.activate(&doc(), &grant(), 100).is_err());
    assert_eq!(
        replay.review.unwrap().digest().unwrap(),
        bundle.review.unwrap().digest().unwrap()
    );
    std::fs::remove_file(path).unwrap();
}
#[test]
fn signed_retirement_removes_activation_and_preserves_candidate_and_report() {
    let mut bundle = fixture();
    let report = bundle.report.digest().unwrap();
    bundle.retirements.push(
        Signed::create(
            Retirement {
                v: "openagents.knowledge-retirement.v1".into(),
                admission_digest: bundle.review.as_ref().unwrap().digest().unwrap(),
                retired_at: 45,
                reason: "Withdrawn after owner inspection".into(),
            },
            &keypair(1),
        )
        .unwrap(),
    );
    assert_eq!(bundle.state(&doc(), 50, &trust()).unwrap(), State::Retired);
    assert!(bundle.activate(&doc(), &grant(), 50).is_err());
    assert_eq!(bundle.report.digest().unwrap(), report);
    assert_eq!(
        bundle.plan.record.candidate_digest,
        digest(doc().as_bytes())
    );
}
#[test]
fn frozen_profile_refuses_source_leakage_partition_overlap_and_changed_candidate() {
    let original = fixture();
    assert!(
        original
            .state(
                &doc().replace("each argument", "every argument"),
                50,
                &trust()
            )
            .is_err()
    );
    let mut bundle = fixture();
    bundle.plan.record.cases[1].task = "source-task".into();
    resign_plan(&mut bundle);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("Source leakage")
    );
    let mut bundle = fixture();
    bundle.plan.record.cases[1].group = "source-family".into();
    resign_plan(&mut bundle);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("Source leakage")
    );
    let mut bundle = fixture();
    bundle.plan.record.cases[1].group = "development-family".into();
    resign_plan(&mut bundle);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("overlap")
    );
    let mut bundle = fixture();
    bundle.plan.record.source_tasks.clear();
    resign_plan(&mut bundle);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("omits")
    );
}
#[test]
fn complete_denominators_costs_and_actual_loaded_candidate_are_required() {
    let mut bundle = fixture();
    bundle.report.record.trials.pop();
    resign_report(&mut bundle);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("denominator")
    );
    let mut bundle = fixture();
    bundle
        .report
        .record
        .costs
        .insert("acquisition_usd".into(), None);
    resign_report(&mut bundle);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("Unknown")
    );
    let mut bundle = fixture();
    bundle.report.record.trials[0].loaded_candidate = Some(digest(b"different"));
    resign_report(&mut bundle);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("Changed loaded")
    );
    let mut bundle = fixture();
    bundle.report.record.trials[0].passed = None;
    resign_report(&mut bundle);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("Unknown assigned")
    );
    let mut bundle = fixture();
    bundle.report.record.trials[0].started_at = 5;
    resign_report(&mut bundle);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("timing")
    );
    let mut bundle = fixture();
    bundle.report.record.trials[0].passed = Some(false);
    resign_report(&mut bundle);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("failed")
    );
}
#[test]
fn self_evidence_unreviewed_and_tampered_promotion_refuse() {
    let mut bundle = fixture();
    bundle.plan.record.author = bundle.plan.record.evaluator.clone();
    resign_plan(&mut bundle);
    assert!(bundle.state(&doc(), 50, &trust()).is_err());
    let mut bundle = fixture();
    bundle.review = None;
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("review")
    );
    let mut bundle = fixture();
    bundle.report.record.trials[0].passed = Some(false);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("signature")
    );
    let mut bundle = fixture();
    bundle.review.as_mut().unwrap().record.admission["scope"]["digest"] =
        json!(digest(b"different"));
    bundle.review =
        Some(Signed::create(bundle.review.take().unwrap().record, &keypair(1)).unwrap());
    assert!(bundle.state(&doc(), 50, &trust()).is_err());
    let bundle = fixture();
    let mut permissions = grant();
    permissions.disclosure = false;
    assert!(bundle.activate(&doc(), &permissions, 50).is_err());
    permissions = grant();
    permissions.scope = "other".into();
    assert!(bundle.activate(&doc(), &permissions, 50).is_err());
}
#[test]
fn local_study_or_historical_evidence_cannot_become_reviewed_admission() {
    let path =
        std::env::temp_dir().join(format!("knowledge-local-study-{}.json", std::process::id()));
    std::fs::write(
        &path,
        r#"{"schema":"openagents.kb-study.v1","results":[{"reward":1}]}"#,
    )
    .unwrap();
    assert!(Bundle::read(&path).is_err());
    std::fs::remove_file(path).unwrap();
    let bundle = fixture();
    let entry = Entry::parse(&doc()).unwrap();
    assert!(crate::evidence::review(&[entry], &[]).is_empty());
    assert_eq!(
        bundle.plan.record.candidate_digest,
        digest(doc().as_bytes())
    );
}
#[test]
fn negative_unknown_receipts_remain_retained_without_admission() {
    let mut bundle = fixture();
    bundle.report.record.trials[0].passed = None;
    bundle.report.record.costs.insert("setup_usd".into(), None);
    resign_report(&mut bundle);
    let path = std::env::temp_dir().join(format!(
        "knowledge-unknown-report-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    bundle.save(&path).unwrap();
    let replay = Bundle::read(&path).unwrap();
    assert_eq!(replay.report.record.trials.len(), 4);
    assert_eq!(replay.report.record.trials[0].passed, None);
    assert_eq!(replay.report.record.costs["setup_usd"], None);
    assert!(replay.state(&doc(), 50, &trust()).is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn current_owner_retirement_index_blocks_rollback_to_an_old_bundle() {
    let old = fixture();
    let mut current = grant();
    current
        .retired_admissions
        .insert(old.review.as_ref().unwrap().digest().unwrap());
    assert!(old.activate(&doc(), &current, 50).is_err());
}
#[test]
fn workbench_projection_cannot_activate_or_relabel_a_changed_candidate() {
    use ::workbench::pane::{PaneAdapter, Subject};
    let selection = crate::workbench::Selection {
        sources: vec![crate::workbench::Source {
            task: "source-task".into(),
            run: "source-task".into(),
            group: "source-family".into(),
            artifact: digest(b"source"),
            citation: "Shell manual".into(),
            disclosed: "Quoting guidance".into(),
        }],
        forbidden: Vec::new(),
        costs: BTreeMap::from([
            ("acquisition_usd".into(), None),
            ("setup_usd".into(), None),
            ("checks_usd".into(), None),
        ]),
    };
    let mut session = crate::workbench::Session::new(selection).unwrap();
    session.candidates.push(crate::workbench::Candidate {
        revision: 1,
        bytes: doc(),
        digest: digest(doc().as_bytes()),
        problems: Vec::new(),
    });
    let host = ::workbench::Host::Local {
        instance: "11".repeat(32),
    };
    let mut adapter = Adapter {
        candidate: crate::workbench::Adapter {
            id: "lesson".into(),
            host: host.clone(),
            session,
        },
        evidence: fixture(),
        trust: trust(),
    };
    let subject = Subject::Record {
        host,
        id: "lesson".into(),
        revision: None,
    };
    let view = adapter.describe(&subject);
    assert!(view.title.starts_with("Knowledge:"));
    assert!(!view.actions.contains(&"activate".into()));
    adapter.candidate.session.candidates[0]
        .bytes
        .push_str("\nChanged candidate.\n");
    let view = adapter.describe(&subject);
    assert_eq!(view.title, "Knowledge: Candidate");
    assert!(view.detail.contains("inconclusive"));
}

#[test]
fn publisher_cannot_choose_reader_trust() {
    let bundle = fixture();
    let reader = Trust {
        operator: keypair(4).x_only_public_key().0.to_string(),
        evaluator: trust().evaluator,
    };
    assert!(
        bundle
            .state(&doc(), 50, &reader)
            .unwrap_err()
            .contains("Reader does not trust")
    );
    let mut current = grant();
    current.evaluator = keypair(4).x_only_public_key().0.to_string();
    assert!(bundle.activate(&doc(), &current, 50).is_err());
}

#[test]
fn relabeling_source_groups_as_tasks_cannot_bypass_exclusion() {
    let mut bundle = fixture();
    bundle.plan.record.source_tasks.push("source-family".into());
    bundle.plan.record.source_groups.clear();
    bundle.plan.record.cases[1].group = "source-family".into();
    resign_plan(&mut bundle);
    assert!(
        bundle
            .state(&doc(), 50, &trust())
            .unwrap_err()
            .contains("Source leakage")
    );
}

#[test]
fn changed_runtime_configuration_requires_new_admission() {
    let bundle = fixture();
    let mut current = grant();
    current.configuration_digest = digest(b"new model configuration");
    assert!(bundle.activate(&doc(), &current, 50).is_err());
}
