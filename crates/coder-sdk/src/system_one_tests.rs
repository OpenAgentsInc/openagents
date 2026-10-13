use std::sync::Mutex;

use futures::future::BoxFuture;
use jev::decisions::{
    ChoiceProbability, DecisionAnswer, DecisionsRequest, DecisionsResponse, Served,
};
use serde_json::json;

use super::*;
use crate::decision::Recorder;

/// Answers every choice with these probabilities, and counts the calls.
struct Fixed {
    probabilities: Vec<(&'static str, f64)>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl Decider for Fixed {
    fn decide(
        &self,
        request: DecisionsRequest,
    ) -> BoxFuture<'static, Result<DecisionsResponse, String>> {
        if let jev::decisions::Input::Text(text) = &request.input {
            self.calls.lock().unwrap().push(text.clone());
        }
        let probabilities = self.probabilities.clone();
        Box::pin(async move {
            let answers = request
                .questions
                .iter()
                .map(|question| {
                    let DecisionQuestion::Choice { name, .. } = question else {
                        panic!("a choice")
                    };
                    let top = probabilities
                        .iter()
                        .max_by(|a, b| a.1.total_cmp(&b.1))
                        .unwrap();
                    DecisionAnswer::Choice {
                        name: name.clone(),
                        choice: top.0.to_owned(),
                        probabilities: probabilities
                            .iter()
                            .map(|(value, probability)| ChoiceProbability {
                                value: (*value).to_owned(),
                                probability: *probability,
                            })
                            .collect(),
                        confidence: top.1,
                    }
                })
                .collect();
            Ok(DecisionsResponse {
                id: "d".into(),
                object: "decision".into(),
                model: "clef-flash".into(),
                answers,
                usage: jev::decisions::DecisionUsage::default(),
                metadata: None,
                openagents: Served {
                    door: Some("clef".into()),
                    latency_ms: 3,
                    receipt: None,
                },
            })
        })
    }
}

fn fixed(probabilities: Vec<(&'static str, f64)>) -> (Arc<Fixed>, Arc<Mutex<Vec<String>>>) {
    let calls: Arc<Mutex<Vec<String>>> = Arc::default();
    (
        Arc::new(Fixed {
            probabilities,
            calls: calls.clone(),
        }),
        calls,
    )
}

fn quiet() -> Recorder {
    Arc::new(|_| {})
}

fn ran(command: &str, failed: bool) -> Call {
    Call::tool(
        Event::PostToolUse,
        "Bash",
        json!({"tool_input": {"command": command}, "tool_response": {"output": "error: no"}, "failed": failed}),
    )
}

fn blocking(hook: DecisionHook) -> DecisionHook {
    DecisionHook {
        timing: Timing::Blocking,
        ..hook
    }
}

#[test]
fn the_monitor_asks_only_on_a_suspect_window_and_escalates_past_its_threshold() {
    let (decider, calls) = fixed(vec![
        ("continue", 0.05),
        ("steer", 0.05),
        ("fetch_evidence", 0.0),
        ("escalate", 0.9),
    ]);
    let monitor = blocking(monitor::hook(true, None, decider, quiet()));
    // Exploring: nothing suspect, nothing asked.
    for path in ["a.rs", "b.rs", "c.rs"] {
        let read = Call::tool(
            Event::PostToolUse,
            "Read",
            json!({"tool_input": {"file_path": path}, "tool_response": "fn x() {}"}),
        );
        assert_eq!(monitor.fire(&read), HookOutcome::none());
    }
    assert_eq!(monitor.fire(&ran("cargo test", true)), HookOutcome::none());
    assert_eq!(monitor.fire(&ran("cargo test", true)), HookOutcome::none());
    assert!(calls.lock().unwrap().is_empty(), "one repeat isn't a loop");
    // The same failure a third time: a loop, asked once, escalated.
    let ended = monitor.fire(&ran("cargo test", true));
    assert_eq!(ended.stop, Stop::End);
    let evidence = ended.context.unwrap().text;
    assert!(
        evidence.starts_with("Host observed: `cargo test` failed 3 times"),
        "{evidence}"
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert!(calls.lock().unwrap()[0].contains("Bash `cargo test` (failed)"));
    // Not asked again until EVERY more calls.
    assert_eq!(monitor.fire(&ran("cargo test", true)), HookOutcome::none());
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn the_monitor_steers_or_fetches_what_the_host_saw_and_otherwise_continues() {
    let window = |monitor: &DecisionHook| {
        let mut last = HookOutcome::none();
        for _ in 0..3 {
            last = monitor.fire(&ran("make", true));
        }
        last
    };
    let (decider, _) = fixed(vec![("continue", 0.4), ("steer", 0.5), ("escalate", 0.1)]);
    let steered = window(&blocking(monitor::hook(false, None, decider, quiet())));
    assert_eq!(steered.stop, Stop::Continue);
    assert!(
        steered
            .context
            .unwrap()
            .text
            .contains("`make` failed 3 times")
    );

    let (decider, _) = fixed(vec![
        ("continue", 0.3),
        ("fetch_evidence", 0.6),
        ("steer", 0.1),
    ]);
    let probe: monitor::Probe = Arc::new(|| Some(" M src/lib.rs".to_owned()));
    let fetched = window(&blocking(monitor::hook(
        false,
        Some(probe),
        decider,
        quiet(),
    )));
    assert_eq!(
        fetched.context.unwrap().text,
        "Host checked the working tree:\n M src/lib.rs"
    );

    let (decider, _) = fixed(vec![("continue", 0.8), ("steer", 0.2)]);
    let went_on = window(&blocking(monitor::hook(false, None, decider, quiet())));
    assert_eq!(went_on, HookOutcome::none());
}

#[test]
fn the_router_passes_the_first_message_and_routes_later_ones() {
    let prompt = |text: &str| Call::new(Event::UserPromptSubmit, json!({ "prompt": text }));
    let (decider, calls) = fixed(vec![
        ("continue", 0.1),
        ("host_answers", 0.05),
        ("rebrief", 0.85),
    ]);
    let router = route::hook(None, decider, quiet());
    assert_eq!(
        router.fire(&prompt("Fix the login bug.")),
        HookOutcome::none()
    );
    assert!(calls.lock().unwrap().is_empty());
    let rebriefed = router.fire(&prompt("Now write the release notes for 2.0."));
    assert_eq!(rebriefed.stop, Stop::End);
    let asked = calls.lock().unwrap()[0].clone();
    assert!(asked.contains("1. Fix the login bug.") && asked.ends_with("release notes for 2.0."));

    let (decider, _) = fixed(vec![("continue", 0.1), ("host_answers", 0.9)]);
    let answerer: route::Answerer = Arc::new(|_| Some("This chat has cost $0.42.".to_owned()));
    let router = route::hook(Some(answerer), decider, quiet());
    let _ = router.fire(&prompt("Fix it."));
    let answered = router.fire(&prompt("What has this cost so far?"));
    assert_eq!(
        answered.permission,
        Permission::deny("This chat has cost $0.42.")
    );

    // With nobody to answer, a host question goes on to the session.
    let (decider, _) = fixed(vec![("continue", 0.1), ("host_answers", 0.9)]);
    let router = route::hook(None, decider, quiet());
    let _ = router.fire(&prompt("Fix it."));
    assert_eq!(router.fire(&prompt("Cost?")), HookOutcome::none());
}

/// Live, against the real Decisions API (Clef first): run with
/// `cargo test -p coder-sdk -- --ignored live_`.
#[test]
#[ignore = "calls the live Decisions API"]
fn live_monitor_and_router_get_answers_from_the_decisions_api() {
    let lines: Arc<Mutex<Vec<hook_log::Decision>>> = Arc::default();
    let recorder: Recorder = {
        let lines = lines.clone();
        Arc::new(move |line: &hook_log::Decision| lines.lock().unwrap().push(line.clone()))
    };
    let api: Arc<dyn Decider> = Arc::new(crate::decide::Api);
    let monitor = DecisionHook {
        deadline: Duration::from_secs(20),
        ..blocking(monitor::hook(true, None, api.clone(), recorder.clone()))
    };
    for _ in 0..3 {
        let _ = monitor.fire(&ran("cargo test -p parser", true));
    }
    let router = DecisionHook {
        deadline: Duration::from_secs(20),
        ..route::hook(None, api, recorder)
    };
    let prompt = |text: &str| Call::new(Event::UserPromptSubmit, json!({ "prompt": text }));
    let _ = router.fire(&prompt("Fix the off-by-one in the parser's line counter."));
    let _ = router.fire(&prompt(
        "Unrelated: draft a tweet announcing our 2.0 release.",
    ));
    let lines = lines.lock().unwrap();
    for line in lines.iter() {
        eprintln!(
            "{} {:?} {} p={:?} threshold={:?} model={:?} door={:?} {}ms",
            line.hook,
            line.how,
            line.action,
            line.p,
            line.threshold,
            line.model,
            line.door,
            line.latency_ms.unwrap_or(0)
        );
    }
    let decided: Vec<_> = lines
        .iter()
        .filter(|line| line.how == hook_log::How::Decision)
        .collect();
    assert_eq!(decided.len(), 2, "{lines:?}");
    assert!(
        decided
            .iter()
            .all(|line| line.p.is_some() && line.door.is_some())
    );
}
