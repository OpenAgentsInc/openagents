use super::*;
use std::path::PathBuf;

fn store(name: &str) -> Store {
    let parent =
        std::env::temp_dir().join(format!("microcoder-study-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&parent);
    std::fs::create_dir(&parent).unwrap();
    let doc = "---\nid: e.one\nversion: 1\nkind: method\ntitle: T\nsummary: S.\ntags: [x]\napplies_when: A.\nstatus: admitted\nauthor: local\nprovenance:\n  written_from: [reference]\n  cites: [Book]\nevidence: []\n---\n\n## Details\n\nBody.\n";
    let signer = nostr::domain::RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
    let candidate =
        knowledge::snapshot::create(&[doc.to_string()], &signer, "test", "1", "CC0-1.0", 1)
            .unwrap();
    let candidate = serde_json::to_vec(&candidate).unwrap();
    let plan = Plan {
        schema: study::SCHEMA.into(),
        study: "fixture-study".into(),
        owner: "local:test".into(),
        binary: PathBuf::from("/fixture/microcoder"),
        binary_digest: knowledge::digest(b"binary"),
        tasks_root: PathBuf::from("/fixture/tasks"),
        candidate_digest: knowledge::digest(&candidate),
        configuration: Configuration {
            provider: "codex".into(),
            cost_basis: "list_price".into(),
            retrieval_mode: "lexical".into(),
            decision_base_url: "https://api.typesafe.ai".into(),
            decision_model: "jev-test-version".into(),
            model: crate::MODEL.into(),
            strong_model: crate::STRONG_MODEL.into(),
            effort: "medium".into(),
            max_steps: 10,
            max_seconds: 60,
            max_usd: 0.1,
            command_seconds: 5,
            test_seconds: 5,
            prompt: "Solve task".into(),
            network: "none".into(),
            acceptance: true,
        },
        cases: vec![
            Case {
                task: "development-task".into(),
                group: "development-family".into(),
                partition: Partition::Development,
                workload_digest: knowledge::digest(b"dev"),
                environment_digest: knowledge::digest(b"env"),
            },
            Case {
                task: "confirmation-task".into(),
                group: "confirmation-family".into(),
                partition: Partition::Confirmation,
                workload_digest: knowledge::digest(b"confirm"),
                environment_digest: knowledge::digest(b"env"),
            },
        ],
        repetitions: 1,
        first_subject: true,
        source_tasks: vec![],
        max_total_usd: 0.4,
    };
    Store::freeze(&parent.join("study"), &plan, &candidate).unwrap()
}
fn recorded(store: &Store, assignment: &Assignment, events: &str) -> PathBuf {
    let output = store.root.join(format!("{}.run", assignment.id));
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("summary.json"),json!({"task":assignment.task,"model":crate::MODEL,"provider":"codex","cost_basis":"list_price","effort":store.plan.configuration.effort,"decision":{"base_url":store.plan.configuration.decision_base_url,"model":store.plan.configuration.decision_model},"retrieval_mode":if assignment.arm==Arm::Subject {"lexical"} else {"off"},"reward":1.0,"outcome":{"knowledge":[],"model_usd":0.01,"jev_usd":0.001,"embedding_usd":0.0}}).to_string()).unwrap();
    std::fs::write(output.join("events.jsonl"), events).unwrap();
    output
}
#[test]
fn argv_pins_codex_and_replaces_ambient_knowledge_only_in_subject() {
    let store = store("argv");
    let subject = store
        .assignments
        .iter()
        .find(|a| a.arm == Arm::Subject)
        .unwrap();
    let baseline = store
        .assignments
        .iter()
        .find(|a| a.arm == Arm::Baseline)
        .unwrap();
    let a = argv(&store, subject);
    let b = argv(&store, baseline);
    for args in [&a, &b] {
        assert!(args.windows(2).any(|p| p == ["--provider", "codex"]));
        assert!(args.windows(2).any(|p| p == ["--route", "never"]));
        assert!(args.iter().any(|p| p == "--kb-lexical"));
        assert!(!args.iter().any(|s| s == "openrouter"));
    }
    assert!(a.iter().any(|s| s == "--kb-snapshot"));
    assert!(a.iter().any(|s| s == "--kb-cache"));
    assert!(!b.iter().any(|s| s == "--kb-snapshot"));
    assert!(b.windows(2).any(|p| p == ["--kb", "off"]));
}
#[test]
fn audit_preserves_lower_bound_when_provider_charge_is_unknown() {
    let store = store("audit");
    let a = &store.assignments[0];
    let output = recorded(
        &store,
        a,
        &json!({"event":"generated","generated":{"model":crate::MODEL,"usd":0.0}}).to_string(),
    );
    let result = audit(&store, &output, a).unwrap();
    assert_eq!(result.reward, Some(1.0));
    assert_eq!(result.costs["model_usd"], None);
    assert!((result.known_cost_lower_bound_usd - 0.011).abs() < 1e-10);
    assert_eq!(result.served_models, [crate::MODEL]);
    std::fs::write(output.join("events.jsonl"), "{malformed}").unwrap();
    let result = audit(&store, &output, a).unwrap();
    assert!(result.costs.values().all(Option::is_none));
    assert!(result.problems.iter().any(|s| s == "malformed_event"));
}
#[test]
fn provider_mismatch_cannot_count_as_a_frozen_configuration_success() {
    let store = store("provider");
    let a = &store.assignments[0];
    let output = recorded(&store, a, "");
    let mut summary: Value =
        serde_json::from_slice(&std::fs::read(output.join("summary.json")).unwrap()).unwrap();
    summary["provider"] = json!("openrouter");
    std::fs::write(output.join("summary.json"), summary.to_string()).unwrap();
    let result = audit(&store, &output, a).unwrap();
    assert_eq!(result.reward, None);
    assert!(
        result
            .problems
            .iter()
            .any(|s| s == "summary_configuration_unknown_or_mismatched")
    );
    assert!(result.summary_digest.is_some());
}
#[test]
fn report_keeps_unstarted_assignments_and_partitions_separate() {
    let store = store("partitions");
    let result = report(&store).unwrap();
    assert_eq!(result["assignments"].as_array().unwrap().len(), 4);
    for name in ["subject", "baseline"] {
        assert_eq!(result["arms"][name]["planned"], 2);
        assert_eq!(result["arms"][name]["outcome_unknown"], 2);
        assert!(result["arms"][name]["total_usd"].is_null());
        assert_eq!(result["partitions"]["confirmation"][name]["planned"], 1);
        assert_eq!(result["partitions"]["development"][name]["planned"], 1);
        assert_eq!(
            result["tasks"]["confirmation-task"]["arms"][name]["planned"],
            1
        );
    }
    assert_eq!(result["verdict"], "inconclusive");
    assert_eq!(result["promotion_eligible"], false);
    let interval = wilson(1, 1).unwrap();
    assert!(interval.0 < 0.21 && interval.1 > 0.99);
}
