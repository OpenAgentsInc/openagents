use super::*;

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("kb-study-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir(&path).unwrap();
    path
}
fn candidate() -> Vec<u8> {
    let doc = "---\nid: e.one\nversion: 1\nkind: method\ntitle: T\nsummary: S.\ntags: [x]\napplies_when: A.\nstatus: admitted\nauthor: local\nprovenance:\n  written_from: [reference]\n  cites: [Book]\nevidence: []\n---\n\n## Details\n\nBody.\n";
    let signer = nostr::domain::RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
    let bundle =
        crate::snapshot::create(&[doc.to_string()], &signer, "candidate", "1", "CC0-1.0", 1)
            .unwrap();
    serde_json::to_vec(&bundle).unwrap()
}
fn plan() -> Plan {
    Plan {
        schema: SCHEMA.into(),
        study: "frozen-comparison".into(),
        owner: "local:operator".into(),
        binary: PathBuf::from("/usr/bin/example"),
        binary_digest: digest(b"binary"),
        tasks_root: PathBuf::from("/tasks"),
        candidate_digest: digest(&candidate()),
        configuration: Configuration {
            provider: "codex".into(),
            cost_basis: "list_price".into(),
            retrieval_mode: "lexical".into(),
            decision_base_url: "https://api.typesafe.ai".into(),
            decision_model: "jev-test-version".into(),
            model: "example/model-v1".into(),
            effort: "medium".into(),
            strong_model: "example/strong-v1".into(),
            max_steps: 10,
            max_seconds: 60,
            max_usd: 0.2,
            command_seconds: 5,
            test_seconds: 5,
            prompt: "Solve this task.".into(),
            network: "none".into(),
            acceptance: true,
        },
        cases: vec![Case {
            task: "task-a".into(),
            group: "group-a".into(),
            partition: Partition::Confirmation,
            workload_digest: digest(b"workload"),
            environment_digest: digest(b"environment"),
        }],
        repetitions: 1,
        first_subject: true,
        source_tasks: Vec::new(),
        max_total_usd: 0.4,
    }
}
fn finish(store: &Store, started: &Started) -> Finished {
    Finished {
        schema: "openagents.kb-study-finish.v1".into(),
        plan_digest: store.plan_digest.clone(),
        assignment: started.assignment.id.clone(),
        started_digest: started_digest(started).unwrap(),
        finished_at_ms: started.started_at_ms,
        exit_code: Some(1),
        ending: "exit 1".into(),
        wall_seconds: 1.0,
        summary_digest: None,
        events_digest: None,
        reward: None,
        costs: BTreeMap::from([
            ("model_usd".into(), None),
            ("jev_usd".into(), None),
            ("embedding_usd".into(), None),
        ]),
        known_cost_lower_bound_usd: 0.0,
        evidence: BTreeMap::new(),
        served_models: Vec::new(),
        problems: vec!["summary unavailable".into()],
    }
}
#[test]
fn frozen_plan_refuses_duplicate_and_cross_partition_groups() {
    let mut p = plan();
    p.cases.push(p.cases[0].clone());
    p.max_total_usd = 0.8;
    assert!(p.validate(&[]).unwrap_err().contains("repeated"));
    p.cases[1].task = "task-b".into();
    p.cases[1].partition = Partition::Development;
    assert!(p.validate(&[]).unwrap_err().contains("crosses"));
}
#[test]
fn source_tasks_never_enter_confirmation() {
    let mut p = plan();
    p.source_tasks.push("task-a".into());
    assert!(p.validate(&[]).unwrap_err().contains("contributed"));
    p.source_tasks = vec!["task-a-1790000001000".into()];
    assert!(p.validate(&[]).unwrap_err().contains("contributed"));
    p.cases[0].partition = Partition::Development;
    p.validate(&[]).unwrap();
}
#[test]
fn freeze_is_exclusive_and_owner_is_unique() {
    let parent = scratch("exclusive");
    let root = parent.join("study");
    let store = Store::freeze(&root, &plan(), &candidate()).unwrap();
    assert!(Store::freeze(&root, &plan(), &candidate()).is_err());
    assert!(Store::open(&root).is_err());
    assert_eq!(store.results().unwrap().len(), 2);
    drop(store);
    Store::open(&root).unwrap();
}
#[test]
fn interrupted_assignment_stays_unknown_and_is_not_replaced() {
    let parent = scratch("interrupted");
    let root = parent.join("study");
    let store = Store::freeze(&root, &plan(), &candidate()).unwrap();
    let started = store
        .begin(&store.assignments[0], vec!["example".into()])
        .unwrap();
    assert!(store.begin(&store.assignments[0], Vec::new()).is_err());
    assert!(store.begin(&store.assignments[1], Vec::new()).is_err());
    assert_eq!(store.results().unwrap()[0]["state"], "unknown");
    drop(store);
    let store = Store::open(&root).unwrap();
    assert_eq!(store.results().unwrap()[0]["state"], "unknown");
    store.finish(&finish(&store, &started)).unwrap();
    assert!(store.finish(&finish(&store, &started)).is_err());
    store.begin(&store.assignments[1], Vec::new()).unwrap();
}
#[test]
fn tampered_plan_assignments_and_candidate_refuse() {
    for item in ["plan.json", "assignments.json", "candidate.json"] {
        let parent = scratch(&format!("tampered-{item}"));
        let root = parent.join("study");
        drop(Store::freeze(&root, &plan(), &candidate()).unwrap());
        std::fs::write(root.join(item), b"changed").unwrap();
        assert!(Store::open(&root).is_err());
    }
}
#[test]
fn finished_receipt_binds_start_and_retained_bytes() {
    let parent = scratch("finish-binding");
    let store = Store::freeze(&parent.join("study"), &plan(), &candidate()).unwrap();
    let start = store.begin(&store.assignments[0], Vec::new()).unwrap();
    let mut result = finish(&store, &start);
    result.started_digest = digest(b"another start");
    assert!(store.finish(&result).is_err());
    result.started_digest = started_digest(&start).unwrap();
    std::fs::create_dir(&start.output).unwrap();
    std::fs::write(start.output.join("summary.json"), b"{}").unwrap();
    result.summary_digest = Some(digest(b"other"));
    assert!(store.finish(&result).is_err());
    result.summary_digest = Some(digest(b"{}"));
    store.finish(&result).unwrap();
    assert_eq!(store.results().unwrap()[0]["state"], "finished");
}

#[test]
fn exact_numeric_source_and_unsupported_provider_refuse_confirmation() {
    let mut p = plan();
    p.configuration.provider = "openrouter".into();
    assert!(p.validate(&[]).is_err());
    p.configuration.provider = "codex".into();
    let mut e = crate::snapshot::verify(&serde_json::from_slice(&candidate()).unwrap())
        .unwrap()
        .base
        .entries
        .remove(0);
    e.written_from = vec!["task-2".into()];
    p.cases[0].task = "task-2".into();
    assert!(p.validate(&[e]).unwrap_err().contains("contributed"));
}

#[test]
fn altered_start_and_result_metrics_refuse_before_continuation() {
    let parent = scratch("reopen-chain");
    let store = Store::freeze(&parent.join("study"), &plan(), &candidate()).unwrap();
    let start = store.begin(&store.assignments[0], vec![]).unwrap();
    let result = finish(&store, &start);
    store.finish(&result).unwrap();
    let path = store
        .root
        .join(format!("{}.finish.json", start.assignment.id));
    let mut value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["wall_seconds"] = json!(-1);
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(store.results().is_err());
    assert!(store.begin(&store.assignments[1], vec![]).is_err());
}
