use serde_json::json;

use super::coordination::{self, CHANGE_OVERLAP, FAILURE_ON_MAIN, READY_TO_CLOSE, RUN_PROGRESS};
use super::*;

#[test]
fn every_coordination_set_parses_with_its_gate() {
    assert_eq!(
        coordination::names(),
        vec![
            CHANGE_OVERLAP,
            FAILURE_ON_MAIN,
            READY_TO_CLOSE,
            RUN_PROGRESS
        ]
    );
    for name in coordination::names() {
        let set = coordination::set(name).unwrap();
        assert_eq!(set.id, format!("openagents.{name}.v1"));
        assert!(set.question(&set.gate).is_some(), "{name}");
        assert!(set.state_max_bytes.is_some(), "{name}");
    }
    let progress = coordination::set(RUN_PROGRESS).unwrap();
    let Some(DecisionQuestion::Choice { choices, .. }) = progress.question("activity") else {
        panic!("activity is a choice");
    };
    let values: Vec<&str> = choices.iter().map(|c| c.value.as_str()).collect();
    assert_eq!(
        values,
        ["progressing", "waiting_on_work", "looping", "stopped"]
    );
}

#[test]
fn a_request_sends_the_state_as_json_text_with_the_set_named() {
    let set = coordination::set(CHANGE_OVERLAP).unwrap();
    let state = json!({"a": {"files": ["crates/coder/src/lib.rs"]}, "b": {"files": ["docs/x.md"]}});
    let request = set.request(MODEL, &state).unwrap();
    let wire = serde_json::to_value(&request).unwrap();
    assert_eq!(wire["model"], MODEL);
    let input: Value = serde_json::from_str(wire["input"].as_str().unwrap()).unwrap();
    assert_eq!(input, state);
    assert_eq!(wire["questions"][0]["type"], "predicate");
    assert_eq!(wire["questions"][0]["name"], "overlap");
    assert_eq!(
        wire["metadata"]["question_set"],
        "openagents.change-overlap.v1"
    );
}

#[test]
fn a_state_over_the_bound_is_refused_before_it_is_sent() {
    let set = coordination::set(FAILURE_ON_MAIN).unwrap();
    let state = json!({"failure": {"message": "x".repeat(20_000)}});
    let error = set.request(MODEL, &state).unwrap_err().to_string();
    assert!(error.contains("over the 12288"), "{error}");
}

#[test]
fn a_judgment_keeps_the_answers_and_the_receipt() {
    let set = coordination::set(RUN_PROGRESS).unwrap();
    let response: DecisionsResponse = serde_json::from_value(json!({
        "id": "dec_123",
        "object": "decision",
        "model": "clef-flash",
        "answers": [
            {"type": "predicate", "name": "progressed", "probability": 0.12},
            {"type": "choice", "name": "activity", "choice": "stopped",
             "probabilities": [{"value": "stopped", "probability": 0.8},
                               {"value": "looping", "probability": 0.2}],
             "confidence": 0.7}
        ],
        "openagents": {"door": "clef", "latency_ms": 41, "receipt": "rcpt_9"}
    }))
    .unwrap();
    let judgment = Judgment::from_response(set, response).unwrap();
    assert_eq!(judgment.probability("progressed"), Some(0.12));
    assert_eq!(judgment.choice("activity"), Some("stopped"));
    assert_eq!(
        judgment.receipt,
        Receipt {
            question_set: "openagents.run-progress.v1".into(),
            decision: "dec_123".into(),
            model: "clef-flash".into(),
            door: Some("clef".into()),
            receipt: Some("rcpt_9".into()),
            latency_ms: 41,
        }
    );
    assert_eq!(
        judgment.summary(),
        "progressed 0.12, activity stopped (0.70)"
    );
}

#[test]
fn a_response_missing_a_question_is_not_a_judgment() {
    let set = coordination::set(RUN_PROGRESS).unwrap();
    let response: DecisionsResponse = serde_json::from_value(json!({
        "id": "dec_1", "object": "decision", "model": "clef",
        "answers": [{"type": "predicate", "name": "progressed", "probability": 0.9}]
    }))
    .unwrap();
    let error = Judgment::from_response(set, response)
        .unwrap_err()
        .to_string();
    assert!(error.contains("`activity`"), "{error}");
}

#[test]
fn files_it_cannot_send_are_refused() {
    let gate = r#"{"v": 1, "id": "x.v1", "gate": "nope", "questions": {"a": {"type": "noul", "instructions": "A?"}}}"#;
    assert!(
        QuestionSet::parse(gate)
            .unwrap_err()
            .to_string()
            .contains("gate")
    );
    let empty = r#"{"v": 1, "id": "x.v1", "questions": {}}"#;
    assert!(QuestionSet::parse(empty).is_err());
    let version =
        r#"{"v": 2, "id": "x.v1", "questions": {"a": {"type": "noul", "instructions": "A?"}}}"#;
    assert!(QuestionSet::parse(version).is_err());
}

#[test]
fn score_levels_take_their_summary_as_the_label() {
    let text = include_str!("../../../questions/memory-importance.json");
    let set = QuestionSet::parse(text).unwrap();
    let Some(DecisionQuestion::Score { levels, .. }) = set.question("importance") else {
        panic!("importance is a score");
    };
    assert_eq!(levels.len(), 4);
    assert_eq!(levels[0].label, "Routine work that went as expected.");
}
