use futures::future::BoxFuture;
use jev::decisions::{DecisionAnswer, Served};
use serde_json::json;

use super::super::compose::{Budget, Runner};
use super::super::outcome::{Context, Stop};
use super::super::scripted::{RECORDED_SESSION_SCHEMA, ScriptedSession};
use super::*;

/// Answers every predicate 0.9, after a pause for input that says `slow`.
struct Fake;

impl Decider for Fake {
    fn decide(
        &self,
        request: DecisionsRequest,
    ) -> BoxFuture<'static, Result<DecisionsResponse, String>> {
        Box::pin(async move {
            if matches!(&request.input, jev::decisions::Input::Text(text) if text.contains("slow"))
            {
                tokio::time::sleep(Duration::from_millis(400)).await;
            }
            Ok(answered(&request, 0.9))
        })
    }
}

fn answered(request: &DecisionsRequest, p: f64) -> DecisionsResponse {
    DecisionsResponse {
        id: "dec_1".into(),
        object: "decision".into(),
        model: "clef-flash".into(),
        answers: request
            .questions
            .iter()
            .map(|question| match question {
                DecisionQuestion::Predicate { name, .. } => DecisionAnswer::Predicate {
                    name: name.clone(),
                    probability: p,
                },
                other => panic!("unexpected {other:?}"),
            })
            .collect(),
        usage: jev::decisions::DecisionUsage::default(),
        metadata: None,
        openagents: Served {
            door: Some("clef".into()),
            latency_ms: 5,
            receipt: None,
        },
    }
}

type Lines = Arc<Mutex<Vec<Decision>>>;

fn recorder(lines: &Lines) -> Recorder {
    let lines = lines.clone();
    Arc::new(move |line: &Decision| lines.lock().unwrap().push(line.clone()))
}

const COSTS: Costs = Costs {
    wrong_act: 1.0,
    missed: 4.0,
};

/// After a tool: nothing to judge on `ok`; the rule steers on a panic it
/// can read itself; otherwise one predicate asks whether the session is
/// stuck, and a yes past the threshold steers with what the host saw.
fn stuck(lines: &Lines, disclosure: Disclosure) -> DecisionHook {
    DecisionHook {
        id: "stuck".into(),
        event: Event::PostToolUse,
        matcher: None,
        question_set: "stuck-v1".into(),
        model: MODEL.into(),
        disclosure,
        costs: COSTS,
        timing: Timing::Blocking,
        deadline: Duration::from_millis(150),
        rule: Arc::new(|call: &Call| {
            let output = call.input["output"].as_str().unwrap_or_default().to_owned();
            if output == "ok" {
                Ruled::Pass
            } else if output.contains("panicked") {
                Ruled::Decided {
                    outcome: HookOutcome::context(Context::new("stuck", "the build panicked", 1)),
                    action: "steer".into(),
                }
            } else {
                Ruled::Unsure {
                    input: output.clone(),
                    questions: vec![DecisionQuestion::Predicate {
                        name: "stuck".into(),
                        instructions: "Is the session repeating a failed step?".into(),
                        yes: None,
                        no: None,
                    }],
                    memo: json!({ "evidence": output }),
                }
            }
        }),
        act: Arc::new(|memo: &Value, response: &DecisionsResponse| {
            let p = response.probability("stuck");
            let threshold = COSTS.threshold();
            let steer = p.is_some_and(|p| p >= threshold);
            Acted {
                outcome: if steer {
                    HookOutcome {
                        // An allow from a model never counts.
                        permission: Permission::Allow,
                        ..HookOutcome::context(Context::new(
                            "stuck",
                            &format!("host saw: {}", memo["evidence"].as_str().unwrap_or("")),
                            1,
                        ))
                    }
                } else {
                    HookOutcome::none()
                },
                action: if steer { "steer" } else { "continue" }.into(),
                question: "stuck".into(),
                p,
                threshold: Some(threshold),
            }
        }),
        fallback: HookOutcome::none(),
        decider: Arc::new(Fake),
        recorder: recorder(lines),
        shadow: false,
    }
}

fn to_model() -> Disclosure {
    Disclosure {
        sends: "the last tool output".into(),
        to: "the Decisions API".into(),
    }
}

#[test]
fn rule_model_late_and_refused_firings_are_each_recorded() {
    let lines: Lines = Arc::default();
    let admission = Admission::none().admitting(to_model());
    let mut registry = Registry::new();
    stuck(&lines, to_model())
        .register(&mut registry, &admission)
        .unwrap();
    // A hook that would send the whole diff isn't admitted: not
    // registered, and the refusal is recorded.
    let wider = Disclosure {
        sends: "the whole diff".into(),
        ..to_model()
    };
    let refused = DecisionHook {
        id: "wide".into(),
        ..stuck(&lines, wider)
    }
    .register(&mut registry, &admission)
    .unwrap_err();
    assert_eq!(refused.id, "wide");
    assert_eq!(registry.hooks().count(), 1);

    let recorded = json!({
        "schema": RECORDED_SESSION_SCHEMA,
        "events": [
            {"event": "PostToolUse", "tool": "Bash", "input": {"output": "ok"}},
            {"event": "PostToolUse", "tool": "Bash", "input": {"output": "thread 'main' panicked"}},
            {"event": "PostToolUse", "tool": "Bash", "input": {"output": "error[E0308] again"}},
            {"event": "PostToolUse", "tool": "Bash", "input": {"output": "slow error"}},
            {"event": "SessionEnd", "input": null},
        ],
    })
    .to_string();
    let mut runner = Runner::new(registry, Budget::default());
    let replay = ScriptedSession::from_json(&recorded)
        .unwrap()
        .replay(&mut runner)
        .unwrap();
    let contexts: Vec<Vec<String>> = replay
        .applied
        .iter()
        .map(|applied| applied.context.iter().map(|c| c.text.clone()).collect())
        .collect();
    assert_eq!(contexts[0], Vec::<String>::new(), "a pass adds nothing");
    assert_eq!(contexts[1], ["the build panicked"], "the rule decided");
    assert_eq!(
        contexts[2],
        ["host saw: error[E0308] again"],
        "the model decided"
    );
    assert_eq!(contexts[3], Vec::<String>::new(), "late: the fallback");
    // The model's allow never counts.
    assert_eq!(replay.applied[2].permission, Permission::NoOpinion);

    let lines = lines.lock().unwrap();
    let seen: Vec<(&str, How, &str)> = lines
        .iter()
        .map(|line| (line.hook.as_str(), line.how, line.action.as_str()))
        .collect();
    assert_eq!(
        seen,
        [
            ("wide", How::Rule, "refused"),
            ("stuck", How::Rule, "steer"),
            ("stuck", How::Decision, "steer"),
            ("stuck", How::Fallback, "fallback"),
        ]
    );
    let model = &lines[2];
    assert_eq!(model.p, Some(0.9));
    assert_eq!(model.threshold, Some(0.2));
    assert_eq!(model.model.as_deref(), Some("clef-flash"));
    assert_eq!(model.door.as_deref(), Some("clef"));
    assert!(model.latency_ms.is_some());
    assert_eq!(model.detail["answers"][0]["probability"], 0.9);
    assert_eq!(model.detail["costs"]["missed"], 4.0);
    assert!(
        lines[3].detail["error"]
            .as_str()
            .unwrap()
            .starts_with("late")
    );
    assert!(
        lines[0].detail["refused"]
            .as_str()
            .unwrap()
            .contains("whole diff")
    );
}

#[test]
fn the_model_and_question_set_are_pinned_in_the_digest() {
    let lines: Lines = Arc::default();
    let digest = |question_set: &str| {
        let mut registry = Registry::new();
        DecisionHook {
            question_set: question_set.into(),
            ..stuck(&lines, to_model())
        }
        .register(&mut registry, &Admission::none().admitting(to_model()))
        .unwrap();
        crate::digest::hook_set_digest(&registry)
    };
    assert_ne!(digest("stuck-v1"), digest("stuck-v2"));
    let mut registry = Registry::new();
    stuck(&lines, to_model())
        .register(&mut registry, &Admission::none().admitting(to_model()))
        .unwrap();
    let set = crate::digest::hook_set(&registry);
    assert_eq!(set["hooks"][0]["model"], MODEL);
    assert_eq!(set["hooks"][0]["question_set"], "stuck-v1");
    assert_eq!(set["hooks"][0]["source"], "decision");
}

#[test]
fn an_async_hook_answers_on_its_next_firing_and_shadow_does_nothing() {
    let lines: Lines = Arc::default();
    let hook = DecisionHook {
        timing: Timing::Async,
        ..stuck(&lines, to_model())
    }
    .into_hook();
    let call = |output: &str| Call::tool(Event::PostToolUse, "Bash", json!({ "output": output }));
    assert_eq!(
        (hook.run)(&call("same error")).unwrap(),
        HookOutcome::none()
    );
    // The answer arrives in the background.
    for _ in 0..100 {
        if !lines.lock().unwrap().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(20));
    let next = (hook.run)(&call("ok")).unwrap();
    assert_eq!(
        next.context.map(|c| c.text).as_deref(),
        Some("host saw: same error")
    );
    let shadow = DecisionHook {
        shadow: true,
        ..stuck(&lines, to_model())
    };
    assert_eq!(shadow.fire(&call("same error")), HookOutcome::none());
    assert_eq!(lines.lock().unwrap().last().unwrap().action, "steer");
    assert!(lines.lock().unwrap().last().unwrap().detail["shadow"] == json!(true));
    let _ = Stop::Continue;
}
