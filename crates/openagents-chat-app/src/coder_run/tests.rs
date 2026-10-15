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
            configuration: None,
            native: None,
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
        "Coder continued on Claude Code (turn 2)",
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
            configuration: None,
            native: None,
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

    // Ctrl+Enter while Coder works queues the message for later (#11685).
    assert!(run.action(Action::Queue, "also cover spaces").is_none());
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
            configuration: None,
            native: None,
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
            configuration: None,
            native: None,
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
        "logged in",
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
        "You asked for Claude Code; it is not logged in here, so Codex is running.".into();
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
            "Codex is working.\nYou asked for Claude Code; it is not logged in here, so Codex is running."
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
        .find("Coder continued on Claude Code (turn 2)")
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
    // The send control sends now while Coder works (#11685).
    assert_eq!(working.placeholder(), "Message Coder…");
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

/// #11685: Enter sends now while Coder works (the turn stops and the
/// message continues it); Ctrl+Enter queues it for after the turn.
#[test]
fn enter_sends_now_and_ctrl_enter_queues_while_coder_works() {
    let whole = &tasks(QUESTION_THEN_RESULT)[0];
    let task = whole[0].task.clone();
    let mut run = fed(&whole[..8], State::Running);
    let (_, stop) = run.action(Action::Send, "use pytest instead").unwrap();
    assert_eq!(stop, Request::Stop { task });
    assert_eq!(run.queued(), 0);
    let mut run = fed(&whole[..8], State::Running);
    assert!(run.action(Action::Queue, "later please").is_none());
    assert_eq!(run.queued(), 1);
    assert!(!run.busy());
}

fn described_lines(controls: route_contract::session::Controls) -> Vec<Line> {
    use route_contract::session::{Backend, Descriptor, NativeSupport, SCHEMA, Selection};
    let mut lines = tasks(QUESTION_THEN_RESULT).remove(0);
    let task = lines[0].task.clone();
    let CoderEvent::CoderStarted(started) = &mut lines[0].event else {
        panic!("start")
    };
    let resolved = Backend {
        selection: Selection {
            provider: started.provider.clone(),
            model: started.model.clone(),
            effort: Some("medium".into()),
        },
        adapter: "coder-loop".into(),
        native: NativeSupport {
            images: true,
            ..NativeSupport::default()
        },
        ..Backend::default()
    };
    started.session = Some(Descriptor {
        schema: SCHEMA.into(),
        task,
        turn: started.turn,
        owner: "local".into(),
        requested: Some(Selection {
            provider: "devin".into(),
            ..Selection::default()
        }),
        resolved: resolved.clone(),
        alternatives: vec![resolved],
        controls,
        ..Descriptor::default()
    });
    lines.truncate(1);
    lines
}

#[test]
fn session_choices_reach_the_shared_desktop_projection_and_survive_row_eviction() {
    let lines = described_lines(route_contract::session::Controls {
        stop: true,
        ..Default::default()
    });
    let mut run = fed(&lines, State::Running);
    let session = run.session().unwrap().clone();
    let label = run.session_label().unwrap();
    assert!(label.contains("Requested Devin"), "{label}");
    assert!(label.contains(&session.resolved.selection.model), "{label}");
    assert!(text_of(&run.rows()).contains(&label));
    assert!(run.can_stop());
    assert!(!run.can_queue());
    assert!(!session.resolved.native.resume);
    run.lines.clear();
    assert_eq!(run.session(), Some(&session));
    assert!(text_of(&run.rows()).contains(&label));
}

#[test]
fn unsupported_controls_cannot_dispatch_or_consume_queued_text() {
    let lines = described_lines(Default::default());
    let mut run = fed(&lines, State::Running);
    assert!(!run.can_stop());
    assert!(!run.can_queue());
    assert!(!run.can_send());
    assert!(run.steer_choice().is_none());
    assert!(run.action(Action::Stop, "").is_none());
    assert!(run.action(Action::Send, "keep this draft").is_none());
    assert!(run.action(Action::Queue, "keep this draft").is_none());
    assert!(run.action(Action::Steer, "keep this draft").is_none());
    assert_eq!(run.queued(), 0);
    run.queue.push_back("already queued".into());
    assert!(run.action(Action::SendQueuedNow(0), "").is_none());
    assert_eq!(run.queued(), 1);
    assert!(!text_of(&run.rows()).contains("Stop Coder"));
    run.state = State::Ended;
    assert!(!run.continue_with("continue"));
}

#[test]
fn unsupported_pending_controls_keep_polling_and_preserve_the_message() {
    let lines = described_lines(Default::default());
    let mut run = fed(&lines, State::Running);
    let task = run.task.clone().unwrap();
    run.due = Some(Request::Continue {
        task,
        text: "keep this message".into(),
    });
    let (_, request) = run.tick(Instant::now() + Duration::from_secs(60)).unwrap();
    assert!(matches!(request, Request::Poll { .. }));
    assert!(matches!(
        run.due,
        Some(Request::Continue { ref text, .. }) if text == "keep this message"
    ));
}

#[test]
fn requested_model_is_visible_when_the_provider_stays_the_same() {
    let mut lines = described_lines(Default::default());
    let CoderEvent::CoderStarted(started) = &mut lines[0].event else {
        panic!("start")
    };
    let session = started.session.as_mut().unwrap();
    session.requested = Some(route_contract::session::Selection {
        provider: started.provider.clone(),
        model: "requested-model".into(),
        effort: None,
    });
    let label = fed(&lines, State::Running).session_label().unwrap();
    assert!(label.contains("requested-model"), "{label}");
    assert!(label.contains("Using"), "{label}");
}

#[test]
fn future_or_wrong_task_descriptions_cannot_enable_controls() {
    for wrong_task in [false, true] {
        let mut lines = described_lines(route_contract::session::Controls {
            stop: true,
            queue: true,
            ..Default::default()
        });
        let CoderEvent::CoderStarted(started) = &mut lines[0].event else {
            panic!("start")
        };
        let session = started.session.as_mut().unwrap();
        if wrong_task {
            session.task = "another-task".into();
        } else {
            session.schema = "future-version".into();
        }
        let run = fed(&lines, State::Running);
        assert!(run.session().is_none());
        assert!(!run.can_stop());
        assert!(!run.can_queue());
    }
    let lines = tasks(QUESTION_THEN_RESULT).remove(0);
    let run = fed(&lines[..1], State::Running);
    assert!(run.session().is_none());
    // The legacy task protocol still supplies its existing Stop operation.
    assert!(run.can_stop());
}

#[test]
fn provider_switch_changes_the_label_without_granting_native_operations() {
    let mut lines = described_lines(route_contract::session::Controls {
        stop: true,
        ..Default::default()
    });
    let mut switched = lines[0].clone();
    switched.seq += 1;
    switched.event = CoderEvent::ProviderSwitched(coder_events::Switched {
        turn: turn_of(&lines[0]),
        step_id: 1,
        from: "old:model".into(),
        to: Some("future:model:variant".into()),
        reason: "test".into(),
        resets_at: None,
    });
    lines.push(switched);
    let run = fed(&lines, State::Running);
    assert!(run.session_label().unwrap().contains("model:variant"));
    let session = run.session().unwrap();
    assert_eq!(session.resolved.native, Default::default());
    assert!(session.resolved.hooks.is_empty());
    assert_eq!(session.requested.as_ref().unwrap().provider, "devin");
}

fn observation_lines() -> Vec<Line> {
    use route_contract::observation::{self as wire, Body, Channel, TextMode};
    let mut lines = described_lines(route_contract::session::Controls {
        stop: true,
        ..Default::default()
    });
    let bodies = [
        (
            None,
            Body::Started {
                provider: "claude".into(),
                model: "observed-model".into(),
            },
        ),
        (
            Some("call"),
            Body::ToolStarted {
                name: "Bash".into(),
                input: r#"{"command":"cargo test"}"#.into(),
            },
        ),
        (
            Some("call"),
            Body::ToolResult {
                text: "passed".into(),
                failed: false,
                truncated: false,
            },
        ),
        (
            Some("reply"),
            Body::Text {
                channel: Channel::Text,
                mode: TextMode::Snapshot,
                text: "Hello".into(),
                complete: false,
                truncated: false,
            },
        ),
        (
            Some("reply"),
            Body::Text {
                channel: Channel::Text,
                mode: TextMode::Delta,
                text: " world".into(),
                complete: false,
                truncated: false,
            },
        ),
        (
            Some("reply"),
            Body::Text {
                channel: Channel::Text,
                mode: TextMode::Snapshot,
                text: "Hello world".into(),
                complete: true,
                truncated: false,
            },
        ),
        (
            None,
            Body::Plan {
                items: vec![wire::PlanItem {
                    text: "Verify".into(),
                    status: wire::PlanStatus::Completed,
                }],
            },
        ),
        (
            None,
            Body::Usage {
                usage: wire::Usage {
                    output: Some(0),
                    ..Default::default()
                },
            },
        ),
        (
            None,
            Body::Context {
                used: Some(120),
                capacity: Some(1000),
            },
        ),
        (
            None,
            Body::Policy {
                tool: "Read".into(),
                decision: "allowed".into(),
            },
        ),
        (
            None,
            Body::Ended {
                ending: wire::Ending::Completed,
                message: None,
            },
        ),
    ];
    let task = lines[0].task.clone();
    for (index, (item, body)) in bodies.into_iter().enumerate() {
        lines.push(Line {
            seq: index as u64 + 2,
            task: task.clone(),
            thread: lines[0].thread.clone(),
            event: CoderEvent::Observation {
                observation: wire::Observation {
                    schema: wire::SCHEMA.into(),
                    scope: wire::Scope {
                        host: LOCAL.into(),
                        task: task.clone(),
                        turn: 1,
                        generation: 1,
                    },
                    seq: index as u64 + 1,
                    native_session: Some("native-session".into()),
                    item: item.map(str::to_owned),
                    child: None,
                    body,
                },
            },
        });
    }
    lines
}

fn deliver(run: &mut Run, lines: &[Line], state: State) {
    let task = run.task.clone().unwrap();
    let (ticket, _) = run.request(Request::Poll { task }).unwrap();
    run.outcome(
        ticket,
        Ok(Answer::Lines {
            configuration: None,
            native: None,
            lines: lines.to_vec(),
            state,
        }),
        Instant::now(),
    );
}

#[test]
fn observations_draw_once_on_load_follow_and_reconnect_and_keep_session_state() {
    let lines: Vec<Line> =
        serde_json::from_value(serde_json::to_value(observation_lines()).unwrap()).unwrap();
    let mut whole = fed(&lines, State::Running);
    let mut followed = fed(&lines[..1], State::Running);
    for chunk in lines[1..].chunks(2) {
        deliver(&mut followed, chunk, State::Running);
    }
    assert_eq!(text_of(&whole.rows()), text_of(&followed.rows()));
    let text = text_of(&followed.rows());
    assert_eq!(text.matches("Hello world").count(), 1, "{text}");
    assert_eq!(followed.lines.iter().filter(|line| matches!(&line.event, CoderEvent::Step(step) if step.kind == StepKind::Command)).count(), 1);
    assert_eq!(
        followed
            .lines
            .iter()
            .filter(|line| matches!(line.event, CoderEvent::Output(_)))
            .count(),
        1
    );
    assert!(text.contains("120 of 1000 tokens"), "{text}");
    assert!(text.contains("0 output tokens"), "{text}");
    assert!(!text.contains("0 input tokens"), "{text}");
    assert!(text.contains("Read: allowed"), "{text}");
    assert_eq!(
        followed.state(),
        State::Running,
        "the provider cannot finish the host task"
    );
    assert_eq!(
        followed.session().unwrap().native_session.as_deref(),
        Some("native-session")
    );
    assert!(followed.session_label().unwrap().contains("observed-model"));
    let count = followed.lines.len();
    deliver(&mut followed, &lines, State::Running);
    assert_eq!(followed.lines.len(), count);
    assert_eq!(text_of(&followed.rows()), text);
    followed.lines.clear();
    deliver(&mut followed, &lines, State::Running);
    assert!(
        followed.lines.is_empty(),
        "row eviction cannot reset the delivery cursor"
    );
    assert_eq!(followed.plan_items().unwrap()[0].text, "Verify");
    assert!(text_of(&followed.rows()).contains("120 of 1000 tokens"));
}

#[test]
fn missing_and_foreign_observations_do_not_join_partial_text_or_change_another_chat() {
    let lines = observation_lines();
    let mut run = fed(&lines[..5], State::Running);
    // Leave out the reply delta. The full snapshot recovers the reply,
    // while the missing-history notice stays visible.
    deliver(&mut run, &lines[6..], State::Running);
    let text = text_of(&run.rows());
    assert_eq!(text.matches("Hello world").count(), 1, "{text}");
    assert!(
        text.contains("Some earlier output is unavailable."),
        "{text}"
    );
    let mut other = lines.clone();
    for line in &mut other {
        line.task = "another-task".into();
    }
    let mut other_chat = Run::follow("other-chat", "another-task", None, Instant::now());
    deliver(&mut other_chat, &other, State::Running);
    assert!(!text_of(&other_chat.rows()).contains("Hello world"));
    assert!(other_chat.session().is_none());
    // A late outcome with no matching pending request is ignored.
    assert!(!other_chat.outcome(
        u64::MAX,
        Ok(Answer::Lines {
            configuration: None,
            native: None,
            lines,
            state: State::Ended
        }),
        Instant::now()
    ));
    assert_eq!(other_chat.state(), State::Running);
}

#[test]
fn duplicate_observations_in_a_batch_and_repeated_outcomes_do_not_repeat_output() {
    let lines = observation_lines();
    let mut run = fed(&lines[..1], State::Running);
    let task = run.task.clone().unwrap();
    let (new, _) = run.request(Request::Poll { task }).unwrap();
    let mut duplicate = Vec::new();
    for line in &lines[1..] {
        duplicate.push(line.clone());
        duplicate.push(line.clone());
    }
    run.outcome(
        new,
        Ok(Answer::Lines {
            configuration: None,
            native: None,
            lines: duplicate,
            state: State::Ended,
        }),
        Instant::now(),
    );
    let text = text_of(&run.rows());
    assert_eq!(text.matches("Hello world").count(), 1, "{text}");
    run.outcome(
        new,
        Ok(Answer::Lines {
            configuration: None,
            native: None,
            lines: Vec::new(),
            state: State::Running,
        }),
        Instant::now(),
    );
    assert_eq!(run.state(), State::Ended);
    assert_eq!(text_of(&run.rows()), text);
}

#[test]
fn observations_without_a_legacy_start_record_still_bind_to_the_right_task() {
    let lines = observation_lines();
    let mut run = Run::follow("chat", &lines[0].task, None, Instant::now());
    deliver(&mut run, &lines[1..], State::Running);
    assert!(
        run.session().is_none(),
        "observations cannot invent capability declarations"
    );
    assert!(run.session_label().unwrap().contains("observed-model"));
    assert_eq!(text_of(&run.rows()).matches("Hello world").count(), 1);
}

#[test]
fn a_task_delivery_gap_stays_visible_even_when_provider_sequences_are_complete() {
    let mut lines = observation_lines();
    for line in &mut lines[2..] {
        line.seq += 1;
    }
    let mut run = fed(&lines, State::Running);
    let text = text_of(&run.rows());
    assert!(
        text.contains("Some earlier output is unavailable."),
        "{text}"
    );
    assert_eq!(text.matches("Hello world").count(), 1);
}

#[test]
fn the_host_result_replaces_the_streamed_answer_and_keeps_exact_review_data() {
    let lines = observation_lines();
    let mut run = fed(&lines, State::Running);
    let mut result = tasks(QUESTION_THEN_RESULT)
        .remove(0)
        .into_iter()
        .find(|line| matches!(line.event, CoderEvent::Result(_)))
        .unwrap();
    result.seq = lines.last().unwrap().seq + 1;
    result.task = lines[0].task.clone();
    let CoderEvent::Result(finished) = &mut result.event else {
        panic!("result")
    };
    finished.turn = 1;
    finished.summary = "Hello world".into();
    let files = finished.files_changed.clone();
    deliver(&mut run, &[result], State::Ended);
    let text = text_of(&run.rows());
    assert_eq!(text.matches("Hello world").count(), 1, "{text}");
    assert_eq!(run.state(), State::Ended);
    let finished = run
        .lines
        .iter()
        .find_map(|line| match &line.event {
            CoderEvent::Result(finished) => Some(finished),
            _ => None,
        })
        .unwrap();
    assert_eq!(finished.files_changed, files);
    let (_, request) = run.tick(Instant::now()).unwrap();
    assert!(matches!(request, Request::Review { .. }));
    assert!(run.reviewer().is_some());
}

#[test]
fn stale_coding_choice_preserves_queue_and_new_draft() {
    let now = Instant::now();
    let mut run = Run::follow("chat", "task", None, now);
    run.state = State::Ended;
    let mut offer = super::configuration::tests::offer();
    run.configuration.update(Some(offer.clone()));
    let mut targets = Vec::new();
    fn collect(
        node: &Node<super::configuration::Target>,
        out: &mut Vec<super::configuration::Target>,
    ) {
        match &node.element {
            Element::Button { intent, .. } => out.push(intent.clone()),
            Element::Stack { children, .. } => {
                for child in children {
                    collect(child, out)
                }
            }
            _ => {}
        }
    }
    run.configuration.open();
    let view = run
        .configuration
        .view("test", true, true, |target| target)
        .unwrap();
    collect(&view.node, &mut targets);
    let model = targets
        .into_iter()
        .find(|target| target.control == super::configuration::Control::Model(1))
        .unwrap();
    run.action(Action::Configure(model), "");
    run.queue.push_back("queued message".into());
    offer.models[1].account = Some("changed-login".into());
    offer.seal();
    run.configuration.update(Some(offer));
    assert!(run.action(Action::SendQueuedNow(0), "").is_none());
    assert_eq!(run.queue.front().unwrap(), "queued message");
    assert!(run.action(Action::Send, "new draft").is_none());
    assert!(!run.take_consumed_input());
    assert!(
        run.notice
            .as_ref()
            .unwrap()
            .contains("login or settings changed")
    );
}

#[test]
fn configured_continuation_uses_explicit_owner_request() {
    let now = Instant::now();
    let mut run = Run::follow("chat", "task", None, now);
    run.state = State::Ended;
    let offer = super::configuration::tests::offer();
    let request = route_contract::session::control::Request {
        schema: route_contract::session::control::SCHEMA.into(),
        offer: offer.id.clone(),
        adapter: "claude-sdk".into(),
        checkout: offer.checkout.clone(),
        target: offer.target.clone(),
        selection: offer.models[1].selection.clone(),
        mode: route_contract::session::control::Mode::New,
        resume: None,
    };
    run.configuration.adopt(offer, Some(&request));
    let (_, sent) = run.action(Action::Send, "next message").unwrap();
    assert!(
        matches!(sent,Request::ContinueConfigured{task,text,configuration} if task=="task"&&text=="next message"&&configuration==request)
    );
}
