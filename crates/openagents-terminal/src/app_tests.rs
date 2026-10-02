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
    assert_eq!(typed(&mut app, "/connect"), vec![Action::Connect]);
    assert_eq!(typed(&mut app, "/quit"), vec![Action::Quit]);
    assert!(typed(&mut app, "/frobnicate").is_empty());
    assert!(shown(&mut app).contains("/frobnicate is not a command"));
    assert!(typed(&mut app, "/help").is_empty());
    let help = shown(&mut app);
    for slash in Slash::ALL {
        assert!(help.contains(&format!("/{}", slash.word())), "{help}");
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

#[test]
fn the_thread_list_opens_starts_and_archives() {
    let mut app = app();
    let row = |id: &str, title: &str| Summary {
        id: id.into(),
        title: title.into(),
        started: 1,
        updated: 2,
        coder: None,
        archived: false,
        pinned: false,
        named: false,
    };
    app.overlay = Some(Overlay::Threads {
        rows: vec![row("1", "one"), row("2", "two")],
        selected: 0,
    });
    assert!(app.key(&key(KeyCode::Down), 80).is_empty());
    assert_eq!(
        app.key(&key(KeyCode::Char('a')), 80),
        vec![Action::Archive("2".into())]
    );
    assert_eq!(
        app.key(&key(KeyCode::Enter), 80),
        vec![Action::Open("2".into())]
    );
    assert!(app.overlay.is_none());
    app.overlay = Some(Overlay::Threads {
        rows: vec![row("1", "one")],
        selected: 0,
    });
    assert_eq!(app.key(&key(KeyCode::Char('n')), 80), vec![Action::New]);
    app.overlay = Some(Overlay::Threads {
        rows: Vec::new(),
        selected: 0,
    });
    assert!(app.key(&key(KeyCode::Esc), 80).is_empty());
    assert!(app.overlay.is_none());
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
    assert!(
        shown.contains(&format!("OpenAgents v{}", env!("CARGO_PKG_VERSION"))),
        "{shown}"
    );
    assert!(shown.contains("Project  demo"), "{shown}");
    // Only the agents that can run are named, without a word of state.
    assert!(shown.contains("Agents   Codex"), "{shown}");
    assert!(
        !shown.contains("Claude Code") && !shown.contains("ready"),
        "{shown}"
    );
    assert!(
        shown.contains("Chats    this computer · Ctrl+S to sync"),
        "{shown}"
    );
    // Three facts and nothing else: no prose, no key legend.
    assert!(
        !shown.contains("Ask anything") && !shown.contains("Enter send"),
        "{shown}"
    );
    let host = Row::Card(welcome(Kind::Host, &Context::default(), Some("Fix CI")))
        .text(80, Ladder::new(Colors::None))
        .join("\n");
    assert!(host.contains("Chats    synced"), "{host}");
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
    assert_eq!(app.status(), "Coder is working · Esc stops");
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
