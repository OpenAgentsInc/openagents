use super::*;

const QUESTION_THEN_RESULT: &str =
    include_str!("../../../openagents-chat/fixtures/coder-events/question-then-result.ndjson");
const OTHER_ENDINGS: &str =
    include_str!("../../../openagents-chat/fixtures/coder-events/other-endings.ndjson");

/// The scripted provider's lines, one task at a time.
pub(crate) fn tasks(text: &str) -> Vec<Vec<Line>> {
    let mut out: Vec<Vec<Line>> = vec![];
    for line in text.lines() {
        let line: Line = serde_json::from_str(line).unwrap();
        match out.last_mut() {
            Some(task) if task[0].task == line.task => task.push(line),
            _ => out.push(vec![line]),
        }
    }
    out
}

/// A run fed `lines` as one poll, ending in `state`.
fn fed(lines: &[Line], state: State) -> Run {
    let now = Instant::now();
    let mut run = Run::follow("a".repeat(32).as_str(), &lines[0].task, None, now);
    let (ticket, request) = run.tick(now).unwrap();
    assert!(matches!(request, Request::Poll { .. }));
    run.outcome(
        ticket,
        Ok(Answer::Lines {
            lines: lines.to_vec(),
            state,
        }),
        now,
    );
    run
}

fn words(node: &Node<()>, out: &mut Vec<String>) {
    match &node.element {
        Element::Text { value, .. } => out.push(value.clone()),
        Element::Button { label, .. } => out.push(label.clone()),
        Element::Working { label } => out.push(label.clone()),
        Element::Tool {
            name,
            detail,
            children,
            ..
        } => {
            out.push(format!("{name} {detail}"));
            for child in children {
                words(child, out);
            }
        }
        Element::Markdown { blocks } => out.push(markdown::plain(blocks)),
        Element::Stack { children, .. } | Element::Message { children, .. } => {
            for child in children {
                words(child, out);
            }
        }
        _ => {}
    }
}

fn text_of(rows: &[Node<()>]) -> String {
    let mut out = vec![];
    for row in rows {
        words(row, &mut out);
    }
    out.join("\n")
}

#[test]
fn every_event_type_the_scripted_provider_emits_draws_a_row() {
    let mut seen = std::collections::BTreeSet::new();
    let all: Vec<Vec<Line>> = tasks(QUESTION_THEN_RESULT)
        .into_iter()
        .chain(tasks(OTHER_ENDINGS))
        .collect();
    for lines in &all {
        for line in lines {
            seen.insert(line.event.name());
        }
    }
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        [
            "approval",
            "coder_started",
            "failure",
            "output",
            "progress",
            "provider_switched",
            "question",
            "result",
            "status",
            "step",
            "stopped"
        ],
        "the fixtures cover every event"
    );

    let whole = &all[0];
    let mut run = fed(whole, State::Ended);
    let text = text_of(&run.rows());
    for expected in [
        "Codex is working.",
        // A switch says only who runs now (#10120).
        "Claude Code is working.",
        "Thinking Write the output.",
        // A command is one line, as Grok Build draws it (#10117); its
        // exit shows only when it failed.
        "Run printf 'import unittest\\n' > test_slugs.py",
        "Coder asks",
        "Should the test cover empty input too?",
        "Coder continued on Claude Code · claude-opus-5-5 (turn 2)",
        "Yes, cover it.",
        "Coder finished",
        "I added test_slugs.py.",
        "1 file changed · +2 −0",
        "added test_slugs.py +2 −0",
        // The fixture's seconds are its recording's own.
        "Worked for ",
        "s · ran a command · thought 2 times",
    ] {
        assert!(
            text.contains(expected),
            "{expected:?} missing from:\n{text}"
        );
    }
    // The fixture's provider refused for a usage limit; no row says so.
    assert!(!text.to_lowercase().contains("limit"), "{text}");
    // The carried conversation and a reply the ending repeats show once.
    assert_eq!(
        text.matches("Should the test cover empty input too?")
            .count(),
        1
    );
    assert_eq!(text.matches("I added test_slugs.py.").count(), 1);
    assert_eq!(run.mode(), Mode::Send);

    let mut approval = fed(&all[1], State::Waiting);
    let rows = approval.rows();
    let text = text_of(&rows);
    assert!(
        text.contains("Coder asks to go ahead\nMay I delete slugs.py?"),
        "{text}"
    );
    assert_eq!(
        approval.actions.get("coder-approve"),
        Some(&Action::Approve)
    );
    assert_eq!(approval.mode(), Mode::Answer);

    let text = text_of(&fed(&all[2], State::Ended).rows());
    assert!(
        text.contains("Coder didn't finish\nNo coding agent is available right now"),
        "{text}"
    );
    assert!(!text.to_lowercase().contains("limit"), "{text}");
    let text = text_of(&fed(&all[3], State::Ended).rows());
    assert!(
        text.contains("Coder stopped before the turn started."),
        "{text}"
    );
}

#[test]
fn a_run_that_landed_or_opened_a_pull_request_shows_where_its_change_went() {
    let issue = |outcome: &str| coder_events::IssueLink {
        repository: "acme/app".into(),
        number: 42,
        url: "https://github.com/acme/app/issues/42".into(),
        title: "Fix the docs".into(),
        outcome: outcome.into(),
        commits: vec!["0123456789abcdef".into()],
        pull_request: None,
        closed: outcome == "landed",
        not_landed: None,
    };
    // Every ending in the fixtures names `link` as its issue.
    let with_issue = |lines: &[Line], link: &coder_events::IssueLink| {
        let mut lines = lines.to_vec();
        for line in &mut lines {
            match &mut line.event {
                CoderEvent::Result(result) => result.issue = Some(link.clone()),
                CoderEvent::Failure(failure) => failure.issue = Some(link.clone()),
                _ => {}
            }
        }
        text_of(&fed(&lines, State::Ended).rows())
    };
    let finished = &tasks(QUESTION_THEN_RESULT)[0];

    let text = with_issue(finished, &issue("landed"));
    assert!(
        text.contains(
            "Landed\n0123456789 on the default branch · closed #42 · \
             https://github.com/acme/app/commit/0123456789abcdef"
        ),
        "{text}"
    );

    let mut opened = issue("pull_request");
    opened.pull_request = Some("https://github.com/acme/app/pull/7".into());
    let text = with_issue(finished, &opened);
    assert!(
        text.contains("Pull request open\nFor #42 · https://github.com/acme/app/pull/7"),
        "{text}"
    );

    // A landing that failed ends in the failure card, with its reason.
    let failed = tasks(OTHER_ENDINGS)
        .into_iter()
        .find(|lines| {
            lines
                .iter()
                .any(|line| matches!(line.event, CoderEvent::Failure(_)))
        })
        .unwrap();
    let mut conflict = issue("failed");
    conflict.not_landed = Some("conflict".into());
    let text = with_issue(&failed, &conflict);
    assert!(
        text.contains("Conflict\n#42 stays open; nothing landed"),
        "{text}"
    );
    conflict.not_landed = Some("push_refused".into());
    let text = with_issue(&failed, &conflict);
    assert!(
        text.contains("Push refused\n#42 stays open; nothing landed"),
        "{text}"
    );
}

#[test]
fn a_running_turn_shows_its_progress_and_open_command() {
    let whole = &tasks(QUESTION_THEN_RESULT)[0];
    // Up to the first command, before its output.
    let cut = whole
        .iter()
        .position(|line| matches!(&line.event, CoderEvent::Step(s) if s.kind == StepKind::Command))
        .unwrap();
    let mut run = fed(&whole[..=cut], State::Running);
    let rows = run.rows();
    let running = rows.iter().any(|row| {
        matches!(
            &row.element,
            Element::Tool {
                state: ToolState::Running,
                ..
            }
        )
    });
    assert!(running, "the command runs until its output comes");
    let text = text_of(&rows);
    // No budget on the running line (#10103): the step and the time, and
    // Jev's estimate of how much is done once it has one.
    assert!(text.contains("Working · step 1 · 0s"), "{text}");
    assert!(!text.contains(" of 24"), "{text}");
    assert!(text.contains("Stop Coder"));
    assert_eq!(run.mode(), Mode::Queue);
    assert_eq!(run.steer_choice(), Some(Choice::StopAndSend));
}

#[test]
fn replayed_lines_are_kept_once_and_polls_follow_the_state() {
    let whole = &tasks(QUESTION_THEN_RESULT)[0];
    let now = Instant::now();
    let mut run = fed(&whole[..5], State::Running);
    // A new follower replays from the first event.
    let (ticket, _) = run.tick(now + Duration::from_secs(1)).unwrap();
    run.outcome(
        ticket,
        Ok(Answer::Lines {
            lines: whole[..8].to_vec(),
            state: State::Running,
        }),
        now,
    );
    let seqs: Vec<u64> = run.lines().map(|line| line.seq).collect();
    assert_eq!(seqs, (1..=8).collect::<Vec<_>>());
}

#[test]
fn stop_steer_queue_and_answers_go_to_the_runner() {
    let whole = &tasks(QUESTION_THEN_RESULT)[0];
    let task = whole[0].task.clone();
    let now = Instant::now();
    let mut run = fed(&whole[..8], State::Running);

    // Sending while Coder works queues the message for the next turn.
    assert!(run.action(Action::Send, "also cover spaces").is_none());
    assert_eq!(run.queued(), 1);
    // Stop and send: a stop, then the message once the turn ends.
    let (ticket, stop) = run.action(Action::Steer, "use pytest instead").unwrap();
    assert_eq!(stop, Request::Stop { task: task.clone() });
    assert!(run.busy());
    run.outcome(ticket, Ok(Answer::Stopping), now);
    let (ticket, _) = run.tick(now + Duration::from_secs(1)).unwrap();
    run.outcome(
        ticket,
        Ok(Answer::Lines {
            lines: whole[..12].to_vec(),
            state: State::Waiting,
        }),
        now,
    );
    let (ticket, next) = run.tick(now + Duration::from_secs(2)).unwrap();
    assert_eq!(
        next,
        Request::Continue {
            task: task.clone(),
            text: "use pytest instead".into()
        }
    );
    assert!(run.outcome(ticket, Ok(Answer::Continued), now));
    assert_eq!(run.state(), State::Running);

    // The turn ends; the queued message starts the next.
    let (ticket, _) = run.tick(now + Duration::from_secs(3)).unwrap();
    run.outcome(
        ticket,
        Ok(Answer::Lines {
            lines: whole.to_vec(),
            state: State::Ended,
        }),
        now,
    );
    let (_, next) = run.tick(now + Duration::from_secs(4)).unwrap();
    assert_eq!(
        next,
        Request::Continue {
            task,
            text: "also cover spaces".into()
        }
    );
    assert_eq!(run.queued(), 0);
}

/// The decision panel pages a question's numbered options, and the run
/// answers with every page, in order (#10469).
#[test]
fn a_paged_question_is_answered_through_the_decision_panel() {
    use crate::decision::Control;
    let whole = &tasks(QUESTION_THEN_RESULT)[0];
    let asked = whole
        .iter()
        .position(|line| line.event.name() == "question")
        .unwrap();
    let mut lines = whole[..=asked].to_vec();
    let CoderEvent::Question(question) = &mut lines[asked].event else {
        panic!("question")
    };
    question.text =
        "Which test first?\n1. empty input\n2. spaces\n\nShould I use pytest?\n1. Yes\n2. No"
            .into();
    let task = lines[0].task.clone();
    let mut run = fed(&lines, State::Waiting);
    let text = text_of(&run.rows());
    assert!(
        text.contains("Coder asks · 1 of 2\nWhich test first?\nempty input\nspaces"),
        "{text}"
    );
    assert_eq!(text.matches("Which test first?").count(), 1, "{text}");
    assert_eq!(
        run.actions.get("coder-option-2"),
        Some(&Action::Decide(Control::Pick(1)))
    );
    // A pick moves to the next page and sends nothing yet.
    assert!(run.action(Action::Decide(Control::Pick(1)), "").is_none());
    assert_eq!(run.decision().map(crate::decision::Flow::page), Some(1));
    let text = text_of(&run.rows());
    assert!(
        text.contains("Coder asks · 2 of 2\nShould I use pytest?"),
        "{text}"
    );
    assert_eq!(
        run.actions.get("coder-decision-back"),
        Some(&Action::Decide(Control::Back))
    );
    // Typed text answers the last page, and the whole answer goes.
    let (_, request) = run.action(Action::Send, "Yes, with fixtures").unwrap();
    assert_eq!(
        request,
        Request::Continue {
            task,
            text: "1. spaces\n2. Yes, with fixtures".into()
        }
    );
}

/// A start carries the engine the person asked for, on its first try and
/// on the retry after a folder is picked (#10076).
#[test]
fn a_start_carries_the_engine_the_person_asked_for() {
    use nostr::cj_conversation::Engine;
    let now = Instant::now();
    let mut run = Run::start("c".repeat(32).as_str(), "slugs", "add a test", vec![], now)
        .requesting(Some(Engine::ClaudeCode));
    let (ticket, start) = run.tick(now).unwrap();
    assert!(matches!(
        &start,
        Request::Start {
            engine: Some(Engine::ClaudeCode),
            ..
        }
    ));
    run.outcome(
        ticket,
        Ok(Answer::NeedsProject {
            why: "Pick the project folder for Coder.".into(),
        }),
        now,
    );
    let (ticket, _) = run.action(Action::ChooseFolder, "").unwrap();
    run.outcome(ticket, Ok(Answer::Folder(Some("/w/slugs".into()))), now);
    let (_, again) = run.tick(now).unwrap();
    assert!(matches!(
        &again,
        Request::Start {
            engine: Some(Engine::ClaudeCode),
            ..
        }
    ));
    let mut plain = Run::start("c".repeat(32).as_str(), "slugs", "add a test", vec![], now);
    let (_, start) = plain.tick(now).unwrap();
    assert!(matches!(&start, Request::Start { engine: None, .. }));
}

#[test]
fn a_start_without_a_checkout_asks_for_a_folder_and_starts_there() {
    let now = Instant::now();
    let mut run = Run::start("c".repeat(32).as_str(), "slugs", "add a test", vec![], now);
    let (ticket, start) = run.tick(now).unwrap();
    assert!(matches!(&start, Request::Start { dirs, .. } if dirs.is_empty()));
    run.outcome(
        ticket,
        Ok(Answer::NeedsProject {
            why: "Pick the project folder for Coder.".into(),
        }),
        now,
    );
    let rows = run.rows();
    assert!(text_of(&rows).contains("Coder needs a project\nPick the project folder for Coder."));
    let (ticket, choose) = run.action(Action::ChooseFolder, "").unwrap();
    assert_eq!(choose, Request::Choose);
    run.outcome(ticket, Ok(Answer::Folder(Some("/w/slugs".into()))), now);
    let (ticket, start) = run.tick(now).unwrap();
    assert!(matches!(&start, Request::Start { dirs, .. } if dirs == &["/w/slugs"]));
    run.outcome(
        ticket,
        Ok(Answer::Started {
            task: "t".repeat(64),
            project: "slugs".into(),
            checkout: "/w/slugs".into(),
        }),
        now,
    );
    assert_eq!(run.take_bind(), Some(("t".repeat(64), "slugs".into())));
    assert!(run.take_bind().is_none());
    assert!(matches!(run.tick(now), Some((_, Request::Poll { .. }))));
}

/// The handoff prompt that starts a run is the person's own message,
/// which the chat shows just above the run: the run draws no second
/// bubble for it, "Continued from the OpenAgents app" or otherwise; a
/// message the person sends Coder later still shows (#10076).
#[test]
fn the_handoff_prompt_is_not_shown_again_under_the_chat() {
    use openagents_chat::coder_events::{CoderEvent, Line, Step, StepKind};
    let task = "a".repeat(64);
    let prompt = crate::basic_chats::handoff(
        "do a test delegation to claude",
        &[openagents_chat::basic_coder::Turn::user(
            "do a test delegation to claude",
        )],
        16 * 1024,
    );
    let step = |seq: u64, turn: usize, text: &str| Line {
        seq,
        task: task.clone(),
        thread: None,
        event: CoderEvent::Step(Step {
            turn,
            step_id: 1,
            kind: StepKind::Message,
            source: "user".into(),
            text: text.into(),
            call: None,
            plan: None,
        }),
    };
    let mut run = fed(
        &[step(1, 1, &prompt), step(2, 2, "also cover empty input")],
        State::Ended,
    );
    let text = text_of(&run.rows());
    assert!(!text.contains("Continued from"), "{text}");
    assert!(!text.contains("do a test delegation to claude"), "{text}");
    assert!(text.contains("also cover empty input"), "{text}");
}

/// A start card is one line, "Codex is working.", with no worktree, no
/// fallbacks, and no "signed in and has capacity"; why it runs shows only
/// when another engine runs than the one asked for (#10115).
#[test]
fn the_start_card_is_one_line_unless_another_engine_runs() {
    let mut whole = tasks(QUESTION_THEN_RESULT)[0].clone();
    let mut run = fed(&whole[..1], State::Running);
    let text = text_of(&run.rows());
    assert!(text.starts_with("Codex is working."), "{text}");
    for noise in [
        "signed in",
        "Falls back",
        "worktree",
        "/",
        "Coder started on",
    ] {
        assert!(!text.contains(noise), "{noise}: {text}");
    }
    let CoderEvent::CoderStarted(started) = &mut whole[0].event else {
        unreachable!()
    };
    started.reason =
        "You asked for Claude Code; it is not signed in here, so Codex is running.".into();
    started.runner = Some(openagents_chat::coder_events::Runner::Runs {
        provider: "codex".into(),
        model: "gpt-6-luna".into(),
        passed: vec![openagents_chat::coder_events::Passed {
            provider: "claude".into(),
            why: openagents_chat::coder_events::PassedOver::NotSignedIn,
        }],
        requested: Some("claude".into()),
    });
    let mut other = fed(&whole[..1], State::Running);
    let text = text_of(&other.rows());
    assert!(
        text.starts_with(
            "Codex is working.\nYou asked for Claude Code; it is not signed in here, so Codex is running."
        ),
        "{text}"
    );
    // Before the start, the row names who is starting.
    let mut starting = Run::start("a".repeat(32).as_str(), "t", "p", vec![], Instant::now())
        .requesting(Some(nostr::cj_conversation::Engine::GrokBuild));
    assert!(text_of(&starting.rows()).contains("Starting Grok Build…"));
}

/// The person's message comes before the "Coder continued" card that it
/// started, never after it (#10094).
#[test]
fn a_later_turns_message_comes_before_its_card() {
    let whole = &tasks(QUESTION_THEN_RESULT)[0];
    let mut run = fed(whole, State::Ended);
    let text = text_of(&run.rows());
    let message = text.find("Yes, cover it.").unwrap();
    let card = text
        .find("Coder continued on Claude Code · claude-opus-5-5 (turn 2)")
        .unwrap();
    assert!(message < card, "{text}");
}

/// Once the run's turn has ended, a message goes to OpenAgents, which
/// answers it or hands it to Coder as the next turn; while Coder works it
/// still waits for the next turn (#10094).
#[test]
fn a_finished_run_routes_followups_and_continues_on_a_dispatch() {
    let whole = &tasks(QUESTION_THEN_RESULT)[0];
    let task = whole[0].task.clone();
    let mut working = fed(&whole[..8], State::Running);
    assert!(!working.routes_followups());
    assert_eq!(
        working.placeholder(),
        "Queue a message for Coder's next turn…"
    );
    assert!(working.result().is_none());
    assert!(!working.continue_with("now add a test"));

    let mut done = fed(whole, State::Ended);
    assert!(done.routes_followups());
    assert_eq!(done.placeholder(), "Ask OpenAgents anything");
    let result = done.result().unwrap();
    assert_eq!(result.ending, openagents_chat::router::RunEnding::Finished);
    assert_eq!(result.turn, 2);
    assert_eq!(result.engine.as_deref(), Some("claude"));
    assert!(result.summary.contains("I added test_slugs.py."));
    assert!(done.continue_with("now add a test"));
    // One request at a time.
    assert!(!done.continue_with("and another"));
    let (_, request) = done.tick(Instant::now()).unwrap();
    assert_eq!(
        request,
        Request::Continue {
            task,
            text: "now add a test".into()
        }
    );
}

/// A turn the chat's router handed to Coder follows the chat's reply that
/// handed it, and its message is the chat's own row, not repeated; a turn
/// with no such reply follows the turn before it (#10094).
#[test]
fn turns_follow_the_chat_reply_that_started_them() {
    let whole = &tasks(QUESTION_THEN_RESULT)[0];
    let mut run = fed(whole, State::Ended);
    // The chat holds "add a unit test for slugify" at 0, its reply at 1,
    // and "Yes, cover it." at 4, its reply at 5.
    let rows = run.rows_anchored(&|text| match text {
        "add a unit test for slugify" => Some(1),
        "Yes, cover it." => Some(5),
        _ => None,
    });
    let first = rows
        .iter()
        .position(|(_, row)| row.key == "coder-1")
        .unwrap();
    assert_eq!(rows[first].0, Some(1));
    let second: Vec<&(Option<usize>, Node<()>)> =
        rows.iter().filter(|(at, _)| *at == Some(5)).collect();
    let text = text_of(
        &second
            .iter()
            .map(|(_, row)| row.clone())
            .collect::<Vec<_>>(),
    );
    assert!(text.starts_with("Coder continued on Claude Code"), "{text}");
    assert!(!text.contains("Yes, cover it."), "{text}");
    assert!(text.contains("Coder finished"), "{text}");
    // With no anchors the rows are the plain run's.
    let plain = run.rows();
    assert_eq!(
        run.rows_anchored(&|_| None)
            .into_iter()
            .map(|(_, row)| row)
            .collect::<Vec<_>>(),
        plain
    );
}

/// A real Grok Build run (#10117): its looking calls show as one row
/// labelled by verb, which a click opens to each call and what it
/// returned; its command is one row of its own. The desktop and the phone
/// draw these same rows.
#[test]
fn tool_calls_show_grouped_and_open_to_each_call() {
    let lines = tasks(include_str!(
        "../../../openagents-chat/fixtures/coder-events/tools-grok.ndjson"
    ))
    .remove(0);
    let mut run = fed(&lines, State::Ended);
    let rows = run.rows();
    let tools: Vec<(&str, &str, usize)> = rows
        .iter()
        .filter_map(|row| match &row.element {
            Element::Tool {
                name,
                detail,
                children,
                ..
            } => Some((name.as_str(), detail.as_str(), children.len())),
            _ => None,
        })
        .collect();
    assert_eq!(
        tools,
        [
            ("Thinking", "Continuing the OpenAgents conversation.", 1),
            ("Read 3 files, Listed 1 dir, Searched 2 patterns", "", 1),
            ("Run", "Build crate and show git history", 1),
            (
                "Thinking",
                "I will summarize the crate's purpose in one sentence.",
                1
            ),
        ]
    );
    let text = text_of(&rows);
    for inside in [
        "Read lib.rs\n  1→pub fn add",
        "List src",
        "Search \"TODO\"\n  found 2 matches",
        "$ cargo build && echo \"---GIT---\" && git log --oneline",
    ] {
        assert!(text.contains(inside), "{inside:?} missing from:\n{text}");
    }
    // The group's key is its first call's event, so an open group stays
    // open as the run goes on.
    let group = rows
        .iter()
        .find(
            |row| matches!(&row.element, Element::Tool { name, .. } if name.starts_with("Read 3")),
        )
        .unwrap();
    assert_eq!(group.key, format!("coder-{}", lines[4].seq));
}

/// A running task that records a plan shows the plan panel under its
/// transcript, not a "plan updated" line in it; the panel's controls are
/// the run's actions (#10471).
#[test]
fn a_running_task_that_records_a_plan_shows_the_plan_panel() {
    use openagents_chat::coder_events::Mapper;
    use serde_json::json;
    let task = "b".repeat(64);
    let mut mapper = Mapper::new(1, None);
    let mut lines: Vec<Line> = vec![];
    for step in [
        json!({"step_id": 1, "source": "user", "message": "fix the parser"}),
        json!({"step_id": 2, "source": "system", "message": "Devin's plan.",
            "extra": {"devin_plan": [
                {"content": "Read the parser", "status": "completed"},
                {"content": "Fix the bug", "status": "in_progress"},
                {"content": "Add a test", "status": "pending"}]}}),
    ] {
        for event in mapper.step(&step) {
            lines.push(Line {
                seq: lines.len() as u64 + 1,
                task: task.clone(),
                thread: None,
                event,
            });
        }
    }
    let mut run = fed(&lines, State::Running);
    let text = text_of(&run.rows());
    assert!(text.contains("Plan · 1/3 · Fix the bug"), "{text}");
    assert!(text.contains("✓ Read the parser"), "{text}");
    assert!(text.contains("◐ Fix the bug"), "{text}");
    assert!(text.contains("○ Add a test"), "{text}");
    assert!(!text.contains("Updated the plan"), "{text}");
    assert!(!text.contains("Dismiss"), "a live plan stays: {text}");
    // The header closes the list.
    let toggle = run.actions.get("coder-plan-toggle").cloned().unwrap();
    assert_eq!(toggle, Action::Plan(crate::plan_panel::Control::Toggle));
    assert!(toggle.late());
    let revision = run.revision;
    assert!(run.action(toggle, "").is_none());
    assert!(run.revision > revision);
    let text = text_of(&run.rows());
    assert!(text.contains("Plan · 1/3 · Fix the bug"), "{text}");
    assert!(!text.contains("○ Add a test"), "{text}");
    // A run with no plan draws no panel.
    let mut plain = fed(&lines[..1], State::Running);
    assert!(plain.plan_items().is_none());
    assert!(!text_of(&plain.rows()).contains("Plan ·"));
}
