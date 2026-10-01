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
            "step",
            "stopped"
        ],
        "the fixtures cover every event"
    );

    let whole = &all[0];
    let mut run = fed(whole, State::Ended);
    let text = text_of(&run.rows());
    for expected in [
        "Coder started on Codex · gpt-6-luna",
        "Codex is signed in and has capacity.",
        "Falls back to Claude Code (claude-opus-5-5)",
        "Switched from Codex (gpt-6-luna) to Claude Code (claude-opus-5-5): Codex refused for a usage limit until",
        "Thinking Write the output.",
        "Command printf 'import unittest\\n' > test_slugs.py · exit 0 in 0.0s",
        "Coder asks",
        "Should the test cover empty input too?",
        "Coder continued on Claude Code · claude-opus-5-5 (turn 2)",
        "Yes, cover it.",
        "Coder finished",
        "I added test_slugs.py.",
        "1 file changed · +2 −0",
        "added test_slugs.py +2 −0",
        "Worked for 0s · ran a command · thought 2 times",
    ] {
        assert!(
            text.contains(expected),
            "{expected:?} missing from:\n{text}"
        );
    }
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
    assert!(text.contains("no other provider has capacity"), "{text}");
    assert!(
        text.contains("Coder didn't finish\nNo admitted provider has capacity"),
        "{text}"
    );
    let text = text_of(&fed(&all[3], State::Ended).rows());
    assert!(
        text.contains("Coder stopped before the turn started."),
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
    assert!(text.contains("Coder is working · step 1 of 24"), "{text}");
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
