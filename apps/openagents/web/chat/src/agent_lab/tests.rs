use super::*;
use agent_lab::{Outcome, starter};

fn event(s: &State, node: &str) -> Activation {
    Activation {
        instance: s.workbench.instance.clone(),
        revision: s.workbench.revision,
        node: node.into(),
    }
}

fn edit(s: &mut State, node: &str, value: &str) {
    s.edit(FieldChange {
        activation: event(s, node),
        value: value.into(),
    })
    .unwrap();
}

fn act(s: &mut State, node: &str) -> Option<Command> {
    s.activate(&event(s, node)).unwrap()
}

fn document(s: &State, version: u64) -> Value {
    json!({ "id": "a".repeat(64), "deleted": false, "versions": [{
        "document": "a".repeat(64), "number": version, "parent": null, "author": "test", "summary": "Saved experiment",
        "created_unix_ms": 1, "schema": "agent-experiment.v1", "content_sha256": "b".repeat(64),
        "content": Content::Experiment { experiment: s.workbench.draft.clone() }
    }] })
}

fn run(s: &State) -> Value {
    json!({ "id": "c".repeat(64), "revision": 1, "version": {"document": "a".repeat(64), "version": 1},
        "experiment": s.workbench.draft, "candidates": [{"id": "d".repeat(64), "revision": 1, "spec": s.workbench.draft.draft.spec, "deleted": false}],
        "trials": [{ "index": 0, "candidate": 0, "case": 0, "repetition": 0, "assignment": "e".repeat(64), "chat": "trial-chat",
            "outcome": "needs_review", "result": "HELLO", "evidence": null, "started_ms": 1, "finished_ms": 2, "cost_usd": 0.0,
            "reviewer": null, "review_note": null }],
        "created_ms": 1, "deadline_ms": 1000, "stopping": false, "archived": false })
}

#[test]
fn edits_validate_the_shared_view_and_reject_stale_and_disabled_events() {
    let mut s = State::new("test".into());
    let stale = event(&s, "name");
    edit(&mut s, "name", "Test agent");
    assert!(
        s.edit(FieldChange {
            activation: stale,
            value: "Old edit".into()
        })
        .is_err()
    );
    assert_eq!(s.workbench.draft.draft.spec.name, "Test agent");
    assert!(
        s.edit(FieldChange {
            activation: event(&s, "name"),
            value: "x".repeat(81)
        })
        .is_err()
    );
    assert!(s.activate(&event(&s, "start")).is_err());
    assert!(s.activate(&event(&s, "promote")).is_err());
    let command = act(&mut s, "save").unwrap();
    s.prepare(command, "1".repeat(64)).unwrap();
    assert!(
        s.edit(FieldChange {
            activation: event(&s, "name"),
            value: "During save".into()
        })
        .is_err()
    );
    assert!(s.activate(&event(&s, "new")).is_err());
}

#[test]
fn a_lost_response_retries_the_exact_request_and_body() {
    let mut s = State::new("test".into());
    s.prepare(Command::Save, "1".repeat(64)).unwrap();
    let original = s.pending.as_ref().unwrap().clone();
    let retry = act(&mut s, "retry").unwrap();
    s.prepare(retry, "2".repeat(64)).unwrap();
    let pending = s.pending.as_ref().unwrap();
    assert_eq!(pending.body, original.body);
    assert_eq!(pending.path, original.path);
    assert!(s.prepare(Command::Save, "3".repeat(64)).is_err());
    assert!(s.complete(Value::Null).is_err());
    assert!(s.pending.is_some());
    s.complete(document(&s, 1)).unwrap();
    assert!(s.pending.is_none());
    assert!(!s.workbench.pending);
}

#[test]
fn edit_save_start_read_review_and_promote_use_the_saved_version() {
    let mut s = State::new("test".into());
    s.workbench.hosts.push(Host {
        id: "scratch".into(),
        name: "Scratch computer".into(),
        projects: vec![Project {
            id: "project".into(),
            name: "Scratch project".into(),
        }],
        runtimes: vec!["codex".into()],
        runners: vec![],
        tools: vec![],
        seen_ms: 1,
        always_on: false,
    });
    act(&mut s, "host:scratch");
    edit(&mut s, "name", "Test agent");
    edit(&mut s, "instructions", "Answer HELLO.");
    let command = act(&mut s, "save").unwrap();
    s.prepare(command, "1".repeat(64)).unwrap();
    assert_eq!(
        s.pending.as_ref().unwrap().body["content"]["experiment"]["host"],
        "scratch"
    );
    s.complete(document(&s, 1)).unwrap();
    let command = act(&mut s, "start").unwrap();
    s.prepare(command, "2".repeat(64)).unwrap();
    assert!(s.pending.as_ref().unwrap().path.ends_with("/jobs"));
    assert_eq!(s.pending.as_ref().unwrap().body["version"], 1);
    s.complete(run(&s)).unwrap();
    assert_eq!(
        s.workbench.runs[0].trials[0].result.as_deref(),
        Some("HELLO")
    );
    assert!(
        matches!(act(&mut s, "session:trial-chat"), Some(Command::Session(id)) if id == "trial-chat")
    );
    let run_id = "c".repeat(64);
    edit(&mut s, &format!("review-note:{run_id}:0"), "Correct answer");
    let command = act(&mut s, &format!("review:{run_id}:0:pass")).unwrap();
    s.prepare(command, "3".repeat(64)).unwrap();
    let body = &s.pending.as_ref().unwrap().body;
    assert_eq!(body["expected_revision"], 1);
    assert_eq!(body["action"]["note"], "Correct answer");
    let mut reviewed = run(&s);
    reviewed["trials"][0]["outcome"] = json!(Outcome::Passed);
    reviewed["revision"] = json!(2);
    s.complete(reviewed).unwrap();
    let command = act(&mut s, "promote").unwrap();
    s.prepare(command, "4".repeat(64)).unwrap();
    assert_eq!(s.pending.as_ref().unwrap().body["version"], 1);
    s.complete(json!({"id": "d".repeat(64), "revision": 1, "spec": s.workbench.draft.draft.spec, "deleted": false})).unwrap();
    assert!(
        s.workbench
            .notice
            .as_deref()
            .unwrap()
            .contains("ready in your agent picker")
    );
    assert_eq!(s.workbench.agents.len(), 1);
    edit(&mut s, "instructions", "New instructions");
    assert!(s.activate(&event(&s, "start")).is_err());
    assert!(s.activate(&event(&s, "promote")).is_err());
    let command = act(&mut s, "save").unwrap();
    s.prepare(command, "5".repeat(64)).unwrap();
    assert_eq!(s.pending.as_ref().unwrap().method, "PUT");
    assert_eq!(s.pending.as_ref().unwrap().body["expected_revision"], 1);
}

#[test]
fn templates_and_result_notes_stay_in_their_tabs() {
    let mut s = State::new("test".into());
    let id = agent_lab::templates::FOREMAN_ID;
    act(&mut s, &format!("template:{id}"));
    assert_eq!(s.workbench.draft.title, "Foreman");
    assert!(s.workbench.document.is_none());
    assert_eq!(area("review-note:run:0"), Area::Results);
    assert_eq!(area("reviewed:0"), Area::Results);
    assert_eq!(area("task:0"), Area::Tests);
    assert_eq!(area("instructions"), Area::Agent);
    s.workbench.draft = starter();
    assert!(!s.workbench.signs_owner);
}
