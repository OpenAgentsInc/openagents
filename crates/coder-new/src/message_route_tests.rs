use std::sync::Mutex;

use futures::future::BoxFuture;
use hook_log::Decision;
use jev::decisions::{ChoiceProbability, DecisionAnswer, DecisionsRequest, Served};

use coder_sdk::{Admission, Registry};

use super::*;

fn said(role: &str, text: &str) -> Said {
    Said {
        role: role.to_owned(),
        text: text.to_owned(),
    }
}

fn note(latest: &str) -> Note {
    Note {
        earlier: vec![
            said("user", "Fix the flaky timeout test in crates/foo."),
            said(
                "assistant",
                "Fixed: the timeout was 50 ms; it is 500 ms now.",
            ),
        ],
        latest: latest.to_owned(),
        midturn: false,
        facts: Facts {
            model: Some("Opus 5.5".to_owned()),
            spend: Some((120_000, 0.42)),
        },
    }
}

/// A response choosing `route` at `p`, the rest shared out.
fn answered(route: &str, p: f64) -> DecisionsResponse {
    let routes = [CONTINUE, REBRIEF, HOST_MODEL, HOST_SPEND];
    DecisionsResponse {
        id: "dec_1".into(),
        object: "decision".into(),
        model: "clef-flash".into(),
        answers: vec![DecisionAnswer::Choice {
            name: QUESTION.into(),
            choice: route.into(),
            probabilities: routes
                .iter()
                .map(|value| ChoiceProbability {
                    value: (*value).into(),
                    probability: if *value == route { p } else { (1.0 - p) / 3.0 },
                })
                .collect(),
            confidence: p,
        }],
        usage: jev::decisions::DecisionUsage::default(),
        metadata: None,
        openagents: Served {
            door: Some("clef".into()),
            latency_ms: 5,
            receipt: None,
        },
    }
}

fn memo_of(ruled: Ruled) -> Value {
    match ruled {
        Ruled::Unsure { memo, .. } => memo,
        _ => panic!("expected a question"),
    }
}

#[test]
fn the_rule_asks_only_about_a_noted_later_message_between_turns() {
    let latest = "Which model are you?";
    assert!(matches!(rule(None, latest), Ruled::Pass));
    let mut first = note(latest);
    first.earlier.clear();
    assert!(matches!(rule(Some(first), latest), Ruled::Pass));
    let mut joining = note(latest);
    joining.midturn = true;
    assert!(matches!(rule(Some(joining), latest), Ruled::Pass));
    assert!(matches!(
        rule(Some(note("/compact")), "/compact"),
        Ruled::Pass
    ));
    // Another prompt than the noted one.
    assert!(matches!(rule(Some(note(latest)), "Reply OK."), Ruled::Pass));
    // A briefing ahead of the person's words is still this prompt.
    let briefed = format!("<briefing>\n...\n</briefing>\n\n{latest}");
    let Ruled::Unsure {
        input, questions, ..
    } = rule(Some(note(latest)), &briefed)
    else {
        panic!("expected a question");
    };
    assert!(input.contains("Fix the flaky timeout test") && input.ends_with(latest));
    let DecisionQuestion::Choice { choices, .. } = &questions[0] else {
        panic!("a choice");
    };
    let values: Vec<&str> = choices.iter().map(|c| c.value.as_str()).collect();
    assert_eq!(values, [CONTINUE, REBRIEF, HOST_MODEL, HOST_SPEND]);
    // Coder offers to answer only what it knows.
    let mut unknown = note(latest);
    unknown.facts = Facts::default();
    let Ruled::Unsure { questions, .. } = rule(Some(unknown), latest) else {
        panic!("expected a question");
    };
    let DecisionQuestion::Choice { choices, .. } = &questions[0] else {
        panic!("a choice");
    };
    assert_eq!(choices.len(), 2);
}

#[test]
fn a_route_acts_only_past_its_threshold() {
    let memo = memo_of(rule(Some(note("Which model?")), "Which model?"));
    let host = act(&memo, &answered(HOST_MODEL, 0.95));
    assert_eq!(host.action, HOST_MODEL);
    assert_eq!(
        host.outcome,
        HookOutcome::deny("This chat runs on Opus 5.5, in Claude Code.")
    );
    assert_eq!(
        routed(&host, &memo),
        Some(Routed::Host(
            "This chat runs on Opus 5.5, in Claude Code.".into()
        ))
    );
    let spend = act(&memo, &answered(HOST_SPEND, 0.95));
    assert_eq!(
        routed(&spend, &memo),
        Some(Routed::Host(
            "This chat has used 120000 tokens so far, about $0.42.".into()
        ))
    );
    // Below 2 / 2.25 the message continues, with p and threshold recorded.
    let unsure = act(&memo, &answered(HOST_MODEL, 0.85));
    assert_eq!(unsure.action, CONTINUE);
    assert_eq!(unsure.outcome, HookOutcome::none());
    assert!((unsure.threshold.unwrap() - 2.0 / 2.25).abs() < 1e-9);
    let rebrief = act(&memo, &answered(REBRIEF, 0.9));
    assert_eq!(rebrief.action, REBRIEF);
    assert_eq!(routed(&rebrief, &memo), Some(Routed::Rebrief));
    assert_eq!(act(&memo, &answered(REBRIEF, 0.8)).action, CONTINUE);
    let go_on = act(&memo, &answered(CONTINUE, 0.99));
    assert_eq!(
        (go_on.action.as_str(), routed(&go_on, &memo)),
        (CONTINUE, None)
    );
}

struct Fixed(Result<(&'static str, f64), &'static str>);

impl Decider for Fixed {
    fn decide(
        &self,
        _request: DecisionsRequest,
    ) -> BoxFuture<'static, Result<DecisionsResponse, String>> {
        let answer = self.0;
        Box::pin(async move {
            answer
                .map(|(route, p)| answered(route, p))
                .map_err(str::to_owned)
        })
    }
}

fn fired(decider: Fixed, shadow: bool, cwd: &Path) -> (HookOutcome, Option<Routed>, Vec<Decision>) {
    let lines: Arc<Mutex<Vec<Decision>>> = Arc::default();
    let recorder: Recorder = {
        let lines = lines.clone();
        Arc::new(move |line: &Decision| lines.lock().unwrap().push(line.clone()))
    };
    let latest = "How much has this chat cost so far?";
    super::note(cwd, note(latest));
    let hook = hook(cwd.to_owned(), Arc::new(decider), recorder, shadow);
    let outcome = hook.fire(&Call::new(
        Event::UserPromptSubmit,
        json!({ "prompt": latest }),
    ));
    let lines = lines.lock().unwrap().clone();
    (outcome, take_routed(cwd), lines)
}

#[test]
fn the_hook_blocks_and_leaves_the_route_for_the_host_and_records_it() {
    let cwd = PathBuf::from("/tmp/message-route-test-a");
    let (outcome, routed, lines) = fired(Fixed(Ok((HOST_SPEND, 0.97))), false, &cwd);
    assert_eq!(
        outcome,
        HookOutcome::deny("This chat has used 120000 tokens so far, about $0.42.")
    );
    assert!(matches!(routed, Some(Routed::Host(_))));
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].hook, HOOK);
    assert_eq!(lines[0].action, HOST_SPEND);
    assert_eq!(lines[0].p, Some(0.97));
    // The note was this prompt's, and is used once.
    assert!(take_note(&cwd).is_none());
    // In shadow, the same line and nothing done.
    let (outcome, routed, lines) = fired(Fixed(Ok((HOST_SPEND, 0.97))), true, &cwd);
    assert_eq!((outcome, routed), (HookOutcome::none(), None));
    assert_eq!(lines[0].action, HOST_SPEND);
    // A failed call lets the message continue.
    let (outcome, routed, lines) = fired(Fixed(Err("offline")), false, &cwd);
    assert_eq!((outcome, routed), (HookOutcome::none(), None));
    assert_eq!(lines[0].action, "fallback");
}

#[test]
fn a_blocked_message_reaches_claude_code_as_a_block() {
    let cwd = PathBuf::from("/tmp/message-route-test-b");
    let mut registry = Registry::new();
    let admission = Admission::none().admitting(disclosure());
    hook(
        cwd.clone(),
        Arc::new(Fixed(Ok((REBRIEF, 0.95)))),
        Arc::new(|_: &Decision| {}),
        false,
    )
    .register(&mut registry, &admission)
    .unwrap();
    let backend = coder_sdk::claude::Claude::new(registry, coder_sdk::Budget::default()).unwrap();
    super::note(&cwd, note("Now write a haiku about autumn."));
    let applied = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(backend.fire(coder_sdk::claude::call(
            Event::UserPromptSubmit,
            &json!({"prompt": "Now write a haiku about autumn.", "session_id": "s"}),
        )));
    let out = coder_sdk::claude::output(&applied);
    assert_eq!(out.decision.as_deref(), Some("block"));
    assert_eq!(take_routed(&cwd), Some(Routed::Rebrief));
}

fn call(input: u64, cache_read: u64) -> replay::Call {
    replay::Call {
        input,
        cache_read,
        cache_write: 0,
    }
}

#[test]
fn the_replay_reads_prompts_and_estimates_what_routes_save() {
    let jsonl = [
        json!({"type": "user", "message": {"content": "Fix the flaky test."}}),
        json!({"type": "assistant", "message": {"id": "m1", "usage": {"input_tokens": 1000, "cache_read_input_tokens": 0}}}),
        json!({"type": "assistant", "message": {"id": "m1", "usage": {"input_tokens": 1000}}}),
        json!({"type": "user", "message": {"content": [{"type": "tool_result", "content": "ok"}]}}),
        json!({"type": "assistant", "message": {"id": "m2", "usage": {"input_tokens": 10, "cache_read_input_tokens": 3000}}}),
        json!({"type": "user", "isMeta": true, "message": {"content": "caveat"}}),
        json!({"type": "user", "message": {"content": [{"type": "text", "text": "Which model?"}]}}),
        json!({"type": "assistant", "message": {"id": "m3", "usage": {"input_tokens": 10, "cache_read_input_tokens": 4000}}}),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    let prompts = replay::prompts(&jsonl);
    assert_eq!(prompts.len(), 2);
    assert_eq!(prompts[0].calls, vec![call(1000, 0), call(10, 3000)]);
    assert_eq!(prompts[1].text, "Which model?");

    let chat = replay::Chat {
        id: "c".into(),
        said: vec![
            said("user", "Fix the flaky test."),
            said("assistant", "Fixed."),
            said("user", "Which model?"),
            said("assistant", "Opus."),
            said("user", "Write a haiku about autumn."),
            said("user", "Make it rhyme."),
        ],
        prompts: vec![
            prompts[0].clone(),
            prompts[1].clone(),
            replay::Prompt {
                text: "Write a haiku about autumn.".into(),
                calls: vec![call(10, 5000)],
            },
            replay::Prompt {
                text: "Make it rhyme.".into(),
                calls: vec![call(10, 5200), call(10, 5300)],
            },
        ],
        model: Some("Opus 5.5".into()),
    };
    /// Host for the model question, re-brief for the haiku, else continue.
    struct ByText;
    impl Decider for ByText {
        fn decide(
            &self,
            request: DecisionsRequest,
        ) -> BoxFuture<'static, Result<DecisionsResponse, String>> {
            let jev::decisions::Input::Text(text) = &request.input else {
                panic!("text input");
            };
            let latest = text.rsplit('\n').next().unwrap_or_default().to_owned();
            Box::pin(async move {
                Ok(match latest.as_str() {
                    "Which model?" => answered(HOST_MODEL, 0.95),
                    "Write a haiku about autumn." => answered(REBRIEF, 0.9),
                    _ => answered(CONTINUE, 0.9),
                })
            })
        }
    }
    let report = replay::replay(&[chat], &ByText);
    assert_eq!(report.messages, 3);
    assert_eq!(report.routes[HOST_MODEL], 1);
    assert_eq!(report.routes[REBRIEF], 1);
    assert_eq!(report.routes[CONTINUE], 1);
    assert_eq!(report.saved_by_host, 4010);
    // Fresh-session size is the first call (1000); the re-brief turn's
    // first call carried 5010, so each call from there saves 4010.
    assert_eq!(report.saved_by_rebrief, 3 * 4010);
    assert_eq!(report.later_tokens, 4010 + 5010 + 5210 + 5310);
}

/// The real Decisions API on three messages. Run by hand:
/// `cargo test -p coder-new --lib message_route::tests::live_decisions -- --ignored --nocapture`.
#[test]
#[ignore = "calls the Decisions API"]
fn live_decisions_route_a_model_question_a_follow_up_and_new_work() {
    let cases = [
        ("Which model are you running on right now?", HOST_MODEL),
        ("Also add a regression test for that timeout.", CONTINUE),
        (
            "Unrelated: write me a short limerick about penguins for my niece's birthday card.",
            REBRIEF,
        ),
    ];
    for (latest, expected) in cases {
        let Ruled::Unsure {
            input,
            questions,
            memo,
        } = rule(Some(note(latest)), latest)
        else {
            panic!("expected a question");
        };
        let mut request = DecisionsRequest::new(coder_sdk::decision::MODEL, input);
        request.questions = questions;
        let started = std::time::Instant::now();
        let response = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(coder_sdk::decide::Api.decide(request))
            .unwrap();
        let acted = act(&memo, &response);
        println!(
            "{latest:?} -> {} (top {:?}) p={:?} via {} {:?} in {} ms",
            acted.action,
            top_choice(&response, QUESTION),
            acted.p,
            response.model,
            response.openagents.door,
            started.elapsed().as_millis()
        );
        // The top choice is the expected route; a re-brief acts only past
        // its threshold, and this one sits near it.
        assert_eq!(
            top_choice(&response, QUESTION)
                .map(|(route, _)| route)
                .as_deref(),
            Some(expected),
            "{latest}"
        );
        if expected != REBRIEF {
            assert_eq!(acted.action, expected, "{latest}");
        }
    }
}

/// The replay over this computer's recorded Claude Code chats, against the
/// real Decisions API. Run by hand:
/// `cargo test -p coder-new --lib message_route::tests::replay_recorded -- --ignored --nocapture`.
/// `CODER_REPLAY_SESSIONS` and `CODER_REPLAY_CLAUDE_PROJECTS` override where
/// the chats and Claude Code's session files are read.
#[test]
#[ignore = "reads recorded chats and calls the Decisions API"]
fn replay_recorded_chats() {
    let home = PathBuf::from(std::env::var("HOME").unwrap());
    let sessions = std::env::var("CODER_REPLAY_SESSIONS").map_or_else(
        |_| home.join(".openagents/coder-new/sessions"),
        PathBuf::from,
    );
    let projects = std::env::var("CODER_REPLAY_CLAUDE_PROJECTS")
        .map_or_else(|_| home.join(".claude/projects"), PathBuf::from);
    let chats = replay::chats(&sessions, &projects);
    let with_tokens = chats.iter().filter(|c| !c.prompts.is_empty()).count();
    println!(
        "{with_tokens} of {} chats have Claude Code's token record",
        chats.len()
    );
    let report = replay::replay(&chats, &coder_sdk::decide::Api);
    println!("{report}");
    assert!(report.messages > 0);
}

/// One real Claude Code chat: a first turn, then a later message the hook
/// answers from the host, so Claude Code never runs a turn for it. Run by
/// hand: `cargo test -p coder-new --lib message_route::tests::a_live_chat -- --ignored --nocapture`.
#[test]
#[ignore = "needs a Claude Code login and calls the Decisions API"]
fn a_live_chat_answers_a_model_question_from_the_host() {
    use crate::claude_chat::{Chat, Event as ChatEvent};
    fn finish(chat: &mut Chat, turn: u64) -> Result<openrouter::Streamed, String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
        while std::time::Instant::now() < deadline {
            for event in chat.poll() {
                match event {
                    ChatEvent::TurnEnd {
                        turn: ended,
                        result,
                    } if ended >= turn => return result,
                    ChatEvent::Exited => return Err("exited".into()),
                    _ => {}
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        Err("timed out".into())
    }
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().canonicalize().unwrap();
    let first = "Remember the word PELICAN. Reply with just OK.";
    let mut chat = Chat::start(
        cwd.clone(),
        crate::claude_tools::Tools::default(),
        None,
        first.into(),
        first.into(),
        None,
        Some(crate::claude::MODEL.into()),
    );
    let reply = finish(&mut chat, 1).unwrap();
    println!(
        "1: {} ({} tokens in)",
        reply.text, reply.usage.prompt_tokens
    );

    let latest = "Which AI model is answering me in this chat?";
    super::note(
        &chat.cwd,
        Note {
            earlier: vec![said("user", first), said("assistant", &reply.text)],
            latest: latest.into(),
            midturn: false,
            facts: Facts {
                model: Some(crate::claude::name(crate::claude::MODEL)),
                spend: None,
            },
        },
    );
    let turn = chat.turn();
    let started = std::time::Instant::now();
    chat.say(latest, false);
    let blocked = finish(&mut chat, turn).unwrap();
    println!(
        "2: {:?} ({} tokens in, {} ms)",
        blocked.text,
        blocked.usage.prompt_tokens,
        started.elapsed().as_millis()
    );
    let routed = take_routed(&chat.cwd);
    println!("routed: {routed:?}");
    assert!(
        matches!(&routed, Some(Routed::Host(text)) if text.contains("Haiku")),
        "{routed:?}"
    );
    assert_eq!(blocked.usage.prompt_tokens, 0, "no executor call");

    // The session goes on: the next message continues it.
    let turn = chat.turn();
    chat.say("What word did I ask you to remember? One word.", false);
    let reply = finish(&mut chat, turn).unwrap();
    println!("3: {}", reply.text);
    assert!(
        reply.text.to_uppercase().contains("PELICAN"),
        "{}",
        reply.text
    );
}
