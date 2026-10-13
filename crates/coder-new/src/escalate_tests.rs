//! Escalation's tests: the policy, the evidence and the briefing, and the
//! paths a person takes: failed checks move a Coder-loop turn to Claude
//! Code, a usage limit moves without a question, a redo moves, and a
//! multi-part ask runs as a plan with one record per part.

use super::*;
use crate::routing_record::Rule;

fn situation(admitted: &[Harness]) -> Situation {
    Situation {
        current: Harness::CoderLoop,
        admitted: admitted.to_vec(),
        claude_five_hour: None,
        retried: false,
        moved: false,
    }
}

fn judged(action: Action, reason: Reason) -> Judgment {
    Judgment { action, reason }
}

#[test]
fn the_policy_moves_retries_or_stays_in_its_order() {
    let both = situation(&[Harness::ClaudeCode, Harness::CoderLoop]);
    let only_loop = situation(&[Harness::CoderLoop]);
    let move_it = judged(Action::Move, Reason::BeyondAgent);
    let decide = |j: Option<&Judgment>, c, s: &Situation| {
        let d = policy(j, c, s);
        (d.call, d.step)
    };
    assert_eq!(
        decide(Some(&move_it), Checkpoint::FailedChecks, &both),
        (Call::To(Harness::ClaudeCode), Step::JudgedMove)
    );
    assert_eq!(
        decide(Some(&move_it), Checkpoint::FailedChecks, &only_loop),
        (Call::Retry, Step::NowhereElse)
    );
    assert_eq!(
        decide(
            Some(&judged(Action::Retry, Reason::Slip)),
            Checkpoint::FailedChecks,
            &both
        ),
        (Call::Retry, Step::JudgedRetry)
    );
    let mut retried = both.clone();
    retried.retried = true;
    assert_eq!(
        decide(
            Some(&judged(Action::Retry, Reason::Slip)),
            Checkpoint::FailedChecks,
            &retried
        ),
        (Call::Stay, Step::JudgedStay)
    );
    assert_eq!(
        decide(
            Some(&judged(Action::Move, Reason::NeedsPerson)),
            Checkpoint::Error,
            &both
        ),
        (Call::Stay, Step::NeedsPerson)
    );
    let mut near = both.clone();
    near.claude_five_hour = Some(95);
    assert_eq!(
        decide(Some(&move_it), Checkpoint::FailedChecks, &near),
        (Call::Stay, Step::NearLimit)
    );
    assert_eq!(
        decide(None, Checkpoint::FailedChecks, &both),
        (Call::Stay, Step::Silent)
    );
    // A usage limit moves anywhere else, without a question.
    let mut on_claude = both.clone();
    on_claude.current = Harness::ClaudeCode;
    assert_eq!(
        decide(None, Checkpoint::Limit, &on_claude),
        (Call::To(Harness::CoderLoop), Step::LimitMove)
    );
    let mut moved = both;
    moved.moved = true;
    assert_eq!(
        decide(Some(&move_it), Checkpoint::FailedChecks, &moved),
        (Call::Stay, Step::MovedOnce)
    );
}

#[test]
fn a_checkpoint_is_an_error_a_limit_a_stall_or_failed_checks() {
    assert_eq!(reached(None, false, false, Some(true)), None);
    assert_eq!(reached(None, false, false, None), None);
    assert_eq!(
        reached(None, false, false, Some(false)),
        Some(Checkpoint::FailedChecks)
    );
    assert_eq!(
        reached(None, false, true, Some(true)),
        Some(Checkpoint::Stall)
    );
    assert_eq!(
        reached(Some("boom"), false, false, None),
        Some(Checkpoint::Error)
    );
    assert_eq!(
        reached(Some("boom"), true, false, None),
        Some(Checkpoint::Limit)
    );
    assert_eq!(
        reached(
            Some("OpenRouter is rate limited (HTTP 429). It resets in 90 seconds."),
            false,
            false,
            None
        ),
        Some(Checkpoint::Limit)
    );
}

fn tool(name: &str, input: serde_json::Value, output: serde_json::Value) -> crate::live::Entry {
    crate::live::Entry::Tool {
        name: name.into(),
        input,
        output,
        running: false,
    }
}

fn failing_turn() -> Vec<crate::live::Entry> {
    vec![
        tool(
            "Edit",
            serde_json::json!({"path": "src/lib.rs"}),
            serde_json::json!({"path": "src/lib.rs"}),
        ),
        tool(
            "Run",
            serde_json::json!({"command": "cargo test -p demo"}),
            serde_json::json!({"exit": 101, "text": "test parses ... FAILED\nassertion failed: left == right"}),
        ),
        crate::live::Entry::Assistant {
            text: "I could not get the test to pass.".into(),
            model: None,
            elapsed_ms: None,
        },
    ]
}

#[test]
fn the_briefing_names_the_task_the_failing_check_and_the_files() {
    let found = evidence(&failing_turn());
    assert_eq!(found.files, ["src/lib.rs"]);
    let (command, output) = found.failed_check.clone().unwrap();
    assert_eq!(command, "cargo test -p demo");
    assert!(output.contains("assertion failed"));
    let text = handoff(
        "Claude (Haiku 5.5)",
        "Coder",
        Checkpoint::FailedChecks,
        "make the parser test pass",
        &found,
    );
    assert!(text.starts_with(
        "Claude (Haiku 5.5) is taking over from Coder: the checks still fail.\n\nThe task: make the parser test pass"
    ));
    assert!(text.contains("The failing check: `cargo test -p demo`"));
    assert!(text.contains("Files changed so far: src/lib.rs."));
    assert_eq!(
        retry_notice(Checkpoint::Error),
        "The turn ended with an error. Send a message to try again."
    );
    // A later passing run clears the failure.
    let mut fixed = failing_turn();
    fixed.push(tool(
        "Run",
        serde_json::json!({"command": "cargo test -p demo"}),
        serde_json::json!({"exit": 0, "text": "ok"}),
    ));
    assert!(evidence(&fixed).failed_check.is_none());
}

fn record_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "coder-escalate-test-{name}-{}-{}",
        std::process::id(),
        atif::now_ms()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn facts() -> crate::harness_plugin::Facts {
    let health = crate::harness_plugin::Health {
        signed_in: true,
        at_limit: false,
    };
    crate::harness_plugin::Facts {
        local: true,
        claude_plugin_on: true,
        claude: health,
        codex_on: false,
        codex: health,
        opencode_on: false,
        opencode: crate::harness_plugin::Health {
            signed_in: false,
            at_limit: false,
        },
        openrouter_key: false,
        leaning: crate::routing_preferences::Leaning::default(),
    }
}

fn map(pairs: &[(&str, f64)]) -> std::collections::BTreeMap<String, f64> {
    pairs.iter().map(|(n, p)| ((*n).to_owned(), *p)).collect()
}

/// A route judgment: a small answer (the Coder loop), or new repository
/// work (Claude Code).
fn route(answer: bool, new_work: Option<f64>) -> crate::router::Recorded {
    crate::router::Recorded {
        kind: if answer {
            map(&[("answer", 0.9), ("repo_change", 0.1)])
        } else {
            map(&[("repo_change", 0.9), ("answer", 0.1)])
        },
        size: map(&[("small", 0.9), ("medium", 0.1)]),
        needs_repo: if answer { 0.1 } else { 0.9 },
        own_tools: 0.0,
        risk: map(&[("low", 0.9), ("medium", 0.1)]),
        new_work,
    }
}

fn move_answer() -> Recorded {
    Recorded {
        action: map(&[("move", 0.8), ("retry", 0.15), ("continue", 0.05)]),
        reason: map(&[
            ("beyond_agent", 0.7),
            ("slip", 0.2),
            ("needs_person", 0.05),
            ("outside", 0.05),
        ]),
    }
}

/// Ends the running turn with the given entries and result.
fn end_turn(app: &mut crate::App, entries: Vec<crate::live::Entry>, error: Option<&str>) {
    app.live.entries.extend(entries);
    let id = app.request_id;
    app.request = None;
    app.apply_update(crate::live::Update::Finished {
        id,
        result: match error {
            Some(error) => Err(error.into()),
            None => Ok(openrouter::Streamed {
                text: "done".into(),
                model: "m".into(),
                ..Default::default()
            }),
        },
    });
}

fn lines(dir: &std::path::Path) -> Vec<crate::routing_record::Line> {
    crate::routing_record::read(dir)
}

fn chose(dir: &std::path::Path) -> Vec<crate::routing_record::ChoseLine> {
    lines(dir)
        .into_iter()
        .filter_map(|line| match line {
            crate::routing_record::Line::Chose(chose) => Some(chose),
            _ => None,
        })
        .collect()
}

fn escalated(dir: &std::path::Path) -> Vec<crate::routing_record::EscalatedLine> {
    lines(dir)
        .into_iter()
        .filter_map(|line| match line {
            crate::routing_record::Line::Escalated(line) => Some(line),
            _ => None,
        })
        .collect()
}

/// A Coder-loop turn whose tests fail moves to Claude Code with a
/// briefing, and the records show the move.
#[test]
fn failing_tests_on_the_coder_loop_move_to_claude_code_with_a_briefing() {
    let dir = record_dir("move");
    crate::harness_plugin::sign_in_claude(true);
    crate::routing_record::record_to(Some((dir.clone(), facts())));
    crate::router::set_test_judge(Some(Some(route(true, None))));
    set_test_judge(Some(Some(move_answer())));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.plugins.bundled.claude = true;
    app.draft.text = "make the parser test pass".into();
    app.submit_live();
    app.poll_route();
    assert_eq!(app.live.runs_on, Some(Harness::CoderLoop));
    end_turn(&mut app, failing_turn(), None);
    assert!(
        app.escalation_pending(),
        "the checkpoint waits for its call"
    );
    app.poll_escalation();
    assert!(!app.escalation_pending());
    assert_eq!(app.live.runs_on, Some(Harness::ClaudeCode));
    // Nothing goes without the person: the briefing waits in the input,
    // and the rail says so.
    assert!(app.request.is_none(), "Coder sent a message nobody wrote");
    assert!(
        app.draft
            .text
            .contains("is taking over from Coder: the checks still fail.")
    );
    assert!(
        app.rail_notice_text()
            .is_some_and(|notice| notice.contains("can take this over"))
    );
    app.submit_live();
    app.poll_route();
    let Some(crate::live::Work::Claude {
        messages, session, ..
    }) = app.request.take().map(|request| request.kind)
    else {
        panic!("a Claude Code turn");
    };
    assert!(session.is_none(), "a new session, seeded from the chat");
    let last = &messages.last().unwrap().content;
    assert!(
        last.contains("is taking over from Coder: the checks still fail."),
        "{last}"
    );
    assert!(last.contains("The task: make the parser test pass"));
    assert!(last.contains("cargo test -p demo"));
    // Claude Code's turn fails too: the work moved once and stays.
    app.live.busy = true;
    end_turn(&mut app, failing_turn(), None);
    assert!(!app.escalation_pending());

    crate::router::set_test_judge(None);
    set_test_judge(None);
    crate::routing_record::record_to(None);
    crate::harness_plugin::sign_in_claude(false);
    let choices = chose(&dir);
    let moves = escalated(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        choices
            .iter()
            .map(|line| (line.harness, line.rule))
            .collect::<Vec<_>>(),
        [
            (Harness::CoderLoop, Rule::Router),
            (Harness::ClaudeCode, Rule::Escalated)
        ]
    );
    assert_eq!(moves.len(), 1);
    let line = &moves[0];
    assert_eq!(line.from, Harness::CoderLoop);
    assert_eq!(line.to, Some(Harness::ClaudeCode));
    assert_eq!(line.checkpoint, Checkpoint::FailedChecks);
    assert_eq!(line.action, "move");
    assert_eq!(line.step, "judged-move");
    assert_eq!(
        line.id, choices[0].id,
        "the line names the turn that failed"
    );
    assert_eq!(line.probabilities.len(), 7);
    assert!(
        choices[1]
            .inputs
            .router
            .as_ref()
            .is_some_and(|trace| trace.question_set == QUESTION_SET)
    );
}

/// The gateway's usage limit moves the Coder loop's work to Claude Code
/// without a question.
#[test]
fn a_usage_limit_moves_the_work_without_a_question() {
    let dir = record_dir("limit");
    crate::harness_plugin::sign_in_claude(true);
    crate::routing_record::record_to(Some((dir.clone(), facts())));
    crate::router::set_test_judge(Some(Some(route(true, None))));
    // A judge that never answers: the limit needs none.
    set_test_judge(Some(None));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.plugins.bundled.claude = true;
    app.draft.text = "what does this crate do".into();
    app.submit_live();
    app.poll_route();
    assert_eq!(app.live.runs_on, Some(Harness::CoderLoop));
    end_turn(
        &mut app,
        Vec::new(),
        Some("OpenRouter is rate limited (HTTP 429). It resets in 3600 seconds."),
    );
    app.poll_escalation();
    assert_eq!(app.live.runs_on, Some(Harness::ClaudeCode));
    assert!(app.request.is_none(), "Coder sent a message nobody wrote");
    app.draft.text = "go on".into();
    app.submit_live();
    app.poll_route();
    assert!(matches!(
        app.request.as_ref().map(|request| &request.kind),
        Some(crate::live::Work::Claude { .. })
    ));
    crate::router::set_test_judge(None);
    set_test_judge(None);
    crate::routing_record::record_to(None);
    crate::harness_plugin::sign_in_claude(false);
    let moves = escalated(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(moves[0].checkpoint, Checkpoint::Limit);
    assert_eq!(moves[0].step, "limit-move");
    assert!(moves[0].probabilities.is_empty());
}

/// Asking for the same thing again is a checkpoint: the redo moves to the
/// stronger harness with a briefing in place of the repeated message.
#[test]
fn a_redo_moves_to_the_stronger_harness() {
    let dir = record_dir("redo");
    crate::harness_plugin::sign_in_claude(true);
    crate::routing_record::record_to(Some((dir.clone(), facts())));
    crate::router::set_test_judge(Some(Some(route(true, None))));
    set_test_judge(Some(Some(move_answer())));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.plugins.bundled.claude = true;
    app.draft.text = "explain why the build is slow".into();
    app.submit_live();
    app.poll_route();
    assert_eq!(app.live.runs_on, Some(Harness::CoderLoop));
    end_turn(&mut app, Vec::new(), None);
    assert!(!app.escalation_pending(), "a plain answer is no checkpoint");
    app.draft.text = "explain why the build is slow".into();
    app.submit_live();
    assert!(app.routing_pending());
    app.poll_route();
    assert_eq!(app.live.runs_on, Some(Harness::ClaudeCode));
    let Some(crate::live::Work::Claude { messages, .. }) =
        app.request.take().map(|request| request.kind)
    else {
        panic!("a Claude Code turn");
    };
    // The person's own words go, never a briefing in their place.
    assert_eq!(
        messages.last().unwrap().content.trim(),
        "explain why the build is slow"
    );
    assert!(app.live.entries.iter().all(
        |entry| !matches!(entry, crate::live::Entry::User(text) if text.contains("taking over"))
    ));
    crate::router::set_test_judge(None);
    set_test_judge(None);
    crate::routing_record::record_to(None);
    crate::harness_plugin::sign_in_claude(false);
    let moves = escalated(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].checkpoint, Checkpoint::Redo);
    assert_eq!(moves[0].to, Some(Harness::ClaudeCode));
}

/// A multi-part ask produces a plan; each part routes as its own piece of
/// work with its own record, and one combined report comes back.
#[test]
fn a_multi_part_ask_runs_as_a_plan_with_one_record_per_part() {
    let dir = record_dir("plan");
    crate::harness_plugin::sign_in_claude(true);
    crate::routing_record::record_to(Some((dir.clone(), facts())));
    crate::router::set_test_judge(Some(Some(route(false, None))));
    crate::dispatch::set_test_judge(Some(Some(0.9)));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.plugins.bundled.claude = true;
    app.draft.text =
        "In this repo:\n1. Add a --verbose flag\n2. Fix the typo in README\n3. Bump the version"
            .into();
    app.submit_live();
    app.poll_route();
    // The outline shows, and part 1 waits for its own route.
    assert!(app.plan.is_some());
    assert!(app.routing_pending(), "part 1 is routed on its own");
    app.poll_route();
    for part in 1..=3 {
        assert_eq!(app.live.runs_on, Some(Harness::ClaudeCode), "part {part}");
        app.request = None;
        app.live.busy = true;
        end_turn(&mut app, Vec::new(), None);
        if part < 3 {
            assert!(
                app.routing_pending(),
                "part {} waits for its route",
                part + 1
            );
            app.poll_route();
        }
    }
    assert!(app.plan.is_none());
    let report = match app.live.entries.last() {
        Some(crate::live::Entry::Assistant { text, .. }) => text.clone(),
        other => panic!("the report, got {other:?}"),
    };
    crate::router::set_test_judge(None);
    crate::dispatch::set_test_judge(None);
    crate::routing_record::record_to(None);
    crate::harness_plugin::sign_in_claude(false);
    let choices = chose(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(report.starts_with("All 3 parts are done:"), "{report}");
    assert!(report.contains("2. Fix the typo in README: Claude ("));
    let parts: Vec<(u32, u32)> = choices
        .iter()
        .map(|line| {
            let part = line
                .inputs
                .plan
                .as_ref()
                .expect("each part's record names it");
            (part.part, part.parts)
        })
        .collect();
    assert_eq!(parts, [(1, 3), (2, 3), (3, 3)]);
    assert!(choices.iter().all(|line| line.rule == Rule::Router));
    let ids: std::collections::BTreeSet<&str> = choices
        .iter()
        .filter_map(|line| line.inputs.plan.as_ref().map(|p| p.plan.as_str()))
        .collect();
    assert_eq!(ids.len(), 1, "one plan");
    let first = match &app.live.entries[0] {
        crate::live::Entry::User(text) => text.clone(),
        other => panic!("{other:?}"),
    };
    assert!(first.starts_with("In this repo:\n1. Add"));
}

/// Escalation reaches Codex and leaves it (#11510): from Coder's loop to
/// Claude Code, else Codex; from Codex to Claude Code; nothing above Claude.
#[test]
fn escalation_goes_to_and_from_codex() {
    use crate::harness_plugin::Harness::{ClaudeCode, CoderLoop, Codex};
    assert_eq!(
        stronger(CoderLoop, &[ClaudeCode, CoderLoop, Codex]),
        Some(ClaudeCode)
    );
    assert_eq!(stronger(CoderLoop, &[CoderLoop, Codex]), Some(Codex));
    assert_eq!(
        stronger(Codex, &[ClaudeCode, CoderLoop, Codex]),
        Some(ClaudeCode)
    );
    assert_eq!(stronger(Codex, &[CoderLoop, Codex]), None);
    assert_eq!(stronger(ClaudeCode, &[ClaudeCode, CoderLoop, Codex]), None);
    assert_eq!(other(Codex, &[CoderLoop, Codex]), Some(CoderLoop));
}

/// A retry at a checkpoint is the person's to send: Coder adds no
/// "That did not work yet… Try again" message as if they typed it
/// (owner, 2026-10-13); the rail says what happened instead.
#[test]
fn a_retry_never_sends_a_message_the_person_did_not_write() {
    let dir = record_dir("retry");
    crate::harness_plugin::sign_in_claude(true);
    crate::routing_record::record_to(Some((dir.clone(), facts())));
    crate::router::set_test_judge(Some(Some(route(true, None))));
    set_test_judge(Some(Some(Recorded {
        action: map(&[("retry", 0.8), ("move", 0.1), ("continue", 0.1)]),
        reason: map(&[
            ("slip", 0.8),
            ("beyond_agent", 0.1),
            ("needs_person", 0.05),
            ("outside", 0.05),
        ]),
    })));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.plugins.bundled.claude = true;
    app.draft.text = "make the parser test pass".into();
    app.submit_live();
    app.poll_route();
    end_turn(&mut app, failing_turn(), None);
    let users_before = app.live.user_count();
    app.poll_escalation();
    crate::router::set_test_judge(None);
    set_test_judge(None);
    crate::routing_record::record_to(None);
    crate::harness_plugin::sign_in_claude(false);
    let moves = escalated(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].action, "retry");
    assert!(app.request.is_none(), "Coder sent a message nobody wrote");
    assert_eq!(app.live.user_count(), users_before);
    assert!(app.live.entries.iter().all(|entry| !matches!(
        entry,
        crate::live::Entry::User(text) if text.contains("did not work yet")
    )));
    assert_eq!(
        app.rail_notice_text(),
        Some("The checks still fail. Send a message to try again.")
    );
    assert!(app.draft.text.is_empty());
}
