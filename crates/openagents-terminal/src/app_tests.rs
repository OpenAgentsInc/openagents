//! The screen's state without a terminal: keys and client events in,
//! actions and transcript text out.

use super::*;
use coder_terminal::Colors;
use crossterm::event::KeyEventState;
use openagents_chat::coder_events::{Asked, Progress, Started, Stopped};

fn app() -> App {
    App::new(
        Ladder::new(Colors::None),
        "a".repeat(32),
        true,
        Kind::InProcess,
        "demo".into(),
    )
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent {
        modifiers: KeyModifiers::CONTROL,
        ..key(KeyCode::Char(c))
    }
}

fn typed(app: &mut App, text: &str) -> Vec<Action> {
    for c in text.chars() {
        assert!(app.key(&key(KeyCode::Char(c)), 80).is_empty());
    }
    app.key(&key(KeyCode::Enter), 80)
}

fn shown(app: &mut App) -> String {
    app.transcript
        .rows(80, |_| true)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn line(seq: u64, event: CoderEvent) -> Event {
    Event::Line(Box::new(CoderLine {
        seq,
        task: "t1".into(),
        thread: Some("a".repeat(32)),
        event,
    }))
}

fn started() -> CoderEvent {
    CoderEvent::CoderStarted(Started {
        turn: 1,
        project: "demo".into(),
        checkout: "/tmp/demo".into(),
        worktree: "/tmp/w/t1".into(),
        base: "abc".into(),
        provider: "codex".into(),
        model: "gpt-5".into(),
        reason: "Codex is signed in and has capacity.".into(),
        fallbacks: Vec::new(),
        via: "local".into(),
        runner: None,
    })
}

fn reply(text: &str, meta: Meta) -> Event {
    let mut turn = Turn::user(text);
    turn.role = Role::Assistant;
    turn.meta = Some(meta);
    Event::Reply {
        thread: "a".repeat(32),
        reply: Box::new(turn),
        computer: true,
        running: false,
    }
}

#[test]
fn a_message_goes_to_the_chat_and_creates_the_thread_once() {
    let mut app = app();
    let actions = typed(&mut app, "what's your working dir");
    let [Action::Run(Op::Send { new, text, .. })] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    assert!(*new);
    assert_eq!(text, "what's your working dir");
    app.began(&actions[0].clone().into_op());
    assert_eq!(app.phase, Phase::Replying);
    app.event(Event::Partial {
        thread: app.thread.clone(),
        text: "Rain".into(),
        delta: None,
    });
    assert_eq!(app.partial, "Rain");
    app.event(reply("Rain on the roof.", Meta::default()));
    app.ended(None);
    assert!(app.partial.is_empty());
    let shown = shown(&mut app);
    assert!(shown.contains("what's your working dir"), "{shown}");
    assert!(shown.contains("Rain on the roof."), "{shown}");
    let again = typed(&mut app, "more");
    let [Action::Run(Op::Send { new, .. })] = again.as_slice() else {
        panic!("{again:?}");
    };
    assert!(!*new, "the second message goes to the same thread");
}

trait IntoOp {
    fn into_op(self) -> Op;
}

impl IntoOp for Action {
    fn into_op(self) -> Op {
        match self {
            Action::Run(op) => op,
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn esc_stops_the_reply_then_the_run_and_never_quits() {
    let mut app = app();
    app.phase = Phase::Replying;
    assert_eq!(app.key(&key(KeyCode::Esc), 80), vec![Action::Interrupt]);
    app.phase = Phase::Following;
    app.event(line(1, started()));
    assert_eq!(
        app.key(&key(KeyCode::Esc), 80),
        vec![Action::StopRun { task: "t1".into() }]
    );
    // Not following, but the run still runs: Esc is `chat stop`.
    app.phase = Phase::Idle;
    assert_eq!(
        app.key(&key(KeyCode::Esc), 80),
        vec![Action::Run(Op::Stop {
            thread: app.thread.clone()
        })]
    );
    app.event(line(
        2,
        CoderEvent::Stopped(Stopped {
            turn: 1,
            message: "Stopped by the person.".into(),
        }),
    ));
    assert!(!app.running);
    assert!(app.key(&key(KeyCode::Esc), 80).is_empty());
    assert!(shown(&mut app).contains("Stopped by the person."));
}

#[test]
fn ctrl_c_clears_the_draft_and_a_second_on_an_empty_draft_quits() {
    let mut app = app();
    for c in "draft".chars() {
        app.key(&key(KeyCode::Char(c)), 80);
    }
    assert!(app.key(&ctrl('c'), 80).is_empty());
    assert!(app.editor.is_empty());
    assert!(app.key(&ctrl('c'), 80).is_empty());
    assert_eq!(app.key(&ctrl('c'), 80), vec![Action::Quit]);
    // Any other key in between disarms it.
    let mut app = self::app();
    app.key(&ctrl('c'), 80);
    app.key(&key(KeyCode::Left), 80);
    assert!(app.key(&ctrl('c'), 80).is_empty());
}

#[test]
fn slash_commands_are_a_closed_list_and_other_text_is_a_message() {
    let mut app = app();
    assert_eq!(typed(&mut app, "/threads"), vec![Action::Threads]);
    assert_eq!(typed(&mut app, "/new"), vec![Action::New]);
    assert_eq!(app.key(&ctrl('n'), 80), vec![Action::New]);
    assert_eq!(typed(&mut app, "/connect"), vec![Action::Connect]);
    assert_eq!(typed(&mut app, "/quit"), vec![Action::Quit]);
    assert!(typed(&mut app, "/frobnicate").is_empty());
    let unknown = shown(&mut app);
    assert!(
        unknown.contains("/frobnicate is not a command"),
        "{unknown}"
    );
    // Nothing starts as it does: every command shows.
    assert!(unknown.contains("/threads"), "{unknown}");
    assert!(typed(&mut app, "/help").is_empty());
    let help = shown(&mut app);
    for slash in Slash::ALL {
        assert!(help.contains(&slash.usage()), "{help}");
    }
    let actions = typed(&mut app, "stop the build from failing");
    assert!(
        matches!(actions.as_slice(), [Action::Run(Op::Send { .. })]),
        "{actions:?}"
    );
}

#[test]
fn a_question_from_coder_is_answered_from_the_composer() {
    let mut app = app();
    app.fresh = false;
    app.event(line(1, started()));
    app.event(line(
        2,
        CoderEvent::Question(Asked {
            turn: 1,
            text: "Which branch?".into(),
            answer: Some("Type your answer and press Enter.".into()),
        }),
    ));
    assert!(app.asked);
    assert!(!app.running);
    assert_eq!(app.status(), "Coder asks · type your answer");
    let actions = typed(&mut app, "main");
    assert_eq!(
        actions,
        vec![Action::Run(Op::Answer {
            thread: app.thread.clone(),
            text: "main".into()
        })]
    );
}

#[test]
fn an_offer_waits_for_enter_on_an_empty_line_and_shows_no_followups() {
    let mut app = app();
    app.fresh = false;
    let meta = Meta {
        offers: vec![Offer::RunCoder],
        followups: vec![openagents_chat::router::Followup {
            answer: None,
            label: "What can Coder do?".into(),
        }],
        ..Meta::default()
    };
    app.event(reply("Coder can fix that.", meta));
    assert!(app.offer);
    let shown = shown(&mut app);
    assert!(shown.contains("Enter to run Coder."), "{shown}");
    // Suggested follow-ups are the apps' chips; the terminal shows none.
    assert!(
        !shown.contains("What can Coder do?") && !shown.contains("Alt+"),
        "{shown}"
    );
    let actions = typed(&mut app, "What can Coder do?");
    let [Action::Run(Op::Send { text, .. })] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    assert_eq!(text, "What can Coder do?");
    // A new message clears the offer.
    assert!(!app.offer);
    let mut app = self::app();
    app.fresh = false;
    app.event(reply(
        "Coder can fix that.",
        Meta {
            offers: vec![Offer::RunCoder],
            ..Meta::default()
        },
    ));
    assert_eq!(
        app.key(&key(KeyCode::Enter), 80),
        vec![Action::Run(Op::RunCoder {
            thread: app.thread.clone()
        })]
    );
}

#[test]
fn progress_is_one_live_row_and_a_replayed_event_shows_once() {
    let mut app = app();
    app.event(line(1, started()));
    for (seq, step) in [(2, 1), (3, 2), (4, 3)] {
        app.event(line(
            seq,
            CoderEvent::Progress(Progress {
                turn: 1,
                step,
                seconds: 9.0,
                done: None,
                complete: Some(0.4),
            }),
        ));
    }
    let before = shown(&mut app);
    assert!(
        !before.contains("step 3"),
        "progress is not a transcript row"
    );
    assert_eq!(
        app.progress,
        Some(RunRow::Progress {
            step: 3,
            percent: Some(40),
            seconds: 9
        })
    );
    app.phase = Phase::Following;
    assert_eq!(app.status(), "Coder · step 3 · ≈40% done · 9s · Esc stops");
    assert!(!app.status().contains(" of "));
    // Following again replays from the first event: nothing repeats.
    app.event(line(1, started()));
    assert_eq!(shown(&mut app), before);
}

#[test]
fn a_send_while_a_reply_streams_waits_and_keeps_the_draft() {
    let mut app = app();
    app.phase = Phase::Replying;
    assert!(typed(&mut app, "next").is_empty());
    assert_eq!(app.editor.text(), "next");
    assert!(shown(&mut app).contains("Wait for this reply"));
}

fn summary(id: &str, title: &str, updated: u64) -> openagents_chat::basic_chats::Summary {
    openagents_chat::basic_chats::Summary {
        id: id.repeat(32),
        title: title.into(),
        started: 1,
        updated,
        coder: None,
        archived: false,
        pinned: false,
        named: false,
    }
}

#[test]
fn the_thread_picker_opens_starts_archives_and_copies() {
    let mut app = app();
    app.overlay = Some(Overlay::Threads(Picker::new(
        vec![summary("1", "one", 3), summary("2", "two", 2)],
        "demo",
    )));
    assert!(app.key(&key(KeyCode::Down), 80).is_empty());
    assert_eq!(
        app.key(&ctrl('a'), 80),
        vec![Action::Archive("2".repeat(32))]
    );
    assert_eq!(
        app.key(&key(KeyCode::Char('y')), 80),
        vec![Action::Copy("2".repeat(32))]
    );
    assert_eq!(
        app.key(&key(KeyCode::Enter), 80),
        vec![Action::Open("2".repeat(32))]
    );
    assert!(app.overlay.is_none());
    app.overlay = Some(Overlay::Threads(Picker::new(
        vec![summary("1", "one", 1)],
        "demo",
    )));
    assert_eq!(app.key(&ctrl('n'), 80), vec![Action::New]);
    app.overlay = Some(Overlay::Threads(Picker::new(Vec::new(), "demo")));
    assert!(app.key(&key(KeyCode::Esc), 80).is_empty());
    assert!(app.overlay.is_none());
}

#[test]
fn typing_in_the_thread_picker_searches_it() {
    let mut app = app();
    app.overlay = Some(Overlay::Threads(Picker::new(
        vec![
            summary("1", "Fix the parser", 3),
            summary("2", "Lunch plans", 2),
            summary("3", "Parser docs", 1),
        ],
        "demo",
    )));
    for c in "PARSE".chars() {
        assert!(app.key(&key(KeyCode::Char(c)), 80).is_empty());
    }
    let Some(Overlay::Threads(picker)) = &app.overlay else {
        panic!("the picker closed");
    };
    assert_eq!(picker.query, "PARSE");
    let titles: Vec<String> = picker
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            crate::picker::Entry::Row(row) => Some(row.title.clone()),
            crate::picker::Entry::Header(_) => None,
        })
        .collect();
    assert_eq!(titles, ["Fix the parser", "Parser docs"]);
    // Down leaves the query for the list; Down again reaches the second.
    assert!(app.key(&key(KeyCode::Down), 80).is_empty());
    assert!(app.key(&key(KeyCode::Down), 80).is_empty());
    assert_eq!(
        app.key(&ctrl('a'), 80),
        vec![Action::Archive("3".repeat(32))]
    );
    // Esc clears the query, then closes.
    assert!(app.key(&key(KeyCode::Esc), 80).is_empty());
    assert!(app.overlay.is_some());
    for c in "lunch".chars() {
        app.key(&key(KeyCode::Char(c)), 80);
    }
    assert_eq!(
        app.key(&key(KeyCode::Enter), 80),
        vec![Action::Open("2".repeat(32))]
    );
}

#[test]
fn resume_opens_the_picker_or_the_thread_it_names() {
    let mut app = app();
    assert_eq!(typed(&mut app, "/resume"), vec![Action::Threads]);
    assert_eq!(app.key(&ctrl('t'), 80), vec![Action::Threads]);
    assert_eq!(
        typed(&mut app, "/resume Fix the parser"),
        vec![Action::Resume("Fix the parser".into())]
    );
    assert_eq!(
        typed(&mut app, "/resume 0a1b"),
        vec![Action::Resume("0a1b".into())]
    );
}

#[test]
fn an_unknown_command_says_so_and_lists_the_ones_like_it() {
    let mut app = app();
    assert!(typed(&mut app, "/resum").is_empty());
    let text = shown(&mut app);
    assert!(text.contains("/resum is not a command."), "{text}");
    assert!(text.contains("/resume [ID or title]"), "{text}");
    assert!(!text.contains("/threads"), "{text}");
}

#[test]
fn the_welcome_card_is_three_short_facts() {
    use openagents_chat::router::{Computer, Engine, Project};
    let context = Context {
        computer: Some(Computer::Here {
            name: None,
            engines: vec![
                Engine {
                    engine: "codex".into(),
                    state: EngineState::Ready,
                },
                Engine {
                    engine: "claude".into(),
                    state: EngineState::NotSignedIn,
                },
            ],
        }),
        project: Project::at("/work/demo"),
        ..Context::default()
    };
    let card = welcome(Kind::InProcess, &context, None);
    let shown = Row::Card(card)
        .text(80, Ladder::new(Colors::None))
        .join("\n");
    assert!(shown.contains("OpenAgents dev build"), "{shown}");
    assert!(shown.contains("Project  /work/demo"), "{shown}");
    // Only the agents that can run are named, without a word of state.
    assert!(shown.contains("Agents   Codex"), "{shown}");
    assert!(
        !shown.contains("Claude Code") && !shown.contains("ready"),
        "{shown}"
    );
    assert!(!shown.contains("Chats"), "{shown}");
    // Three facts and nothing else: no prose, no key legend.
    assert!(
        !shown.contains("Ask anything") && !shown.contains("Enter send"),
        "{shown}"
    );
    let host = Row::Card(welcome(Kind::Host, &Context::default(), Some("Fix CI")))
        .text(80, Ladder::new(Colors::None))
        .join("\n");
    assert!(!host.contains("synced"), "{host}");
    assert!(!host.contains("Ctrl+S"), "a host needs no offer: {host}");
    assert!(host.contains("Project  none"), "{host}");
    assert!(host.contains("Agents   none signed in"), "{host}");
    assert!(host.lines().count() <= 5, "{host}");
}

/// A real Grok Build run (#10117, `openagents-chat`'s
/// `fixtures/coder-events/tools-grok.ndjson`), fed as the client streams
/// it: condensed, its looking calls show as one labelled line and its
/// command as one line; Ctrl+O shows each call with its output, and again
/// condenses them. `/expand` does the same.
#[test]
fn tool_calls_are_grouped_condensed_and_expand_with_ctrl_o() {
    let mut app = app();
    for text in
        include_str!("../../openagents-chat/fixtures/coder-events/tools-grok.ndjson").lines()
    {
        let parsed: CoderLine = serde_json::from_str(text).unwrap();
        app.event(Event::Line(Box::new(parsed)));
    }
    let condensed = shown(&mut app);
    check_snapshot("tools_grok_condensed_80", &condensed);
    assert!(condensed.contains("◈ Read 3 files, Listed 1 dir, Searched 2 patterns"));
    assert!(
        !condensed.contains("pub fn add"),
        "file contents stay hidden"
    );
    assert!(app.key(&ctrl('o'), 80).is_empty());
    let expanded = shown(&mut app);
    check_snapshot("tools_grok_expanded_80", &expanded);
    assert!(expanded.contains("◆ Read lib.rs"));
    assert!(expanded.contains("pub fn add"));
    assert!(expanded.contains("$ cargo build && echo"));
    app.key(&ctrl('o'), 80);
    assert_eq!(shown(&mut app), condensed);
    assert!(typed(&mut app, "/expand").is_empty());
    assert_eq!(shown(&mut app), expanded);
}

/// A long Codex-shaped run: its first commands fold under one count, the
/// last ten stay, and a failure says its exit.
#[test]
fn a_long_run_of_commands_folds_its_oldest() {
    use openagents_chat::coder_events::{Call, Output, Step, StepKind, Verb};
    let mut app = app();
    app.event(line(1, started()));
    let mut seq = 1;
    for n in 0..13 {
        let command = format!("cargo test -p part{n}");
        seq += 1;
        app.event(line(
            seq,
            CoderEvent::Step(Step {
                turn: 1,
                step_id: n,
                kind: StepKind::Command,
                source: "agent".into(),
                text: command.clone(),
                call: Some(Call {
                    verb: Verb::Run,
                    target: command.clone(),
                    about: None,
                    failed: false,
                }),
            }),
        ));
        seq += 1;
        app.event(line(
            seq,
            CoderEvent::Output(Output {
                turn: 1,
                step_id: n,
                command,
                exit: Some(if n == 12 { 101 } else { 0 }),
                timed_out: false,
                seconds: 1.0,
                text: "test result: ok".into(),
                truncated: false,
            }),
        ));
    }
    let text = shown(&mut app);
    check_snapshot("tools_folded_80", &text);
    assert!(text.contains("◈ Ran 3 commands"), "{text}");
    assert!(
        text.contains("◆ Run cargo test -p part12 · exit 101"),
        "{text}"
    );
    assert!(
        !text.contains("part0\n") && !text.contains("test result"),
        "{text}"
    );
}

fn check_snapshot(name: &str, actual: &str) {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("snapshots")
        .join(format!("{name}.txt"));
    let actual = format!("{}\n", actual.trim_end());
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("no snapshot {name}; run with UPDATE_SNAPSHOTS=1:\n{actual}"));
    assert_eq!(expected, actual, "the {name} snapshot differs");
}

/// A start never leaves the screen silent and says little (#10115): the
/// rail says who is starting from the dispatch reply on, and the run's
/// start is one line, with no task ID, worktree path, or "signed in".
#[test]
fn a_start_shows_starting_then_one_line() {
    use openagents_chat::coder_events::Runner;
    let mut app = app();
    app.began(&Op::Send {
        thread: app.thread.clone(),
        new: true,
        text: "do a test delegation to grok".into(),
        start: Start::Settings,
        timeout: openagents_chat::client::DEFAULT_TIMEOUT,
    });
    let runner = Runner::Runs {
        provider: "grok".into(),
        model: "default".into(),
        passed: Vec::new(),
        requested: Some("grok".into()),
    };
    let Event::Reply { thread, reply, .. } = reply(
        "We'll dispatch Grok Build to take this on.",
        Meta {
            offers: vec![Offer::RunCoder],
            runner: Some(runner.clone()),
            ..Meta::default()
        },
    ) else {
        unreachable!()
    };
    app.event(Event::Reply {
        thread,
        reply,
        computer: true,
        running: true,
    });
    assert_eq!(app.status(), "Starting Grok Build…");
    assert!(app.busy(), "the spinner turns while it starts");
    app.event(Event::Starting {
        thread: app.thread.clone(),
        engine: "grok".into(),
    });
    app.event(Event::Coder {
        thread: app.thread.clone(),
        accepted: true,
        message: "Coder started.".into(),
        task: Some(serde_json::json!({"task": "t1", "worktree": "/tmp/w/t1"})),
        quiet: true,
    });
    assert_eq!(app.status(), "Starting Grok Build…");
    let CoderEvent::CoderStarted(mut grok) = started() else {
        unreachable!()
    };
    grok.provider = "grok".into();
    grok.reason = "You asked for Grok Build; it is signed in and has capacity.".into();
    grok.runner = Some(runner);
    app.event(line(1, CoderEvent::CoderStarted(grok)));
    assert_eq!(app.status(), "Working · Esc stops");
    let shown = shown(&mut app);
    let after: Vec<&str> = shown
        .lines()
        .skip_while(|line| !line.contains("We'll dispatch"))
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(after.len(), 1, "{shown}");
    assert!(after[0].contains("Grok Build is working."), "{shown}");
    for noise in ["t1", "/tmp/w", "signed in", "Coder started"] {
        assert!(!shown.contains(noise), "{noise}: {shown}");
    }
}

#[test]
fn help_says_what_esc_does_and_never_names_a_limit() {
    let card = help();
    let text = card
        .rows
        .iter()
        .flat_map(|(label, value)| [label.as_str(), value.as_str()])
        .chain(card.body.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    assert!(text.contains("press esc to stop a reply or a coder run"));
    for word in ["limit", "budget", "quota"] {
        assert!(!text.contains(word), "help names a {word}: {text}");
    }
}

#[test]
fn a_sent_prompt_is_handed_to_the_screen_once_and_up_reaches_saved_ones() {
    let mut app = app();
    app.editor.set_history(vec!["saved earlier".into()]);
    typed(&mut app, "hello");
    assert_eq!(app.take_sent().as_deref(), Some("hello"));
    assert_eq!(app.take_sent(), None);
    app.key(&key(KeyCode::Enter), 80);
    assert_eq!(app.take_sent(), None);
    app.key(&key(KeyCode::Up), 80);
    assert_eq!(app.editor.text(), "hello");
    app.key(&key(KeyCode::Up), 80);
    assert_eq!(app.editor.text(), "saved earlier");
}

#[test]
fn ctrl_y_copies_the_last_reply_then_its_code_blocks() {
    let mut app = app();
    assert!(app.key(&ctrl('y'), 80).is_empty());
    assert!(shown(&mut app).contains("No reply to copy yet."));
    app.push(Row::Turn(Who::OpenAgents, "older".into()));
    let reply = "Run:\n\n```sh\nls\n```\n\nthen\n\n```rust\nfn main() {}\n```";
    app.push(Row::Turn(Who::OpenAgents, reply.into()));
    app.push(Row::Turn(Who::You, "thanks".into()));
    let copy = |app: &mut App| app.key(&ctrl('y'), 80);
    assert_eq!(copy(&mut app), vec![Action::Copy(reply.into())]);
    assert_eq!(copy(&mut app), vec![Action::Copy("fn main() {}".into())]);
    assert_eq!(copy(&mut app), vec![Action::Copy("ls".into())]);
    assert_eq!(copy(&mut app), vec![Action::Copy(reply.into())]);
    assert!(shown(&mut app).contains("Copied code block 2 of 2."));
    // Another key in between starts over at the reply.
    copy(&mut app);
    app.key(&key(KeyCode::Char('x')), 80);
    assert_eq!(copy(&mut app), vec![Action::Copy(reply.into())]);
}

/// A run's result shows its files collapsed; Ctrl+O shows each file's
/// patch under it (#10152, drawn as grok-build draws a diff: #10154), and
/// condenses it again.
#[test]
fn a_results_changes_expand_with_ctrl_o() {
    use openagents_chat::coder_events::{FileChange, Finished};
    let mut app = app();
    app.event(line(1, started()));
    app.event(line(
        2,
        CoderEvent::Result(Finished {
            turn: 1,
            summary: "I renamed it.".into(),
            files_changed: vec![FileChange {
                path: "src/lib.rs".into(),
                status: "modified".into(),
                added: Some(1),
                removed: Some(1),
                patch: Some("@@ -1 +1 @@\n-pub fn old() {}\n+pub fn new() {}".into()),
                patch_cut: 0,
            }],
            insertions: 1,
            deletions: 1,
            worktree: "/w".into(),
            trajectory: "/t".into(),
            issue: None,
            cost_microusd: Some(940_000),
        }),
    ));
    let collapsed = shown(&mut app);
    assert!(
        collapsed.contains("modified src/lib.rs (+1 -1)"),
        "{collapsed}"
    );
    assert!(collapsed.contains("Press Ctrl+O to see the changes."));
    assert!(!collapsed.contains("pub fn new"));
    // The run's cost is recorded, never shown (owner, 2026-10-02).
    assert!(!collapsed.contains('$'), "{collapsed}");
    app.key(&ctrl('o'), 80);
    let expanded = shown(&mut app);
    // Drawn as grok-build draws an edit: numbered, no +/- marks (#10154).
    assert!(expanded.contains("1  pub fn old() {}"), "{expanded}");
    assert!(expanded.contains("1  pub fn new() {}"));
    assert!(!expanded.contains("Press Ctrl+O"));
    app.key(&ctrl('o'), 80);
    assert_eq!(shown(&mut app), collapsed);
}

/// When the chat cannot be reached, the screen says so once, calmly, and
/// says when it is back (#10151). The client sends `Offline` only after the
/// outage has lasted `QUIET_FOR`, so a restart's blip never reaches here.
#[test]
fn offline_is_said_once_and_the_rail_shows_the_next_try() {
    let mut app = app();
    let actions = typed(&mut app, "hello");
    let [Action::Run(op)] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    app.began(op);
    let thread = "a".repeat(32);
    for retry_in in [2, 4] {
        app.event(Event::Offline {
            thread: thread.clone(),
            retry_in,
        });
    }
    assert_eq!(app.status(), "reconnecting · Esc stops");
    assert_eq!(
        shown(&mut app)
            .matches("Reconnecting to OpenAgents… (Esc stops)")
            .count(),
        1
    );
    app.event(Event::Online { thread });
    assert_eq!(app.status(), "replying · Esc stops");
    assert!(shown(&mut app).ends_with("Connected again."));
    // Esc still stops the reply while it waits.
    assert_eq!(app.key(&key(KeyCode::Esc), 80), vec![Action::Interrupt]);
}

fn choice(key: &str, on: bool, blocked: Option<&str>) -> crate::Choice {
    crate::Choice {
        key: key.into(),
        label: key.into(),
        on,
        blocked: blocked.map(str::to_owned),
    }
}

/// `/settings` lists the choices; Enter or Space turns the selected one on
/// or off, and the list keeps its place when the change comes back.
#[test]
fn settings_turn_on_and_off_in_place() {
    let mut app = app();
    assert_eq!(typed(&mut app, "/settings"), vec![Action::Settings]);
    let settings = crate::Settings {
        path: "/s.json".into(),
        problem: None,
        choices: vec![
            choice("start", true, None),
            choice("agent:codex", true, None),
            choice("agent:opencode", false, Some("OpenCode needs a model.")),
        ],
    };
    app.settings(settings.clone());
    assert_eq!(
        app.key(&key(KeyCode::Enter), 80),
        vec![Action::Change {
            key: "start".into(),
            on: false
        }]
    );
    app.key(&key(KeyCode::Down), 80);
    assert_eq!(
        app.key(&key(KeyCode::Char(' ')), 80),
        vec![Action::Change {
            key: "agent:codex".into(),
            on: false
        }]
    );
    // A blocked choice says why instead of changing.
    app.key(&key(KeyCode::Down), 80);
    assert!(app.key(&key(KeyCode::Enter), 80).is_empty());
    assert!(shown(&mut app).ends_with("OpenCode needs a model."));
    // The change comes back: the list shows it, still on the same row.
    let mut changed = settings;
    changed.choices[2].on = true;
    app.settings(changed);
    let Some(Overlay::Settings { settings, selected }) = &app.overlay else {
        panic!("the list closed");
    };
    assert_eq!(*selected, 2);
    assert!(settings.choices[2].on);
    app.key(&key(KeyCode::Esc), 80);
    assert!(app.overlay.is_none());
}

fn plugin(name: &str, installed: bool) -> crate::Plugin {
    crate::Plugin {
        name: name.into(),
        about: "does a thing".into(),
        key: installed.then(|| format!("/plugins/{name}")),
        on: None,
        id: None,
    }
}

/// Space on an installed plugin turns it on or off on this computer; on a
/// published one it does nothing.
#[test]
fn space_turns_an_installed_plugin_on_or_off() {
    let mut app = app();
    let mut cleanup = plugin("disk-cleanup", true);
    cleanup.on = Some(false);
    cleanup.id = Some("00:disk-cleanup".into());
    app.overlay = Some(Overlay::Plugins {
        rows: vec![plugin("elsewhere", false), cleanup],
        selected: 0,
    });
    assert!(app.key(&key(KeyCode::Char(' ')), 80).is_empty());
    assert!(app.overlay.is_some());
    app.key(&key(KeyCode::Down), 80);
    assert_eq!(
        app.key(&key(KeyCode::Char(' ')), 80),
        vec![Action::TurnPlugin {
            id: "00:disk-cleanup".into(),
            name: "disk-cleanup".into(),
            on: true,
        }]
    );
    assert!(app.overlay.is_none());
}

/// Enter on a published plugin installs it by its id.
#[test]
fn enter_installs_a_published_plugin() {
    let mut app = app();
    let mut published = plugin("disk-cleanup", false);
    published.id = Some("aa:disk-cleanup".into());
    app.overlay = Some(Overlay::Plugins {
        rows: vec![published],
        selected: 0,
    });
    assert_eq!(
        app.key(&key(KeyCode::Enter), 80),
        vec![Action::InstallPlugin {
            id: "aa:disk-cleanup".into(),
            name: "disk-cleanup".into(),
        }]
    );
    assert!(app.overlay.is_none());
    assert!(shown(&mut app).contains("it starts off"));
}

/// `/plugins` lists them; Enter on an installed one takes the next message
/// as its request and runs it; one only published says it cannot run here.
#[test]
fn a_plugin_picked_from_the_list_runs_with_the_next_message() {
    let mut app = app();
    assert_eq!(typed(&mut app, "/plugins"), vec![Action::Plugins]);
    app.overlay = Some(Overlay::Plugins {
        rows: vec![plugin("explain-error", true), plugin("elsewhere", false)],
        selected: 1,
    });
    assert!(app.key(&key(KeyCode::Enter), 80).is_empty());
    assert!(shown(&mut app).contains("not installed on this computer"));
    assert!(app.plugin.is_none());
    app.overlay = Some(Overlay::Plugins {
        rows: vec![plugin("explain-error", true)],
        selected: 0,
    });
    app.key(&key(KeyCode::Enter), 80);
    assert_eq!(
        app.status(),
        "plugin explain-error · type what to ask · Esc cancels"
    );
    assert_eq!(
        typed(&mut app, "why does cargo fail"),
        vec![Action::RunPlugin {
            key: "/plugins/explain-error".into(),
            name: "explain-error".into(),
            request: "why does cargo fail".into(),
        }]
    );
    assert!(app.plugin.is_none());
    // Esc cancels a picked plugin; the next message goes to the chat.
    app.plugin = Some(plugin("explain-error", true));
    app.key(&key(KeyCode::Esc), 80);
    assert!(app.plugin.is_none());
    assert!(matches!(
        typed(&mut app, "hello").as_slice(),
        [Action::Run(Op::Send { .. })]
    ));
}

/// Another computer's threads: the welcome card names the computer, not
/// this one's project and agents.
#[test]
fn another_computers_welcome_names_it() {
    let mut app = App::new(
        Ladder::new(Colors::None),
        "a".repeat(32),
        true,
        Kind::Computer,
        "demo".into(),
    );
    app.computer = Some("Desk".into());
    app.welcome(&Context::default(), None, &["disk cleanup".into()]);
    let text = shown(&mut app);
    assert!(text.contains("Computer") && text.contains("Desk"), "{text}");
    assert!(!text.contains("Chats"), "{text}");
    assert!(!text.contains("│ Agents"), "{text}");
    assert!(
        !text.contains("watcher"),
        "this computer's watchers: {text}"
    );
}

/// The background watchers running here show on the welcome card at every
/// start, counted and named; with none, no row.
#[test]
fn the_welcome_card_counts_the_background_watchers() {
    let card = |watchers: &[String]| {
        let mut app = App::new(
            Ladder::new(Colors::None),
            "a".repeat(32),
            true,
            Kind::Host,
            "demo".into(),
        );
        app.welcome(&Context::default(), None, watchers);
        shown(&mut app)
    };
    let one = card(&["disk cleanup".into()]);
    assert!(
        one.contains("Running  1 background watcher · disk cleanup"),
        "{one}"
    );
    let two = card(&["disk cleanup".into(), "logs".into()]);
    assert!(
        two.contains("Running  2 background watchers · disk cleanup, logs"),
        "{two}"
    );
    let none = card(&[]);
    assert!(
        !none.contains("Running") && !none.contains("watcher"),
        "{none}"
    );
}

fn run_shown(app: &mut App) -> String {
    app.run_log
        .rows(80, |_| true)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn output(seq: u64, command: &str, text: &str) -> Event {
    line(
        seq,
        CoderEvent::Output(openagents_chat::coder_events::Output {
            turn: 1,
            step_id: seq,
            command: command.into(),
            exit: Some(0),
            timed_out: false,
            seconds: 1.0,
            text: text.into(),
            truncated: false,
        }),
    )
}

#[test]
fn the_run_view_shows_the_run_alone_and_its_composer_steers_it() {
    let mut app = app();
    // No run yet: nothing to open.
    assert!(app.key(&ctrl('r'), 80).is_empty());
    assert!(app.run_view.is_none());
    assert!(shown(&mut app).contains("This thread has no Coder run yet."));
    app.event(reply("Coder is on it.", Meta::default()));
    app.phase = Phase::Following;
    app.event(line(1, started()));
    app.event(output(2, "cargo test", "running 3 tests\nok"));
    assert!(app.key(&ctrl('r'), 80).is_empty());
    assert!(app.run_view.is_some());
    assert!(app.status().starts_with("Coder run · working"));
    // The run alone, with each command's output; the chat's reply is not
    // in it.
    let run = run_shown(&mut app);
    assert!(run.contains("running 3 tests"), "{run}");
    assert!(!run.contains("Coder is on it."), "{run}");
    // What is typed goes to the run, not to the chat.
    assert_eq!(
        typed(&mut app, "Use tabs, not spaces."),
        vec![Action::Steer {
            task: "t1".into(),
            text: "Use tabs, not spaces.".into()
        }]
    );
    app.steered(Ok(openagents_chat::client::Steering::NextStep));
    assert!(run_shown(&mut app).contains("Coder reads it at its next step."));
    // The run reads it: the trace records the person's message.
    app.event(line(
        3,
        CoderEvent::Step(openagents_chat::coder_events::Step {
            turn: 1,
            step_id: 3,
            kind: coder_events::StepKind::Message,
            source: "user".into(),
            text: "Use tabs, not spaces.".into(),
            call: None,
        }),
    ));
    assert!(run_shown(&mut app).contains("Coder read your message."));
    // Esc goes back to the chat; the run keeps going.
    assert!(app.key(&key(KeyCode::Esc), 80).is_empty());
    assert!(app.run_view.is_none() && app.running);
    // In the chat, a message goes to the router as before.
    app.phase = Phase::Idle;
    assert!(matches!(
        typed(&mut app, "how is it going?").as_slice(),
        [Action::Run(Op::Send { .. })]
    ));
}

#[test]
fn a_message_that_starts_the_next_turn_keeps_the_run_going() {
    let mut app = app();
    app.event(line(1, started()));
    app.key(&ctrl('r'), 80);
    let _ = typed(&mut app, "Stop and use spaces.");
    app.steered(Ok(openagents_chat::client::Steering::NextTurn(2)));
    // The turn the message replaced ends as stopped; the run goes on.
    app.event(line(
        2,
        CoderEvent::Stopped(Stopped {
            turn: 1,
            message: "Stopped to start again with the person's message.".into(),
        }),
    ));
    assert!(app.running, "the next turn is coming");
    let mut next = started();
    if let CoderEvent::CoderStarted(started) = &mut next {
        started.turn = 2;
    }
    app.event(line(3, next));
    app.event(line(
        4,
        CoderEvent::Stopped(Stopped {
            turn: 2,
            message: "Stopped by the person.".into(),
        }),
    ));
    assert!(!app.running, "the new turn's end ends it");
    // A refused message says why.
    app.steered(Err("Coder could not be reached.".into()));
    assert!(run_shown(&mut app).contains("Coder could not be reached."));
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn a_drag_copies_what_it_covers_and_a_click_opens_a_files_path() {
    let mut app = app();
    app.shown = crate::view::Shown {
        area: ratatui::layout::Rect::new(0, 0, 30, 2),
        rows: ["Changed src/app.rs:12 today", "and README.md"]
            .iter()
            .map(|row| format!("{row:<30}").chars().map(String::from).collect())
            .collect(),
    };
    let left = MouseButton::Left;
    assert!(
        app.mouse(&mouse(MouseEventKind::Down(left), 8, 0))
            .is_empty()
    );
    assert!(
        app.mouse(&mouse(MouseEventKind::Drag(left), 2, 1))
            .is_empty()
    );
    assert_eq!(
        app.mouse(&mouse(MouseEventKind::Up(left), 2, 1)),
        vec![Action::Copy("src/app.rs:12 today\nand".into())]
    );
    assert!(app.selection.is_some(), "the copied text stays marked");
    // A key clears the mark.
    app.key(&key(KeyCode::Left), 80);
    assert!(app.selection.is_none());
    // A click on a path asks the screen to open that file at its line;
    // on a plain word, it asks the same, and the screen finds no file.
    app.mouse(&mouse(MouseEventKind::Down(left), 12, 0));
    assert_eq!(
        app.mouse(&mouse(MouseEventKind::Up(left), 12, 0)),
        vec![Action::OpenFile {
            path: "src/app.rs".into(),
            line: Some(12)
        }]
    );
    // A click on a blank asks nothing.
    app.mouse(&mouse(MouseEventKind::Down(left), 28, 1));
    assert!(
        app.mouse(&mouse(MouseEventKind::Up(left), 28, 1))
            .is_empty()
    );
}

#[test]
fn an_open_file_fills_the_screen_and_esc_closes_it() {
    let mut app = app();
    app.event(line(1, started()));
    assert_eq!(
        app.bases(Some(std::path::Path::new("/tmp/demo"))),
        [
            std::path::PathBuf::from("/tmp/w/t1"),
            std::path::PathBuf::from("/tmp/demo")
        ],
        "the run's worktree first"
    );
    let text: String = (1..=50).map(|n| format!("line {n}\n")).collect();
    app.show_file(crate::view::FileView {
        path: "/tmp/w/t1/notes.txt".into(),
        lines: crate::view::file_lines(&text, "txt", app.ladder),
        top: 0,
    });
    assert_eq!(app.status(), "Esc closes the file");
    // Keys scroll it and never reach the composer.
    assert!(app.key(&key(KeyCode::PageDown), 80).is_empty());
    assert!(app.file.as_ref().unwrap().top > 0);
    assert!(app.key(&key(KeyCode::Char('x')), 80).is_empty());
    assert!(app.editor.is_empty());
    app.key(&key(KeyCode::Esc), 80);
    assert!(app.file.is_none());
}

#[test]
fn the_project_is_written_from_the_home_folder() {
    let home = std::env::var("HOME").unwrap();
    assert_eq!(home_relative(&format!("{home}/openagents")), "~/openagents");
    assert_eq!(home_relative(&home), "~");
    assert_eq!(home_relative("/srv/repo"), "/srv/repo");
    assert_eq!(
        home_relative(&format!("{home}x/repo")),
        format!("{home}x/repo")
    );
}

fn replay(fixture: &str) -> App {
    let mut app = app();
    for text in fixture.lines() {
        let parsed: CoderLine = serde_json::from_str(text).unwrap();
        app.event(Event::Line(Box::new(parsed)));
    }
    app
}

/// The two runs the owner watched on CoderOS (2026-10-02), replayed as the
/// host sent them: a Grok Build delegation and a Codex run. Their results
/// draw as Markdown, collapsed and expanded, never as `**`, backticks, or
/// list dashes; an expanded command shows once.
#[test]
fn the_owners_runs_draw_their_markdown() {
    for (fixture, bullet) in [
        (
            include_str!(
                "../../openagents-chat/fixtures/coder-events/owner-grok-delegation.ndjson"
            ),
            None,
        ),
        (
            include_str!("../../openagents-chat/fixtures/coder-events/owner-codex-terminal.ndjson"),
            Some("• 45 terminal unit tests."),
        ),
    ] {
        let mut app = replay(fixture);
        for expanded in [false, true] {
            let text = shown(&mut app);
            let raw: Vec<&str> = text
                .lines()
                .filter(|row| {
                    let lead = row.trim_start();
                    row.contains("**")
                        || (row.contains('`') && !lead.starts_with("$ ") && !lead.starts_with("◆ "))
                        || lead.starts_with("- ")
                })
                .filter(|row| !expanded || !row.starts_with("  "))
                .collect();
            assert!(raw.is_empty(), "raw Markdown: {raw:#?}\n{text}");
            if let Some(bullet) = bullet {
                assert!(text.contains(bullet), "{text}");
            }
            let mut previous = "";
            for row in text.lines() {
                let command = row.trim_start().strip_prefix("$ ");
                let call = previous.trim_start().strip_prefix("◆ Run ");
                if let (Some(command), Some(call)) = (command, call) {
                    assert_ne!(command, call, "a command shown twice:\n{text}");
                }
                previous = row;
            }
            app.key(&ctrl('o'), 80);
        }
    }
    let mut grok = replay(include_str!(
        "../../openagents-chat/fixtures/coder-events/owner-grok-delegation.ndjson"
    ));
    let text = shown(&mut grok);
    assert!(
        text.contains("\n Coder finished\n"),
        "nothing changed: no counts\n{text}"
    );
    assert!(text.contains("at detached commit fe70e10131"), "{text}");
}

fn finished() -> CoderEvent {
    CoderEvent::Result(openagents_chat::coder_events::Finished {
        turn: 1,
        summary: "Done.".into(),
        files_changed: Vec::new(),
        insertions: 0,
        deletions: 0,
        worktree: "/w".into(),
        trajectory: "/t".into(),
        issue: None,
        cost_microusd: None,
    })
}

/// Following a run again replays it from its first event: the rows already
/// shown stay as they are, but the replay still says where the run stands.
/// A replayed turn that ended leaves the screen idle; before, it stayed
/// "working" and followed the run again and again with nothing coming
/// (owner, 2026-10-02).
#[test]
fn a_replayed_run_that_ended_leaves_the_screen_idle() {
    let mut app = app();
    app.event(line(1, started()));
    app.event(line(2, finished()));
    assert!(!app.running);
    let before = shown(&mut app);
    // The next message's follow of the same task.
    app.began(&Op::Follow {
        thread: app.thread.clone(),
    });
    app.event(Event::Coder {
        thread: app.thread.clone(),
        accepted: true,
        message: "Following task t1.".into(),
        task: Some(serde_json::json!({"task": "t1"})),
        quiet: true,
    });
    assert!(app.running);
    app.event(line(1, started()));
    app.event(line(2, finished()));
    app.ended(None);
    assert!(!app.running, "the replay ended the turn");
    assert!(!app.wants_follow());
    assert!(app.live_status().is_none());
    assert_eq!(shown(&mut app), before, "nothing shown twice");
}

/// While a run has nothing to show, the line under the transcript says
/// what it is doing beside Grok Build's spinner, from the run's own
/// events, and how long it has been at it.
#[test]
fn the_spinner_line_says_what_the_run_is_doing() {
    use openagents_chat::coder_events::Status;
    let status = |seq: u64, text: &str| {
        line(
            seq,
            CoderEvent::Status(Status {
                turn: 1,
                step_id: seq,
                text: text.into(),
            }),
        )
    };
    let mut app = app();
    app.event(Event::Starting {
        thread: app.thread.clone(),
        engine: "grok".into(),
    });
    assert_eq!(
        app.live_status().map(|(text, _)| text),
        Some("Starting Grok Build…")
    );
    app.event(line(1, started()));
    app.tick += 30;
    let (text, waited) = app.live_status().unwrap();
    assert_eq!(text, "Starting Codex…");
    assert_eq!(coder_terminal::grok_spinner::timer(waited), "1.0s");
    app.event(status(2, "Grok Build connected · grok-4.7"));
    app.event(status(3, "Thinking…"));
    assert_eq!(app.live_status().map(|(text, _)| text), Some("Thinking…"));
    let rows = shown(&mut app);
    assert!(
        !rows.contains("Thinking…") && !rows.contains("connected"),
        "{rows}"
    );
    let drawn = crate::draw::working(
        "Thinking…",
        std::time::Duration::from_millis(1200),
        4,
        app.ladder,
    );
    let drawn: String = drawn
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    assert_eq!(drawn, "  ⠙ Thinking… 1.2s");
    app.event(line(4, finished()));
    assert!(app.live_status().is_none());
}

/// `/background` lists the rules; `r` shows a dry run first and runs only
/// when pressed again on the same rule; `p` pauses or resumes; `l` and
/// Enter read.
#[test]
fn background_runs_only_after_its_dry_run() {
    use crate::{BackgroundAct, BackgroundRow};
    let mut app = app();
    assert_eq!(typed(&mut app, "/background"), vec![Action::Background]);
    let rows = vec![BackgroundRow {
        id: "disk".into(),
        line: "disk · on".into(),
        paused: false,
    }];
    let open = |app: &mut App| {
        app.overlay = Some(Overlay::Background {
            rows: rows.clone(),
            selected: 0,
        });
    };
    let act = |act| {
        vec![Action::BackgroundAct {
            id: "disk".into(),
            act,
        }]
    };
    open(&mut app);
    assert_eq!(
        app.key(&key(KeyCode::Char('r')), 80),
        act(BackgroundAct::DryRun)
    );
    assert!(app.overlay.is_none());
    app.background_armed = Some("disk".into());
    open(&mut app);
    assert_eq!(
        app.key(&key(KeyCode::Char('r')), 80),
        act(BackgroundAct::Run)
    );
    open(&mut app);
    assert_eq!(
        app.key(&key(KeyCode::Char('r')), 80),
        act(BackgroundAct::DryRun)
    );
    open(&mut app);
    assert_eq!(
        app.key(&key(KeyCode::Char('p')), 80),
        act(BackgroundAct::Pause)
    );
    open(&mut app);
    assert_eq!(
        app.key(&key(KeyCode::Char('l')), 80),
        act(BackgroundAct::Log)
    );
    open(&mut app);
    assert_eq!(app.key(&key(KeyCode::Enter), 80), act(BackgroundAct::Show));
}

/// #10170, thread 7cd10ec8…: the worker judged "balance and wallet address
/// basic readonly identifying shit" on the computer lane with route
/// `wallet` and no `run_coder` offer. `run-coder` refuses that reply, so
/// the screen must not offer Coder for it; a `work.dispatch` reply on the
/// same lane is an offer, and Enter runs it.
#[test]
fn a_computer_lane_reply_without_an_offer_shows_no_enter_to_run_coder() {
    let mut app = app();
    app.fresh = false;
    let meta = Meta {
        tier: Some("model".into()),
        route: Some("wallet".into()),
        bank: Some("chat-answers-v1@5f34ebd34f0a".into()),
        ..Meta::default()
    };
    app.event(reply(
        "Coder can query the OpenAgents wallet here and report the current address and balance without sending transactions.",
        meta,
    ));
    assert!(!app.offer);
    let shown = shown(&mut app);
    assert!(!shown.contains("Enter to run Coder."), "{shown}");
    assert_ne!(app.status(), "Enter starts Coder");

    let mut app = self::app();
    app.fresh = false;
    let meta = Meta {
        route: Some(openagents_chat::delegation::DISPATCH_ROUTE.into()),
        ..Meta::default()
    };
    app.event(reply("Coder can fix that.", meta.clone()));
    assert!(app.offer);
    // What the screen offers is what `run-coder` accepts.
    assert!(openagents_chat::delegation::offered(Some(&meta), true));
}

fn of(task: &str, seq: u64, event: CoderEvent) -> Event {
    Event::Line(Box::new(CoderLine {
        seq,
        task: task.into(),
        thread: Some("a".repeat(32)),
        event,
    }))
}

fn started_on(provider: &str) -> CoderEvent {
    let CoderEvent::CoderStarted(mut started) = started() else {
        unreachable!()
    };
    started.provider = provider.into();
    CoderEvent::CoderStarted(started)
}

fn status(text: &str) -> CoderEvent {
    CoderEvent::Status(openagents_chat::coder_events::Status {
        turn: 1,
        step_id: 1,
        text: text.into(),
    })
}

/// Three runs the thread started, each with what it is doing.
fn three_runs() -> App {
    let mut app = app();
    app.event(of("t1", 1, started_on("codex")));
    app.event(of("t1", 2, status("Run cargo test")));
    app.event(of("t2", 1, started_on("claude")));
    app.event(of("t2", 2, status("Thinking…")));
    app.event(of("t3", 1, started_on("grok")));
    app.event(of("t3", 2, output(2, "ls", "a\nb").as_line_event()));
    app
}

trait LineEvent {
    fn as_line_event(self) -> CoderEvent;
}

impl LineEvent for Event {
    fn as_line_event(self) -> CoderEvent {
        match self {
            Event::Line(line) => line.event,
            _ => unreachable!(),
        }
    }
}

/// The whole frame as text, row by row.
fn frame(app: &mut App, width: u16, height: u16) -> String {
    let area = ratatui::layout::Rect::new(0, 0, width, height);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    crate::draw::draw(app, area, &mut buf);
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn running_runs_show_in_the_rail_under_the_composer() {
    let mut app = three_runs();
    assert_eq!(app.rail_numbers(), vec![1, 2, 3]);
    let rows = app.rail_rows();
    assert_eq!(rows[0].agent, "Codex");
    assert_eq!(rows[0].doing, "Run cargo test");
    assert_eq!(rows[1].doing, "Thinking…");
    assert!(rows.iter().all(|row| row.elapsed.is_some()));
    let shown = frame(&mut app, 60, 14);
    eprintln!("{shown}");
    let lines: Vec<&str> = shown.lines().collect();
    // The rail is the last three rows, under the composer's frame.
    assert!(lines[11].contains("1 Codex · Run cargo test"), "{shown}");
    assert!(lines[12].contains("2 Claude Code · Thinking…"), "{shown}");
    assert!(lines[13].contains("3 Grok"), "{shown}");
    // Text starts two cells in, and two cells stay free at the right.
    assert!(lines[11].starts_with("   "), "{shown}");
    assert!(
        lines[11..].iter().all(|line| line.chars().count() <= 58),
        "{shown}"
    );
    // A run that finished keeps its row for a while, then leaves.
    app.event(of(
        "t2",
        3,
        CoderEvent::Stopped(Stopped {
            turn: 1,
            message: "Stopped.".into(),
        }),
    ));
    assert_eq!(app.rail_rows()[1].elapsed, None);
    app.tick += crate::rail::KEPT_AFTER_DONE + 1;
    assert_eq!(app.rail_numbers(), vec![1, 3]);
}

#[test]
fn up_moves_into_the_rail_down_returns_and_enter_opens_full_screen() {
    let mut app = three_runs();
    // A draft keeps Up for the composer.
    app.editor.insert_str("draft");
    app.key(&key(KeyCode::Up), 80);
    assert_eq!(app.rail, None);
    app.editor.take();
    app.key(&key(KeyCode::Up), 80);
    assert_eq!(app.rail, Some(1));
    assert!(app.status().starts_with("Enter opens the run full screen"));
    app.key(&key(KeyCode::Up), 80);
    app.key(&key(KeyCode::Up), 80);
    app.key(&key(KeyCode::Up), 80);
    assert_eq!(app.rail, Some(3));
    app.key(&key(KeyCode::Down), 80);
    assert_eq!(app.rail, Some(2));
    app.key(&key(KeyCode::Down), 80);
    app.key(&key(KeyCode::Down), 80);
    assert_eq!(app.rail, None);
    // Enter on a row opens that run, not the thread's current one.
    app.key(&key(KeyCode::Up), 80);
    app.key(&key(KeyCode::Up), 80);
    assert!(app.key(&key(KeyCode::Enter), 80).is_empty());
    assert_eq!(app.run_view.and_then(|view| view.number), Some(2));
    assert_eq!(app.rail, None);
    assert!(
        app.status()
            .starts_with("Coder run 2 · Claude Code · working"),
        "{}",
        app.status()
    );
    // Its full screen is its log alone; what is typed steers it.
    assert_eq!(
        typed(&mut app, "use spaces"),
        vec![Action::Steer {
            task: "t2".into(),
            text: "use spaces".into()
        }]
    );
    let view = frame(&mut app, 60, 14);
    assert!(view.contains("use spaces"), "{view}");
    assert!(!view.contains("Run cargo test"), "{view}");
    // Esc goes back; Esc in the rail returns to the composer.
    app.key(&key(KeyCode::Esc), 80);
    assert!(app.run_view.is_none());
    app.key(&key(KeyCode::Up), 80);
    app.key(&key(KeyCode::Esc), 80);
    assert_eq!(app.rail, None);
    assert!(app.running, "Esc in the rail stops nothing");
    // A typed key leaves the rail and goes to the composer.
    app.key(&key(KeyCode::Up), 80);
    app.key(&key(KeyCode::Char('x')), 80);
    assert_eq!(app.rail, None);
    assert_eq!(app.editor.text(), "x");
}

#[test]
fn alt_and_a_number_or_open_and_a_number_opens_one() {
    let mut app = three_runs();
    let alt = KeyEvent {
        modifiers: KeyModifiers::ALT,
        ..key(KeyCode::Char('3'))
    };
    app.key(&alt, 80);
    assert_eq!(app.run_view.and_then(|view| view.number), Some(3));
    app.key(&key(KeyCode::Esc), 80);
    assert!(typed(&mut app, "/open 1").is_empty());
    assert_eq!(app.run_view.and_then(|view| view.number), Some(1));
    app.key(&key(KeyCode::Esc), 80);
    assert!(typed(&mut app, "/open 7").is_empty());
    assert!(app.run_view.is_none());
    assert!(shown(&mut app).contains("There are 3 Coder runs: /open 1 to /open 3."));
}

#[test]
fn unfollowed_runs_refresh_from_the_store_and_expire_without_end_events() {
    use openagents_chat::client::{Follow, Progress as State};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    struct StoreReader {
        task: String,
        store: Arc<Mutex<HashMap<String, Result<State, String>>>>,
    }
    impl Follow for StoreReader {
        fn poll(&mut self) -> Result<(Vec<CoderLine>, State), String> {
            self.store.lock().unwrap()[&self.task]
                .clone()
                .map(|state| (Vec::new(), state))
        }
    }
    let store = Arc::new(Mutex::new(HashMap::from([
        ("t1".into(), Ok(State::Running)),
        ("t2".into(), Ok(State::Running)),
        ("t3".into(), Ok(State::Running)),
    ])));
    let mut refresh = crate::rail::Refresh::default();
    let mut created = 0;
    let mut poll = |refresh: &mut crate::rail::Refresh| {
        refresh.poll(&["t1".into(), "t2".into(), "t3".into()], |task| {
            created += 1;
            Box::new(StoreReader {
                task: task.into(),
                store: store.clone(),
            })
        })
    };
    let mut app = three_runs();
    app.task = Some("t3".into());
    app.refresh_rail(poll(&mut refresh));
    assert!(app.delegations.iter().all(|held| held.running));
    store.lock().unwrap().insert("t2".into(), Ok(State::Ended));
    store
        .lock()
        .unwrap()
        .insert("t1".into(), Err("store busy".into()));
    app.tick = 30;
    app.refresh_rail(poll(&mut refresh));
    assert!(
        app.delegations[0].running,
        "read failures must not end a run"
    );
    assert!(!app.delegations[1].running);
    assert_eq!(app.delegations[1].ended, Some(30));
    let shown = frame(&mut app, 80, 14);
    let row = shown
        .lines()
        .find(|line| line.contains("2 Claude Code"))
        .unwrap();
    assert!(row.contains("done"), "{shown}");
    assert_eq!(app.rail_rows()[1].elapsed, None);
    app.tick += crate::rail::KEPT_AFTER_DONE;
    app.refresh_rail(poll(&mut refresh));
    assert_eq!(
        app.delegations[1].ended,
        Some(30),
        "polls must not reset expiry"
    );
    assert_eq!(app.rail_numbers(), vec![1, 2, 3]);
    app.tick += 1;
    app.refresh_rail(poll(&mut refresh));
    assert_eq!(app.rail_numbers(), vec![1, 3]);
    store
        .lock()
        .unwrap()
        .insert("t2".into(), Ok(State::Running));
    app.refresh_rail(poll(&mut refresh));
    assert_eq!(app.rail_numbers(), vec![1, 2, 3]);
    assert_eq!(app.delegations[1].ended, None);
    assert_eq!(app.delegations[1].since, app.tick);
    assert_eq!(
        created, 3,
        "refreshes reuse readers instead of replaying logs"
    );
}

/// #10170, thread 2deab1bf…: "openagents cli" got "Here's the openagents
/// command for that. It runs only when you confirm it." and nothing to
/// confirm. A command that waits shows itself and Enter runs it; a
/// read-only one runs at once and its output is the answer.
#[test]
fn a_proposed_command_shows_itself_and_enter_runs_it() {
    let mut app = app();
    app.fresh = false;
    let mut meta = Meta::default();
    meta.offered(
        &serde_json::json!({"offer": "cli", "argv": ["wallet", "init"],
        "effect": "local_write", "runs_on": "this_device", "confirm": true}),
    );
    app.event(reply(
        "Here's the openagents command for that. It runs only when you confirm it.",
        meta,
    ));
    app.event(Event::Command {
        thread: app.thread.clone(),
        argv: vec!["wallet".into(), "init".into()],
        confirm: true,
    });
    let screen = shown(&mut app);
    assert!(
        screen.contains("Enter runs openagents wallet init · Esc cancels"),
        "{screen}"
    );
    assert!(!app.offer);
    assert_eq!(app.status(), "Enter runs the command · Esc cancels");
    let actions = app.key(&key(KeyCode::Enter), 80);
    assert_eq!(
        actions,
        vec![Action::Run(Op::RunCommand {
            thread: app.thread.clone()
        })]
    );
    app.began(&Op::RunCommand {
        thread: app.thread.clone(),
    });
    app.event(Event::Ran {
        thread: app.thread.clone(),
        argv: vec!["wallet".into(), "init".into()],
        ok: true,
        output: "Wallet created.".into(),
    });
    app.ended(None);
    let screen = shown(&mut app);
    assert!(screen.contains("Wallet created."), "{screen}");
    assert!(app.command.is_none());

    // Esc cancels a waiting command.
    let mut app = self::app();
    app.fresh = false;
    app.event(Event::Command {
        thread: app.thread.clone(),
        argv: vec!["wallet".into(), "init".into()],
        confirm: true,
    });
    assert!(app.key(&key(KeyCode::Esc), 80).is_empty());
    assert!(app.command.is_none());
    assert!(app.key(&key(KeyCode::Enter), 80).is_empty());

    // A read-only command runs at once: no confirm, its output shown.
    let mut app = self::app();
    app.fresh = false;
    app.event(Event::Command {
        thread: app.thread.clone(),
        argv: vec!["wallet".into(), "balance".into()],
        confirm: false,
    });
    assert!(app.command.is_none());
    app.event(Event::Ran {
        thread: app.thread.clone(),
        argv: vec!["wallet".into(), "balance".into()],
        ok: true,
        output: "Your balance is ₿2,100 (0.00002100 BTC).".into(),
    });
    let screen = shown(&mut app);
    assert!(screen.contains("openagents wallet balance"), "{screen}");
    assert!(
        screen.contains("Your balance is ₿2,100 (0.00002100 BTC)."),
        "{screen}"
    );
    assert!(!screen.contains("Enter"), "{screen}");
}
