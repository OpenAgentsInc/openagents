use agent_lab::*;
use lab_kit::{LabKind, Want};
use serde_json::{Value, json};
#[test]
fn strict_documents_and_bounded_trials() {
    let mut e = starter();
    assert!(e.validate().is_ok());
    e.repetitions = 5;
    assert!(e.validate().is_err());
    e.repetitions = 1;
    e.suite.cases.push(e.suite.cases[0].clone());
    assert!(e.validate().is_err());
    let mut v = serde_json::to_value(Content::Experiment {
        experiment: starter(),
    })
    .unwrap();
    v["extra"] = json!(true);
    assert!(AgentLab::check(&v, &Value::Null).is_err());
    let content = serde_json::to_value(Content::Experiment {
        experiment: starter(),
    })
    .unwrap();
    let version = lab_kit::Version {
        document: "a".repeat(64),
        number: 1,
        parent: None,
        author: "test".into(),
        summary: "Initial".into(),
        created_unix_ms: 1,
        schema: "agent-experiment.v1".into(),
        content: content.clone(),
        content_sha256: lab_kit::digest(&content),
        report: Default::default(),
        assets: vec![],
    };
    assert!(AgentLab::jobs_for(&version, &Want::Card).is_empty());
    assert_eq!(
        AgentLab::card(&version, &[], &[]).view,
        lab_kit::View::Report
    );
}
#[test]
fn reviews_unknowns_and_real_checks_are_distinct() {
    assert_eq!(
        Check::Review {
            criteria: "Check correctness".into()
        }
        .evaluate("Looks good"),
        None
    );
    assert_eq!(
        Check::Contains {
            expected: "OK".into()
        }
        .evaluate("NO"),
        Some(false)
    );
    assert_eq!(
        Check::Json {
            pointer: "/success".into(),
            expected: json!(true)
        }
        .evaluate("{\"success\":true}"),
        Some(true)
    );
    assert_eq!(
        Check::Json {
            pointer: "/success".into(),
            expected: json!(true)
        }
        .evaluate("not JSON"),
        Some(false)
    );
    assert_ne!(trial_id(&"a".repeat(64), 0), trial_id(&"a".repeat(64), 1));
}
#[test]
fn references_are_pinned_and_not_executable() {
    let mut e = starter();
    let name = "Style".to_string();
    let text = "Use concise sentences".to_string();
    let sha256 = lab_kit::digest(&json!({"name":name,"text":text}));
    e.draft
        .components
        .push(Component::Skill { name, text, sha256 });
    assert!(e.validate().is_ok());
    assert!(e.task(0).contains("Use concise"));
    if let Component::Skill { text, .. } = &mut e.draft.components[0] {
        *text = "Changed".into();
    }
    assert!(e.validate().is_err());
}
#[test]
fn lab_references_are_scoped_read_only_and_pin_every_argument() {
    use agent_lab::components::*;
    let mut b = ToolBinding {
        id: "cad".into(),
        name: "CAD check".into(),
        account: "alice".into(),
        project: "project".into(),
        origin: "http://127.0.0.1:8793/".into(),
        tool: ReadTool::Cad,
        arguments: json!({"doc":"part","version":1}),
        training_allowed: true,
        private_local: false,
    };
    assert!(b.validate("alice", "project"));
    assert!(!b.validate("bob", "project"));
    assert!(!b.validate("alice", "other"));
    let pin = b.digest();
    b.arguments["version"] = json!(2);
    assert_ne!(pin, b.digest());
    b.training_allowed = false;
    assert!(!b.validate("alice", "project"));
    b.training_allowed = true;
    b.tool = ReadTool::Video;
    b.arguments = json!({"document":"film","version":1,"render":false});
    assert!(b.validate("alice", "project"));
    b.arguments["render"] = json!(true);
    assert!(!b.validate("alice", "project"));
    b.tool = ReadTool::Robot;
    b.arguments = json!({"document":"arm","version":"a".repeat(64),"frame":0});
    assert!(b.validate("alice", "project"));
    b.arguments["command"] = json!("move");
    assert!(!b.validate("alice", "project"));
    for (check, pass, fail) in [
        (
            LabTaskCheck::Video {
                expected_version: 3,
            },
            json!({"card":{"version":3}}),
            json!({"card":{"version":4}}),
        ),
        (
            LabTaskCheck::Cad,
            json!({"passed":true}),
            json!({"passed":false}),
        ),
        (
            LabTaskCheck::Robot,
            json!({"step":{"limit_violations":[]}}),
            json!({"step":{"limit_violations":["joint"]}}),
        ),
    ] {
        assert_eq!(check.check().evaluate(&pass.to_string()), Some(true));
        assert_eq!(check.check().evaluate(&fail.to_string()), Some(false));
    }
}
#[cfg(feature = "views")]
#[test]
fn workbench_preserves_edits_rejects_stale_actions_and_locks_pending_requests() {
    use agent_lab::view::Workbench;
    use rust_native::{Activation, FieldChange};
    let mut s = Workbench::new("fixture".into());
    let view = s.view().validate().unwrap();
    let old = Activation {
        instance: s.instance.clone(),
        revision: s.revision,
        node: "name".into(),
    };
    let change = FieldChange {
        activation: old.clone(),
        value: "My changed agent".into(),
    };
    let action = view.change_field(&change).unwrap();
    s.field(&action.0, change.value).unwrap();
    assert_eq!(s.draft.draft.spec.name, "My changed agent");
    assert!(
        s.view()
            .validate()
            .unwrap()
            .change_field(&FieldChange {
                activation: old,
                value: "stale".into()
            })
            .is_err()
    );
    s.action("check:0:json").unwrap();
    s.field("expected:0", "{".into()).unwrap();
    assert!(
        s.view()
            .validate()
            .unwrap()
            .activate(&Activation {
                instance: s.instance.clone(),
                revision: s.revision,
                node: "save".into()
            })
            .is_err()
    );
    s.pending = true;
    s.bump();
    let v = s.view().validate().unwrap();
    assert!(
        v.activate(&Activation {
            instance: s.instance.clone(),
            revision: s.revision,
            node: "new".into()
        })
        .is_err()
    );
    assert!(
        v.activate(&Activation {
            instance: s.instance.clone(),
            revision: s.revision,
            node: "retry".into()
        })
        .is_ok()
    );
}
#[test]
fn comparisons_keep_every_trial_and_require_matching_evidence() {
    let mut e = starter();
    e.repetitions = 2;
    e.compare = Some(Pin {
        id: "b".repeat(64),
        revision: 1,
    });
    e.suite.partition = Partition::Confirmation;
    let candidates = (0..2)
        .map(|i| agent_contract::AgentDefinition {
            id: format!("{i:064x}"),
            revision: 1,
            spec: e.draft.spec.clone(),
            deleted: false,
        })
        .collect();
    let trials = (0..4)
        .map(|i| Trial {
            index: i,
            candidate: i / 2,
            case: 0,
            repetition: (i % 2) as u32,
            assignment: trial_id(&"f".repeat(64), i),
            chat: None,
            outcome: Outcome::Passed,
            result: Some("HELLO".into()),
            evidence: Some(Evidence {
                project: e.project.clone(),
                source_sha256: "a".repeat(64),
                context_sha256: "b".repeat(64),
                runtime_version: format!("{:064x}", i / 2 + 1),
                context: None,
            }),
            started_ms: Some(1),
            finished_ms: Some(2),
            cost_usd: None,
            reviewer: None,
            review_note: None,
        })
        .collect();
    let mut r = Run {
        id: "f".repeat(64),
        revision: 1,
        version: Reference {
            document: "e".repeat(64),
            version: 1,
        },
        experiment: e,
        candidates,
        trials,
        created_ms: 1,
        deadline_ms: 100,
        stopping: false,
        archived: false,
        host_settled: false,
    };
    assert!(r.validate().is_ok());
    assert!(r.comparison_eligible());
    assert_eq!(r.counts(0).planned, 2);
    assert_eq!(r.counts(1).planned, 2);
    r.trials[3].outcome = Outcome::Stopped;
    assert!(!r.comparison_eligible());
    assert_eq!(r.counts(1).incomplete, 1);
    assert_eq!(r.counts(1).planned, 2);
    r.trials[3].outcome = Outcome::Passed;
    r.trials[3].evidence.as_mut().unwrap().source_sha256 = "c".repeat(64);
    assert!(!r.comparison_eligible());
    r.trials[3].evidence.as_mut().unwrap().source_sha256 = "a".repeat(64);
    r.trials[3].evidence.as_mut().unwrap().runtime_version = "d".repeat(64);
    assert!(!r.comparison_eligible());
}
#[cfg(feature = "views")]
#[test]
fn editing_one_reference_preserves_the_others() {
    let mut s = agent_lab::view::Workbench::new("references".into());
    s.action("add-guidance").unwrap();
    s.field("component:0", "First skill".into()).unwrap();
    s.action("add-guidance").unwrap();
    s.field("component:1", "Second skill".into()).unwrap();
    s.field("component:0", "Changed first skill".into())
        .unwrap();
    assert_eq!(s.draft.draft.components.len(), 2);
    assert!(s.draft.task(0).contains("Second skill"));
    assert!(
        s.draft
            .draft
            .agent_spec()
            .unwrap()
            .instructions
            .contains("Changed first skill")
    );
    s.action("remove-reference:0").unwrap();
    assert_eq!(s.draft.draft.components.len(), 1);
    assert!(
        s.draft
            .draft
            .agent_spec()
            .unwrap()
            .instructions
            .contains("Second skill")
    );
}
