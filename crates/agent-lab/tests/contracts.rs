use agent_lab::*;
use lab_kit::{LabKind, Want};
use serde_json::{Value, json};
#[test]
fn older_experiments_still_copy_project_files() {
    let mut saved = serde_json::to_value(starter()).unwrap();
    saved.as_object_mut().unwrap().remove("no_project_files");
    let back: Experiment = serde_json::from_value(saved).unwrap();
    assert!(!back.no_project_files);
    let mut chosen = starter();
    chosen.no_project_files = true;
    assert!(chosen.validate().is_ok());
    assert_ne!(chosen.workload(), starter().workload());
    assert!(templates::foreman().no_project_files);
    let mut evidence = serde_json::to_value(Evidence {
        project: "p".into(),
        source_sha256: "a".repeat(64),
        context_sha256: "b".repeat(64),
        runtime_version: "c".repeat(64),
        context: None,
        project_files: true,
    })
    .unwrap();
    evidence.as_object_mut().unwrap().remove("project_files");
    let read: Evidence = serde_json::from_value(evidence).unwrap();
    assert!(read.project_files);
    assert_eq!(empty_project_source().len(), 64);
}
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
                project_files: true,
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
#[cfg(feature = "views")]
#[test]
fn no_project_files_is_a_choice_and_the_report_says_so() {
    use agent_lab::view::Workbench;
    let mut s = Workbench::new("files".into());
    s.hosts.push(agent_lab::api::Host {
        id: "mac".into(),
        name: "Mac".into(),
        projects: vec![agent_lab::api::Project {
            id: "proj".into(),
            name: "openagents".into(),
        }],
        runtimes: vec!["claude-code".into()],
        runners: vec![],
        tools: vec![],
        seen_ms: 1,
    });
    s.draft.host = "mac".into();
    s.action("project-files").unwrap();
    assert!(s.draft.no_project_files);
    let form = serde_json::to_string(&s.view()).unwrap();
    assert!(form.contains("No project files"));
    assert!(form.contains("Tests run in an empty folder."));
    let experiment = s.draft.clone();
    let spec = experiment.draft.spec.clone();
    let run = Run {
        id: "f".repeat(64),
        revision: 1,
        version: Reference {
            document: "e".repeat(64),
            version: 1,
        },
        experiment,
        candidates: vec![agent_contract::AgentDefinition {
            id: "a".repeat(64),
            revision: 1,
            spec,
            deleted: false,
        }],
        trials: vec![],
        created_ms: 1,
        deadline_ms: 100,
        stopping: false,
        archived: false,
        host_settled: false,
    };
    s.selected_run = Some(run.id.clone());
    s.runs.push(run);
    let report = serde_json::to_string(&s.view()).unwrap();
    assert!(report.contains("The agent had no project files."));
}
#[cfg(feature = "views")]
#[test]
fn forms_validate_the_submitted_values_after_the_edit() {
    use rust_native::Activation;
    let mut s = agent_lab::view::Workbench::new("form".into());
    s.action("add-guidance").unwrap();
    let click = Activation {
        instance: s.instance.clone(),
        revision: s.revision,
        node: "save".into(),
    };
    assert!(s.view().validate().unwrap().activate(&click).is_err());
    assert!(s.form_view().validate().unwrap().activate(&click).is_ok());
    assert!(s.action("save").is_err());
    s.field("component:0", "A completed reference".into())
        .unwrap();
    assert!(s.action("save").unwrap().is_some());
    s.action("check:0:json").unwrap();
    s.field("expected:0", "{".into()).unwrap();
    assert!(s.action("save").is_err());
}
#[test]
fn foreman_template_is_a_valid_claude_code_experiment() {
    let mut e = agent_lab::templates::foreman();
    assert_eq!(e.draft.spec.runtime, "claude-code");
    // Claude Code runs with the host's own access; the host refuses any other setting.
    assert_eq!(e.draft.spec.access, agent_contract::Access::HostDefault);
    e.draft.agent_spec().unwrap();
    e.host = "host-1".into();
    e.project = "project-1".into();
    e.validate().unwrap();
    // The automatic cases check reach; the reviewed ones never pass on their own.
    let automatic = e
        .suite
        .cases
        .iter()
        .filter(|c| c.check.evaluate("").is_some())
        .count();
    assert_eq!(automatic, 3);
    assert_eq!(e.suite.cases[0].check.evaluate("It is #11840."), Some(true));
    // Shell calls are capped and every probe carries its own timeout (#12029).
    let text = &e.draft.spec.instructions;
    let autonomy = e.draft.spec.autonomy.as_ref().expect("autonomy");
    assert_eq!(autonomy.shell_timeout_secs, Some(120));
    assert!(text.contains("`work_issues`") && text.contains("`request_id`"));
    assert!(!text.contains("nohup openagents chat work"));
    // Check-ins and standing authority come on by default (#12030).
    assert!(autonomy.check_ins && autonomy.authority);
    assert_eq!(autonomy.every_minutes, Some(10));
    assert!(text.contains("Check-ins are off on this computer, so I only wake when you ask."));
    assert!(text.contains("`timeout 60 CMD`") && text.contains("`curl -m 20`"));
    // The stall ladder acts in three recorded rungs (#12032).
    for rung in [
        "**Message it** (rung 1",
        "**Interrupt and restart it with a brief** (rung 2",
        "**Tell the owner** (rung 3",
    ] {
        assert!(text.contains(rung), "{rung}");
    }
}
#[cfg(feature = "views")]
#[test]
fn legacy_lab_references_cannot_be_added_but_can_be_removed() {
    use agent_lab::{api::*, view::Workbench};
    let mut s = Workbench::new("references".into());
    s.hosts.push(Host {
        id: s.draft.host.clone(),
        name: "Test computer".into(),
        projects: vec![],
        runtimes: vec![],
        runners: vec![],
        tools: vec![ToolOffer {
            id: "part".into(),
            name: "Part".into(),
            project: s.draft.project.clone(),
            sha256: "a".repeat(64),
        }],
        seen_ms: 0,
    });
    s.draft.draft.components.push(Component::ToolContext {
        binding: "part".into(),
        sha256: "a".repeat(64),
    });
    let view = s.view().validate().unwrap();
    let activation = |node: &str| rust_native::Activation {
        instance: s.instance.clone(),
        revision: s.revision,
        node: node.into(),
    };
    assert!(view.activate(&activation("tool:part")).is_err());
    assert!(view.activate(&activation("remove-reference:0")).is_ok());
    assert!(s.action("tool:part").is_err());
    assert_eq!(s.draft.draft.components.len(), 1);
    s.action("remove-reference:0").unwrap();
    assert!(s.draft.draft.components.is_empty());
}
#[cfg(feature = "views")]
#[test]
fn workbench_starts_from_foreman() {
    use agent_lab::view::Workbench;
    let mut s = Workbench::new("fixture".into());
    s.view()
        .validate()
        .unwrap()
        .activate(&rust_native::Activation {
            instance: s.instance.clone(),
            revision: s.revision,
            node: "template:foreman".into(),
        })
        .unwrap();
    s.action("template:foreman").unwrap();
    assert_eq!(s.draft.draft.spec.name, "Foreman");
    assert!(s.document.is_none());
}
