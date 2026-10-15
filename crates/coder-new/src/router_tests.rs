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
                        Harness::OpenCode => Payer::OpenCode,
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
        pins: Vec::new(),
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
        pins: Vec::new(),
    }
}

#[test]
fn the_cheap_answer_step_has_the_name_the_turn_reads() {
    assert_eq!(Step::CheapAnswer.name(), CHEAP_ANSWER);
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
        opencode_on: false,
        opencode: crate::harness_plugin::Health {
            signed_in: false,
            at_limit: false,
        },
        openrouter_key: false,
        leaning: crate::routing_preferences::Leaning::default(),
        failed: Default::default(),
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
    // A cheap answer runs the Coder loop on a cheap model, never the
    // logins' strongest (#11537).
    assert!(app.live.cheap);
    match app.request.as_ref().map(|request| &request.kind) {
        Some(crate::live::Work::Microcoder { execution, .. }) => {
            assert_eq!(execution.cheap, Some(crate::route::Cheap::default()));
        }
        _ => panic!("a Coder loop turn"),
    }
    finish(&mut app);

    // A follow-up judged new repository work moves on from Coder's loop to
    // the harness the router picks, Claude Code here, which starts from
    // the transcript (#11696). A harness chat never moves again (#11556).
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
    assert!(!app.live.cheap, "the work runs on Claude Code's own path");
    assert_eq!(
        app.live.claude_resume(),
        None,
        "a new Claude Code session, from the transcript"
    );
    finish(&mut app);

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
            _ => None,
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
    // The record names the cheap model and why (#11537); a signed-out
    // test computer has no login, so the Codex plan's cheapest is named.
    let named = chose[0].model.as_ref().expect("the cheap model is named");
    assert_eq!(named.model, crate::route::CHEAP_CODEX_MODEL);
    assert!(
        named.reason.starts_with("A small answer"),
        "{}",
        named.reason
    );
    // Once on Claude Code the work is off the cheap model (#11696).
    for later in &chose[1..] {
        assert_ne!(
            later.model.as_ref().map(|named| named.model.as_str()),
            Some(crate::route::CHEAP_CODEX_MODEL)
        );
    }
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

fn press(app: &mut crate::App, code: crossterm::event::KeyCode) {
    use crossterm::event::{Event, KeyEvent, KeyModifiers};
    app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
}

/// An app with two harnesses admitted, so a message is a routing point.
fn routed_app() -> crate::App {
    crate::harness_plugin::sign_in_claude(true);
    set_test_judge(Some(Some(small_answer())));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.plugins.bundled.claude = true;
    app
}

fn screen(app: &mut crate::App) -> String {
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(70, 20)).unwrap();
    terminal
        .draw(|frame| crate::ui::render(frame, app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..20)
        .map(|y| (0..70).map(|x| buffer[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// While a message waits for its route it shows at once as the person's
/// message, with the input's spinner, not up to a second later (#11460).
#[test]
fn a_held_message_shows_at_once() {
    let mut app = routed_app();
    app.draft.text = "What is a trait object?".into();
    app.submit_live();
    assert!(app.routing_pending());
    assert!(app.chat_started());
    let shown = screen(&mut app);
    assert!(!shown.contains("OpenAgents Coder"), "{shown}");
    assert!(shown.contains("What is a trait object?"), "{shown}");
    assert!(shown.contains("Working"), "{shown}");
    app.poll_route();
    set_test_judge(None);
    crate::harness_plugin::sign_in_claude(false);
    let shown = screen(&mut app);
    assert_eq!(
        shown.matches("What is a trait object?").count(),
        1,
        "once the turn starts the message is shown once: {shown}"
    );
}

/// Esc while a message waits takes it back: it returns to the input
/// ahead of what was typed meanwhile, and nothing is sent or recorded.
#[test]
fn esc_takes_back_a_message_waiting_for_its_route() {
    let dir = temp("esc");
    crate::routing_record::record_to(Some((dir.clone(), healthy_facts())));
    let mut app = routed_app();
    app.draft.text = "What is a trait object?".into();
    app.submit_live();
    assert!(app.routing_pending());
    app.draft.text = "and a vtable".into();
    press(&mut app, crossterm::event::KeyCode::Esc);
    assert!(!app.routing_pending());
    app.poll_route();
    set_test_judge(None);
    crate::routing_record::record_to(None);
    crate::harness_plugin::sign_in_claude(false);
    let lines = crate::routing_record::read(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(app.live.entries.is_empty(), "nothing was sent");
    assert!(app.request.is_none());
    assert!(!app.live.busy);
    assert_eq!(app.draft.text, "What is a trait object?\nand a vtable");
    assert_eq!(app.draft.cursor, app.draft.text.len());
    assert!(lines.is_empty(), "no route was chosen");
}

/// Opening an agent's chat while a message waits leaves the message in
/// the main chat's input; it is never sent into the agent's chat.
#[test]
fn switching_chats_keeps_the_waiting_message_with_its_own_chat() {
    let mut app = routed_app();
    app.delegations.push(crate::live::Delegation {
        id: "child".into(),
        name: "helper".into(),
        task: "Look around.".into(),
        chat: crate::live::Chat::default(),
        started_at: 0,
        elapsed_seconds: 0,
        running: true,
        draft: Default::default(),
        composer: Default::default(),
        scroll: 0,
        background: false,
    });
    app.draft.text = "Fix the parser".into();
    app.submit_live();
    assert!(app.routing_pending());
    app.select_agent(Some(0));
    app.poll_route();
    assert!(!app.routing_pending());
    assert!(app.delegations[0].chat.entries.is_empty());
    assert!(app.live.entries.is_empty());
    assert!(app.draft.text.is_empty(), "the agent's input stays its own");
    app.select_agent(None);
    set_test_judge(None);
    crate::harness_plugin::sign_in_claude(false);
    assert_eq!(app.draft.text, "Fix the parser");
}

/// An answer that lands after another chat was opened is not sent there:
/// the message goes back to the input.
#[test]
fn an_answer_for_another_chat_is_not_sent_into_this_one() {
    let mut app = routed_app();
    app.draft.text = "Fix the parser".into();
    app.submit_live();
    if let Some(pending) = app.pending_route.as_mut() {
        pending.session = Some("an-earlier-chat".into());
    }
    app.poll_route();
    set_test_judge(None);
    crate::harness_plugin::sign_in_claude(false);
    assert!(!app.routing_pending());
    assert!(app.live.entries.is_empty());
    assert!(app.request.is_none());
    assert_eq!(app.draft.text, "Fix the parser");
}

/// Sending the same message again marks the earlier turn redone in the
/// records, after its outcome.
#[test]
fn the_same_message_again_marks_the_earlier_turn_redone() {
    let dir = temp("redo");
    crate::harness_plugin::sign_in_claude(false);
    crate::routing_record::record_to(Some((dir.clone(), healthy_facts())));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    for _ in 0..2 {
        app.draft.text = "Fix the parser".into();
        app.submit_live();
        finish(&mut app);
    }
    app.record_end(Some(crate::routing_record::Ended::Done), None, None);
    crate::routing_record::record_to(None);
    let lines = crate::routing_record::read(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let first = match &lines[0] {
        crate::routing_record::Line::Chose(chose) => chose.id.clone(),
        other => panic!("a choice first, got {other:?}"),
    };
    let redone: Vec<_> = lines
        .iter()
        .filter_map(|line| match line {
            crate::routing_record::Line::Redone(redone) => Some(redone),
            _ => None,
        })
        .collect();
    assert_eq!(redone.len(), 1, "{lines:?}");
    assert_eq!(redone[0].id, first);
    assert_eq!(redone[0].why, crate::routing_record::Redo::SameMessage);
}

/// Router v0 on the Terminal-Bench development tasks (#11461): each
/// task's instruction as a chat's first message, with System One's
/// recorded judgment and the pick the frozen policy makes from it. The
/// Gym (`gym coder router`) prices those picks against the oracle and the
/// fixed rule.
const TASKS: &str = include_str!("../../../bench/terminal-bench/profiles/route-v1-tasks.json");

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct TaskPick {
    harness: Harness,
    step: Step,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct TaskRow {
    task: String,
    instruction: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    judged: Option<Recorded>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    door: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pick: Option<TaskPick>,
}

/// The situation every task row is routed in: a first message, Claude
/// Code and the Coder loop admitted, default sliders.
fn task_situation() -> Situation {
    situation_of(&Row {
        id: String::new(),
        earlier: Vec::new(),
        latest: String::new(),
        chat_on: None,
        situation: None,
        label: Judgment {
            kind: Kind::Answer,
            size: Size::Small,
            needs_repo: false,
            own_tools: false,
            risk: Risk::Low,
            new_work: None,
        },
        expect: Expect {
            harness: Harness::CoderLoop,
            step: Step::OnlyOne,
        },
        judged: None,
    })
}

fn task_rows() -> (serde_json::Value, Vec<TaskRow>) {
    let value: serde_json::Value = serde_json::from_str(TASKS).expect("the task set parses");
    let rows = serde_json::from_value(value["rows"].clone()).expect("the rows parse");
    (value, rows)
}

/// The picks in the checked-in file are what the frozen policy makes of
/// the recorded judgments, so the Gym's report prices this policy.
#[test]
fn the_terminal_bench_picks_are_the_frozen_policys() {
    let (value, rows) = task_rows();
    assert_eq!(value["set"], QUESTION_SET);
    assert_eq!(value["policy"], POLICY);
    assert_eq!(rows.len(), 8);
    for row in rows {
        let judgment = row
            .judged
            .as_ref()
            .and_then(Recorded::judgment)
            .unwrap_or_else(|| panic!("{} has a recorded judgment", row.task));
        let decision = policy(&judgment, &task_situation());
        let pick = row.pick.as_ref().expect("a pick");
        assert_eq!(
            (pick.harness, pick.step),
            (decision.harness, decision.step),
            "{}",
            row.task
        );
    }
}

/// Asks the gateway door about each Terminal-Bench task. With
/// `ROUTER_RECORD=1` it writes the judgments and picks into
/// `bench/terminal-bench/profiles/route-v1-tasks.json`.
#[test]
#[ignore = "asks openagents.com"]
fn the_gateway_judges_the_terminal_bench_tasks() {
    let (mut value, mut rows) = task_rows();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for row in &mut rows {
        let request = jev::SystemOneRequest::new(state(&[], &row.instruction), questions(false));
        let (response, door) = runtime
            .block_on(crate::issue_run::decide::ask(
                crate::issue_run::Decider::Gateway,
                request,
            ))
            .unwrap_or_else(|error| panic!("{}: {error}", row.task));
        let recorded = Recorded::from_response(&response).expect("every question answered");
        let judgment = recorded.judgment().expect("the answers read");
        let decision = policy(&judgment, &task_situation());
        println!(
            "{:<24} {:?}/{:?} repo={} own={} risk={:?} -> {:?} by {:?}",
            row.task,
            judgment.kind,
            judgment.size,
            judgment.needs_repo,
            judgment.own_tools,
            judgment.risk,
            decision.harness,
            decision.step
        );
        row.judged = Some(recorded);
        row.model = Some(response.model.clone());
        row.door = Some(door);
        row.pick = Some(TaskPick {
            harness: decision.harness,
            step: decision.step,
        });
    }
    if std::env::var_os("ROUTER_RECORD").is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bench/terminal-bench/profiles/route-v1-tasks.json");
        value["rows"] = serde_json::to_value(&rows).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap() + "\n").unwrap();
        println!("recorded into {}", path.display());
    }
}

/// #11689: what the OpenCode plugin's description says. With OpenCode on
/// and Claude Code off, a new chat that opens with a quick question gets a
/// cheap answer from the Coder loop (and keeps it: a chat never hops,
/// #11556); one that opens with repository work runs on OpenCode.
#[test]
fn a_new_chat_opens_on_opencode_unless_it_starts_with_a_quick_question() {
    let situation = Situation {
        admitted: vec![Harness::OpenCode, Harness::CoderLoop],
        preferred: None,
        payers: vec![
            (Harness::OpenCode, Payer::OpenCode),
            (Harness::CoderLoop, Payer::GatewayCredits),
        ],
        current: None,
        claude_five_hour: None,
        time_weight: crate::preferences::BALANCED_TIME_WEIGHT,
        pins: Vec::new(),
    };
    let judgment = |kind, needs_repo| Judgment {
        kind,
        size: Size::Small,
        needs_repo,
        own_tools: false,
        risk: Risk::Low,
        new_work: None,
    };
    assert_eq!(
        policy(&judgment(Kind::Answer, false), &situation),
        Decision {
            harness: Harness::CoderLoop,
            step: Step::CheapAnswer
        }
    );
    assert_eq!(
        policy(&judgment(Kind::RepoChange, true), &situation).harness,
        Harness::OpenCode
    );
    assert_eq!(
        policy(&judgment(Kind::Answer, true), &situation).harness,
        Harness::OpenCode
    );
}

/// #11696: a chat on Coder's own loop continues there only for another
/// small answer; repository work after it is routed and goes to the
/// harness (OpenCode here). A harness chat continues as before.
#[test]
fn coders_loop_continues_only_small_answers() {
    let situation = |current| Situation {
        admitted: vec![Harness::OpenCode, Harness::CoderLoop],
        preferred: None,
        payers: vec![
            (Harness::OpenCode, Payer::OpenCode),
            (Harness::CoderLoop, Payer::GatewayCredits),
        ],
        current: Some(current),
        claude_five_hour: None,
        time_weight: crate::preferences::BALANCED_TIME_WEIGHT,
        pins: Vec::new(),
    };
    let judgment = |kind, needs_repo| Judgment {
        kind,
        size: Size::Small,
        needs_repo,
        own_tools: false,
        risk: Risk::Low,
        new_work: Some(false),
    };
    assert_eq!(
        policy(
            &judgment(Kind::Answer, false),
            &situation(Harness::CoderLoop)
        ),
        Decision {
            harness: Harness::CoderLoop,
            step: Step::Continue
        }
    );
    let repo = policy(
        &judgment(Kind::Answer, true),
        &situation(Harness::CoderLoop),
    );
    assert_eq!(repo.harness, Harness::OpenCode, "{repo:?}");
    assert_ne!(repo.step, Step::Continue);
    assert_eq!(
        policy(
            &judgment(Kind::RepoChange, true),
            &situation(Harness::OpenCode)
        ),
        Decision {
            harness: Harness::OpenCode,
            step: Step::Continue
        }
    );
}
