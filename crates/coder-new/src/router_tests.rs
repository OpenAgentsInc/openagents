//! Router v0's tests: the policy over the labelled set, the recorded
//! judgments replayed with no network, the fallback, the routing record,
//! the held message, and one ignored live run against the gateway door.

use super::*;
use crate::harness_plugin::Harness;
use crate::routing_record::Rule;

/// The labelled set.
const FIXTURE: &str = include_str!("../fixtures/router/route-v1.json");

/// The share of rows whose recorded judgments must route as labelled. Set
/// to what the recorded run measured, so a policy change that loses rows
/// fails here.
const RECORDED_GATE: f64 = 0.90;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct Fixture {
    schema: String,
    set: String,
    policy: String,
    rows: Vec<Row>,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
struct Shape {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    claude: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    five_hour: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prefer: Option<Harness>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cost: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    speed: Option<u8>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct Expect {
    harness: Harness,
    step: Step,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct Row {
    id: String,
    earlier: Vec<Said>,
    latest: String,
    chat_on: Option<Harness>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    situation: Option<Shape>,
    label: Judgment,
    expect: Expect,
    judged: Option<Recorded>,
}

fn fixture() -> Fixture {
    serde_json::from_str(FIXTURE).expect("the labelled set parses")
}

/// The situation a row describes, over the default: Claude Code and the
/// Coder loop admitted, no preference, no window, balanced sliders.
fn situation_of(row: &Row) -> Situation {
    let shape = row.situation.clone().unwrap_or_default();
    let mut admitted = if shape.claude.unwrap_or(true) {
        vec![Harness::ClaudeCode, Harness::CoderLoop]
    } else {
        vec![Harness::CoderLoop]
    };
    if let Some(prefer) = shape.prefer {
        admitted.sort_by_key(|harness| *harness != prefer);
    }
    Situation {
        preferred: shape.prefer,
        payers: admitted
            .iter()
            .map(|harness| {
                (
                    *harness,
                    match harness {
                        Harness::ClaudeCode => Payer::ClaudePlan,
                        Harness::Codex => Payer::ChatGptLogin,
                        Harness::CoderLoop => Payer::GatewayCredits,
                    },
                )
            })
            .collect(),
        admitted,
        current: row.chat_on,
        claude_five_hour: shape.five_hour,
        time_weight: crate::preferences::time_weight(
            shape.cost.unwrap_or(3),
            shape.speed.unwrap_or(3),
        ),
    }
}

#[test]
fn the_labelled_set_is_frozen_on_this_question_set_and_policy() {
    let fixture = fixture();
    assert_eq!(fixture.schema, "openagents.coder.route.labeled.v1");
    assert_eq!(fixture.set, QUESTION_SET);
    assert_eq!(fixture.policy, POLICY);
    assert!(fixture.rows.len() >= 30, "{}", fixture.rows.len());
    let mut ids: Vec<&str> = fixture.rows.iter().map(|row| row.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), fixture.rows.len(), "ids are unique");
    for row in &fixture.rows {
        // A follow-up row asks the new-work question and a first message
        // does not.
        assert_eq!(
            row.label.new_work.is_some(),
            row.chat_on.is_some(),
            "{}",
            row.id
        );
    }
}

/// The policy, given the judgment a correct System One gives, picks every
/// row's labelled harness by its labelled step.
#[test]
fn the_policy_routes_every_labelled_judgment_as_expected() {
    let mut wrong = Vec::new();
    for row in fixture().rows {
        let decision = policy(&row.label, &situation_of(&row));
        if (decision.harness, decision.step) != (row.expect.harness, row.expect.step) {
            wrong.push(format!(
                "{}: {:?} by {:?}, expected {:?} by {:?}",
                row.id, decision.harness, decision.step, row.expect.harness, row.expect.step
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The judgments the gateway door gave, replayed through the policy with
/// no network: the share routed as labelled stays at or above the
/// recorded run's, and the two checks #11460 names hold.
#[test]
fn the_recorded_judgments_route_the_labelled_set() {
    let rows: Vec<Row> = fixture()
        .rows
        .into_iter()
        .filter(|row| row.judged.is_some())
        .collect();
    assert!(rows.len() >= 30, "every row has recorded judgments");
    let mut right = 0usize;
    let mut report = String::new();
    for row in &rows {
        let judgment = row
            .judged
            .as_ref()
            .and_then(Recorded::judgment)
            .expect("a recorded judgment reads");
        let decision = policy(&judgment, &situation_of(row));
        let ok = decision.harness == row.expect.harness;
        right += usize::from(ok);
        report.push_str(&format!(
            "{} {}: {:?} by {:?}\n",
            if ok { "ok  " } else { "MISS" },
            row.id,
            decision.harness,
            decision.step
        ));
        if row.id == "first/answer-01" {
            assert_eq!(
                decision.harness,
                Harness::CoderLoop,
                "a small what-is goes the cheap path"
            );
        }
        if row.id == "first/issue-01" {
            assert_eq!(
                decision.harness,
                Harness::ClaudeCode,
                "fix issue #N goes to the plan's harness"
            );
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let share = right as f64 / rows.len() as f64;
    println!("{report}{right}/{} routed as labelled", rows.len());
    assert!(share >= RECORDED_GATE, "{report}{right}/{}", rows.len());
}

#[test]
fn a_follow_up_asks_whether_it_is_new_work() {
    let first = serde_json::to_value(questions(false)).unwrap();
    let follow = serde_json::to_value(questions(true)).unwrap();
    let ids = |value: &serde_json::Value| {
        value
            .as_object()
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    };
    assert_eq!(
        ids(&first),
        ["kind", "size", "needs_repo", "own_tools", "risk"]
    );
    assert_eq!(
        ids(&follow),
        [
            "kind",
            "size",
            "needs_repo",
            "own_tools",
            "risk",
            "new_work"
        ]
    );
    // Every option name reads back as the policy's type.
    for (name, _) in KINDS {
        assert!(parse::<Kind>(name).is_some(), "{name}");
    }
    for (name, _) in SIZES {
        assert!(parse::<Size>(name).is_some(), "{name}");
    }
    for (name, _) in RISKS {
        assert!(parse::<Risk>(name).is_some(), "{name}");
    }
}

#[test]
fn the_state_shows_the_latest_turns_cut_short() {
    let earlier: Vec<Said> = (0..10)
        .map(|n| Said {
            role: if n % 2 == 0 { "user" } else { "assistant" }.into(),
            text: format!("turn {n} {}", "x".repeat(800)),
        })
        .collect();
    let text = state(&earlier, "what next?");
    assert!(!text.contains("turn 3 "), "only the latest six turns");
    assert!(text.contains("Person: turn 4 "));
    assert!(text.contains("Coder: turn 9 "));
    assert!(text.contains('…'));
    assert!(text.ends_with("what next?"));
    assert!(!state(&[], "hi").contains("Earlier"));
}

fn small_answer() -> Recorded {
    Recorded {
        kind: [
            ("answer", 0.9),
            ("repo_change", 0.05),
            ("issue_to_pr", 0.0),
            ("research", 0.05),
        ]
        .into_iter()
        .map(|(name, p)| (name.to_owned(), p))
        .collect(),
        size: [("small", 0.8), ("medium", 0.15), ("large", 0.05)]
            .into_iter()
            .map(|(name, p)| (name.to_owned(), p))
            .collect(),
        needs_repo: 0.1,
        own_tools: 0.02,
        risk: [("low", 0.95), ("medium", 0.04), ("high", 0.01)]
            .into_iter()
            .map(|(name, p)| (name.to_owned(), p))
            .collect(),
        new_work: None,
    }
}

fn two_admitted(current: Option<Harness>) -> Situation {
    Situation {
        admitted: vec![Harness::ClaudeCode, Harness::CoderLoop],
        preferred: None,
        payers: vec![
            (Harness::ClaudeCode, Payer::ClaudePlan),
            (Harness::CoderLoop, Payer::GatewayCredits),
        ],
        current,
        claude_five_hour: None,
        time_weight: crate::preferences::BALANCED_TIME_WEIGHT,
    }
}

#[test]
fn an_answer_is_routed_and_every_probability_is_kept() {
    let routed = decide(
        Ok(Answered {
            recorded: small_answer(),
            model: "clef-flash".into(),
            door: "Clef-Flash, OpenAgents gateway H100".into(),
            ms: 61,
        }),
        &two_admitted(None),
        Duration::from_millis(70),
    );
    assert_eq!(routed.harness, Harness::CoderLoop);
    assert_eq!(routed.rule, Rule::Router);
    assert_eq!(routed.trace.step.as_deref(), Some("cheap-answer"));
    assert_eq!(routed.trace.ms, 61);
    assert_eq!(routed.trace.question_set, QUESTION_SET);
    assert_eq!(routed.trace.policy, POLICY);
    // Four kinds, three sizes, two yes-or-no, three risks.
    assert_eq!(routed.probabilities.len(), 12);
    assert!(routed.probabilities.iter().all(|p| p.model == "clef-flash"
        && p.door.contains("H100")
        && p.question.starts_with("route-v1.")));
}

#[test]
fn no_answer_falls_back_to_the_fixed_rule_and_a_follow_up_stays() {
    let first = decide(
        Err("System One did not answer in time.".into()),
        &two_admitted(None),
        BUDGET,
    );
    assert_eq!(first.harness, Harness::ClaudeCode);
    assert_eq!(first.rule, Rule::Fallback);
    assert!(first.probabilities.is_empty());
    assert!(first.trace.step.is_none());
    assert_eq!(
        first.trace.missing.as_deref(),
        Some("System One did not answer in time.")
    );
    let follow = decide(
        Err("unreachable".into()),
        &two_admitted(Some(Harness::CoderLoop)),
        BUDGET,
    );
    assert_eq!(follow.harness, Harness::CoderLoop);
    assert!(follow.trace.follow_up);
    // An answer naming an option the set lacks is no answer.
    let mut odd = small_answer();
    odd.kind = [("poem".to_owned(), 1.0)].into_iter().collect();
    let routed = decide(
        Ok(Answered {
            recorded: odd,
            model: "m".into(),
            door: "d".into(),
            ms: 1,
        }),
        &two_admitted(None),
        Duration::ZERO,
    );
    assert_eq!(routed.rule, Rule::Fallback);
    assert_eq!(routed.probabilities.len(), 9, "what it said is still kept");
}

fn healthy_facts() -> crate::harness_plugin::Facts {
    let healthy = crate::harness_plugin::Health {
        signed_in: true,
        at_limit: false,
    };
    crate::harness_plugin::Facts {
        local: true,
        claude_plugin_on: true,
        claude: healthy,
        codex_on: false,
        codex: healthy,
        openrouter_key: false,
        leaning: crate::routing_preferences::Leaning::default(),
    }
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "coder-router-test-{name}-{}-{}",
        std::process::id(),
        atif::now_ms()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn finish(app: &mut crate::App) {
    let id = app.request_id;
    app.apply_update(crate::live::Update::Finished {
        id,
        result: Ok(openrouter::Streamed {
            text: "done".into(),
            model: "m".into(),
            ..Default::default()
        }),
    });
    // The terminal's loop takes the request; nothing takes it here.
    app.request = None;
}

/// The path a person takes with the router on: the message waits for
/// System One, then runs where the policy sends it, and the record keeps
/// the probabilities, the rule and the step. A follow-up judged a
/// continuation stays; one judged new work can move, and a move to Claude
/// Code starts its session from the transcript.
#[test]
fn a_held_message_runs_where_the_router_sends_it_and_is_recorded() {
    let dir = temp("held");
    crate::harness_plugin::sign_in_claude(true);
    crate::routing_record::record_to(Some((dir.clone(), healthy_facts())));
    set_test_judge(Some(Some(small_answer())));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.plugins.bundled.claude = true;
    app.draft.text = "What is a trait object?".into();
    app.submit_live();
    assert!(app.routing_pending(), "the message waits for its route");
    assert!(app.live.entries.is_empty());
    // The person keeps typing while it waits.
    app.draft.text = "next thought".into();
    app.poll_route();
    assert!(!app.routing_pending());
    assert_eq!(app.live.runs_on, Some(Harness::CoderLoop));
    assert!(matches!(
        app.live.entries.as_slice(),
        [crate::live::Entry::User(text)] if text == "What is a trait object?"
    ));
    assert_eq!(
        app.draft.text, "next thought",
        "what was typed meanwhile stays"
    );
    finish(&mut app);

    // A follow-up judged new repository work moves to Claude Code, whose
    // session starts from the whole chat.
    let mut work = small_answer();
    work.kind = [("repo_change".to_owned(), 0.9), ("answer".to_owned(), 0.1)]
        .into_iter()
        .collect();
    work.needs_repo = 0.9;
    work.new_work = Some(0.85);
    set_test_judge(Some(Some(work)));
    app.draft.text = "Now add a --verbose flag to src/main.rs".into();
    app.submit_live();
    app.poll_route();
    assert_eq!(app.live.runs_on, Some(Harness::ClaudeCode));
    match app.request.take().map(|request| request.kind) {
        Some(crate::live::Work::Claude {
            session, messages, ..
        }) => {
            assert!(session.is_none(), "a new session, seeded from the chat");
            assert!(messages.len() >= 3, "the earlier turns go with it");
            assert_eq!(
                messages.last().map(|message| message.content.as_str()),
                Some("Now add a --verbose flag to src/main.rs")
            );
        }
        _ => panic!("a claude turn"),
    }
    app.live.busy = false;

    // A follow-up judged a continuation stays on Claude Code.
    let mut more = small_answer();
    more.new_work = Some(0.1);
    set_test_judge(Some(Some(more)));
    app.draft.text = "also print the config path".into();
    app.submit_live();
    app.poll_route();
    assert_eq!(app.live.runs_on, Some(Harness::ClaudeCode));
    app.live.busy = false;
    app.record_end(Some(crate::routing_record::Ended::Done), None, None);

    set_test_judge(None);
    crate::routing_record::record_to(None);
    crate::harness_plugin::sign_in_claude(false);
    let chose: Vec<crate::routing_record::ChoseLine> = crate::routing_record::read(&dir)
        .into_iter()
        .filter_map(|line| match line {
            crate::routing_record::Line::Chose(chose) => Some(chose),
            crate::routing_record::Line::Ended(_) => None,
        })
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(chose.len(), 3);
    let steps: Vec<_> = chose
        .iter()
        .map(|line| {
            (
                line.harness,
                line.rule,
                line.inputs
                    .router
                    .as_ref()
                    .and_then(|trace| trace.step.clone()),
            )
        })
        .collect();
    assert_eq!(
        steps,
        [
            (
                Harness::CoderLoop,
                Rule::Router,
                Some("cheap-answer".into())
            ),
            (Harness::ClaudeCode, Rule::Router, Some("plan-work".into())),
            (Harness::ClaudeCode, Rule::Router, Some("continue".into())),
        ]
    );
    assert_eq!(chose[0].inputs.probabilities.len(), 12);
    assert_eq!(
        chose[1].inputs.probabilities.len(),
        11,
        "two kinds, with new work"
    );
    assert!(chose.iter().all(|line| {
        line.inputs
            .router
            .as_ref()
            .is_some_and(|trace| trace.policy == POLICY)
    }));
}

/// System One that never answers: the message still goes, on the fixed
/// rule, recorded as the fallback.
#[test]
fn a_message_goes_on_the_fallback_when_system_one_is_silent() {
    let dir = temp("silent");
    crate::harness_plugin::sign_in_claude(true);
    crate::routing_record::record_to(Some((dir.clone(), healthy_facts())));
    set_test_judge(Some(None));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.plugins.bundled.claude = true;
    app.draft.text = "hello".into();
    app.submit_live();
    app.settle_route();
    assert_eq!(app.live.runs_on, Some(Harness::ClaudeCode));
    assert!(matches!(
        app.request.as_ref().map(|request| &request.kind),
        Some(crate::live::Work::Claude { .. })
    ));
    set_test_judge(None);
    crate::routing_record::record_to(None);
    crate::harness_plugin::sign_in_claude(false);
    let lines = crate::routing_record::read(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    match lines.first() {
        Some(crate::routing_record::Line::Chose(chose)) => {
            assert_eq!(chose.rule, Rule::Fallback);
            assert!(
                chose
                    .inputs
                    .router
                    .as_ref()
                    .is_some_and(|trace| trace.missing.is_some())
            );
        }
        other => panic!("a choice line, got {other:?}"),
    }
}

/// With one harness admitted there is nothing to choose: no request, and
/// the turn starts at once.
#[test]
fn one_admitted_harness_needs_no_request() {
    crate::harness_plugin::sign_in_claude(false);
    set_test_judge(Some(Some(small_answer())));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.draft.text = "hello".into();
    app.submit_live();
    set_test_judge(None);
    assert!(!app.routing_pending());
    assert_eq!(app.live.runs_on, Some(Harness::CoderLoop));
    assert!(app.live.busy);
}

/// The labelled set against the real gateway door (Clef-Flash on our H100):
/// `cargo test -p coder-new --lib router::tests::the_gateway_routes_the_labelled_set -- --ignored --nocapture`.
/// With `ROUTER_RECORD=1` it writes the answers into the fixture's
/// `judged` fields for the replay test.
#[test]
#[ignore = "asks openagents.com"]
fn the_gateway_routes_the_labelled_set() {
    let mut fixture = fixture();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut right = 0usize;
    let mut times = Vec::new();
    let mut doors = std::collections::BTreeSet::new();
    for row in &mut fixture.rows {
        let request = jev::SystemOneRequest::new(
            state(&row.earlier, &row.latest),
            questions(row.chat_on.is_some()),
        );
        let started = Instant::now();
        let (response, door) = runtime
            .block_on(crate::issue_run::decide::ask(
                crate::issue_run::Decider::Gateway,
                request,
            ))
            .unwrap_or_else(|error| panic!("{}: {error}", row.id));
        let ms = started.elapsed().as_millis();
        times.push(ms);
        doors.insert(format!("{door} ({})", response.model));
        let recorded = Recorded::from_response(&response).expect("every question answered");
        let judgment = recorded.judgment().expect("the answers read");
        let decision = policy(&judgment, &situation_of(row));
        let ok = decision.harness == row.expect.harness;
        right += usize::from(ok);
        println!(
            "{} {:<22} {:>4} ms  {:?}/{:?} repo={} own={} risk={:?} new={:?} -> {:?} by {:?} (expected {:?})",
            if ok { "ok  " } else { "MISS" },
            row.id,
            ms,
            judgment.kind,
            judgment.size,
            judgment.needs_repo,
            judgment.own_tools,
            judgment.risk,
            judgment.new_work,
            decision.harness,
            decision.step,
            row.expect.harness,
        );
        row.judged = Some(recorded);
    }
    times.sort_unstable();
    println!(
        "{right}/{} routed as labelled; median {} ms, slowest {} ms; doors: {doors:?}",
        fixture.rows.len(),
        times[times.len() / 2],
        times[times.len() - 1]
    );
    // The same request inside the router's own budget, as a message would
    // ask it.
    let row = &fixture.rows[0];
    let budgeted = ask_now(jev::SystemOneRequest::new(
        state(&row.earlier, &row.latest),
        questions(false),
    ));
    println!(
        "within the budget: {:?}",
        budgeted
            .as_ref()
            .map(|answered| (answered.ms, &answered.door))
    );
    assert!(budgeted.is_ok(), "{budgeted:?}");
    if std::env::var_os("ROUTER_RECORD").is_some() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/router/route-v1.json");
        let mut value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        value["rows"] = serde_json::to_value(&fixture.rows).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap() + "\n").unwrap();
        println!("recorded into {}", path.display());
    }
    for id in ["first/answer-01", "first/issue-01"] {
        let row = fixture.rows.iter().find(|row| row.id == id).unwrap();
        let judgment = row.judged.as_ref().and_then(Recorded::judgment).unwrap();
        assert_eq!(
            policy(&judgment, &situation_of(row)).harness,
            row.expect.harness,
            "{id}"
        );
    }
}
