//! Frame-level proof that the interactive coder takes keys and shows replies.
//!
//! Every assertion here is against what the terminal would actually show. The
//! frames come from ratatui's `TestBackend`, so a test failing means the
//! reader would not have seen the thing, not that some intermediate value was
//! wrong.
//!
//! The end-to-end tests at the bottom run the real `run_loop` and the real
//! `runtime_actor` over a real HTTP server speaking real server-sent events.
//! Only the model behind that server is a stand-in.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use futures::Stream;
use openagents_cli::interactive::{run_loop, runtime_actor, CoderApp, Control, TurnEvent};
use openagents_cli::runtime::{CoderRuntimeSession, Lane};
use openagents_cli::tools::HarnessToolRegistry;

mod support;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

const WIDTH: u16 = 74;
const HEIGHT: u16 = 22;

fn terminal() -> Terminal<TestBackend> {
    terminal_of(WIDTH, HEIGHT)
}

fn terminal_of(width: u16, height: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(width, height)).expect("test terminal")
}

/// The frame as the reader would see it, one row per line.
fn screen(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn app() -> (
    CoderApp,
    UnboundedSender<Control>,
    UnboundedReceiver<Control>,
) {
    let (tx, rx) = unbounded_channel();
    (CoderApp::new("openagents coder"), tx, rx)
}

fn type_str(app: &mut CoderApp, control: &UnboundedSender<Control>, text: &str) {
    for ch in text.chars() {
        app.on_key(&key(KeyCode::Char(ch)), WIDTH, control);
    }
}

// ---------------------------------------------------------------- the input

#[test]
fn what_you_type_appears_in_the_composer() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "list the open issues");
    app.draw(&mut term).unwrap();

    let frame = screen(&term);
    assert!(
        frame.contains("› list the open issues"),
        "the composer did not show what was typed:\n{frame}"
    );
}

#[test]
fn backspace_takes_a_character_back_off_the_screen() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "hello");
    app.on_key(&key(KeyCode::Backspace), WIDTH, &control);
    app.draw(&mut term).unwrap();

    let frame = screen(&term);
    assert!(frame.contains("› hell"), "{frame}");
    assert!(!frame.contains("› hello"), "{frame}");
}

#[test]
fn the_caret_sits_where_the_next_character_will_go() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "abc");
    app.on_key(&key(KeyCode::Left), WIDTH, &control);
    app.draw(&mut term).unwrap();

    // The composer pane's left border is column 0, its inner text starts at
    // column 1, and the prompt `› ` takes two more.
    let (x, _y) = term.get_cursor_position().unwrap().into();
    assert_eq!(x, 1 + 2 + 2, "caret was not left of the last character");

    type_str(&mut app, &control, "X");
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("› abXc"), "{}", screen(&term));
}

#[test]
fn alt_enter_opens_a_second_composer_row_and_enter_still_sends() {
    let (mut app, control, mut rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "one");
    app.on_key(
        &KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT),
        WIDTH,
        &control,
    );
    type_str(&mut app, &control, "two");
    app.draw(&mut term).unwrap();

    let frame = screen(&term);
    assert!(frame.contains("› one"), "{frame}");
    assert!(frame.contains("  two"), "{frame}");

    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    match rx.try_recv() {
        Ok(Control::Prompt(prompt)) => assert_eq!(prompt, "one\ntwo"),
        other => panic!("Enter did not send both rows as one prompt: {other:?}"),
    }
}

// ------------------------------------------------------------ the turn cycle

#[test]
fn submitting_puts_the_prompt_on_the_transcript_and_asks_the_runtime() {
    let (mut app, control, mut rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "what changed today");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.draw(&mut term).unwrap();

    match rx.try_recv() {
        Ok(Control::Prompt(prompt)) => assert_eq!(prompt, "what changed today"),
        other => panic!("the runtime was not asked for a turn: {other:?}"),
    }

    let frame = screen(&term);
    assert!(
        frame.contains("what changed today"),
        "the prompt is not on the transcript:\n{frame}"
    );
    assert!(
        frame.contains("waiting for the reply"),
        "the composer does not say it is on hold:\n{frame}"
    );
    assert!(frame.contains("streaming"), "{frame}");
}

#[test]
fn a_reply_shows_up_while_it_is_still_arriving() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "hi");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);

    app.on_turn_event(TurnEvent::Chunk("The first ".to_string()));
    app.draw(&mut term).unwrap();
    let partial = screen(&term);
    assert!(
        partial.contains("The first"),
        "the first chunk was not drawn:\n{partial}"
    );
    assert!(
        app.busy(),
        "the turn was treated as finished by its first chunk"
    );

    app.on_turn_event(TurnEvent::Chunk("half arrived.".to_string()));
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("The first half arrived."));

    app.on_turn_event(TurnEvent::Done(String::new()));
    app.draw(&mut term).unwrap();
    let done = screen(&term);
    assert!(!app.busy(), "the composer stayed on hold after Done");
    assert!(done.contains("ready"), "{done}");
    assert!(!done.contains("waiting for the reply"), "{done}");
}

#[test]
fn keys_typed_during_a_turn_do_not_reach_the_composer() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "go");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    type_str(&mut app, &control, "ignored");
    app.draw(&mut term).unwrap();

    assert!(
        !screen(&term).contains("ignored"),
        "a key typed mid-turn landed in a composer that says it is on hold"
    );
}

#[test]
fn a_reply_with_no_chunks_falls_back_to_the_returned_answer() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "go");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Done("the whole answer at once".to_string()));
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("the whole answer at once"),
        "{}",
        screen(&term)
    );
}

#[test]
fn a_failed_turn_lands_on_the_transcript_and_the_session_stays_open() {
    let (mut app, control, mut rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "go");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    let _ = rx.try_recv();

    app.on_turn_event(TurnEvent::Failed("connection reset".to_string()));
    app.draw(&mut term).unwrap();

    let frame = screen(&term);
    assert!(frame.contains("Turn failed"), "{frame}");
    assert!(frame.contains("connection reset"), "{frame}");
    assert!(!app.should_exit(), "a failed turn ended the session");
    assert!(!app.busy(), "a failed turn left the composer on hold");

    // And the next prompt still goes out.
    type_str(&mut app, &control, "again");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    assert!(matches!(rx.try_recv(), Ok(Control::Prompt(p)) if p == "again"));
}

// ------------------------------------------------------------- the keybinds

#[test]
fn every_key_the_status_bar_names_does_something() {
    const WIDE: u16 = 120;
    let (mut app, control, mut rx) = app();
    let mut term = terminal_of(WIDE, HEIGHT);
    app.draw(&mut term).unwrap();
    let frame = screen(&term);

    // Whatever the bar claims, claim it here too, so a new label without a
    // key behind it fails this test.
    assert!(frame.contains("Enter: send"), "{frame}");
    assert!(frame.contains("Alt+Enter: newline"), "{frame}");
    assert!(frame.contains("PgUp/PgDn: scroll"), "{frame}");
    assert!(frame.contains("Esc: exit"), "{frame}");

    // Neither of the keys the old bar advertised is here. `Tab: effort` had
    // nothing behind it — `execute_turn` sends no effort field. `Shift+Tab:
    // lane` could not be given anything behind it: the thread endpoint
    // publishes no model parameter and the grant pins the model.
    assert!(!frame.contains("Tab: effort"), "{frame}");
    assert!(!frame.contains("Shift+Tab"), "{frame}");

    // Enter sends.
    type_str(&mut app, &control, "x");
    app.on_key(&key(KeyCode::Enter), WIDE, &control);
    assert!(matches!(rx.try_recv(), Ok(Control::Prompt(_))));
    app.on_turn_event(TurnEvent::Done("ok".to_string()));

    // Esc exits.
    app.on_key(&key(KeyCode::Esc), WIDE, &control);
    assert!(app.should_exit());
}

/// Shift+Tab is not bound, so it does nothing rather than pretending to.
#[test]
fn shift_tab_is_not_bound() {
    let (mut app, control, mut rx) = app();
    app.on_key(&key(KeyCode::BackTab), WIDTH, &control);
    assert!(!app.should_exit());
    assert!(app.model().is_none());
    assert!(
        rx.try_recv().is_err(),
        "Shift+Tab sent something to the runtime"
    );
}

/// The bar names the model the grant chose, and says so honestly before one.
#[test]
fn the_model_shown_is_the_one_the_grant_named() {
    let (mut app, control, _rx) = app();
    let mut term = terminal_of(120, HEIGHT);
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("Model: not yet granted"),
        "{}",
        screen(&term)
    );

    type_str(&mut app, &control, "go");
    app.on_key(&key(KeyCode::Enter), 120, &control);
    app.on_turn_event(TurnEvent::Model("ox-alpha-2".to_string()));
    app.on_turn_event(TurnEvent::Done("done".to_string()));
    app.draw(&mut term).unwrap();
    assert_eq!(app.model(), Some("ox-alpha-2"));
    assert!(
        screen(&term).contains("Model: ox-alpha-2"),
        "{}",
        screen(&term)
    );
}

/// PgUp reaches material the transcript has scrolled past, and PgDn returns.
#[test]
fn paging_up_shows_what_scrolled_off_the_top() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();

    type_str(&mut app, &control, "the first question");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Chunk(
        (1..=30)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n"),
    ));
    app.on_turn_event(TurnEvent::Done(String::new()));
    app.draw(&mut term).unwrap();

    // The newest rows are what the reader sees, so the prompt is off the top.
    let bottom = screen(&term);
    assert!(bottom.contains("line 30"), "{bottom}");
    assert!(!bottom.contains("the first question"), "{bottom}");

    for _ in 0..8 {
        app.on_key(&key(KeyCode::PageUp), WIDTH, &control);
    }
    app.draw(&mut term).unwrap();
    let scrolled = screen(&term);
    assert!(
        scrolled.contains("the first question"),
        "PgUp did not reach the prompt:\n{scrolled}"
    );
    assert!(!scrolled.contains("line 30"), "{scrolled}");

    for _ in 0..8 {
        app.on_key(&key(KeyCode::PageDown), WIDTH, &control);
    }
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("line 30"),
        "PgDn did not come back to the bottom:\n{}",
        screen(&term)
    );
}

/// The status bar's own row, which is the second from the bottom.
fn status_row(terminal: &Terminal<TestBackend>) -> String {
    let frame = screen(terminal);
    let rows: Vec<&str> = frame.lines().collect();
    rows[rows.len() - 2].to_string()
}

#[test]
fn a_narrow_window_drops_hints_rather_than_showing_half_of_one() {
    let (app, _control, _rx) = app();

    // Wide enough for two whole hints and no more.
    let mut term = terminal_of(74, HEIGHT);
    app.draw(&mut term).unwrap();
    let row = status_row(&term);
    assert!(row.contains("Enter: send \u{b7} Esc: exit"), "{row}");
    assert!(!row.contains("PgU"), "a hint was cut in half: {row}");

    // Too narrow for even the first: the status and the lane stay, the hints go.
    let mut term = terminal_of(46, HEIGHT);
    app.draw(&mut term).unwrap();
    let row = status_row(&term);
    assert!(row.contains("Status: ready"), "{row}");
    assert!(row.contains("Model: not yet granted"), "{row}");
    assert!(!row.contains("Ent"), "a hint was cut in half: {row}");
    for line in screen(&term).lines() {
        assert_eq!(
            line.chars().count(),
            46,
            "a row is not exactly the window's width:\n{}",
            screen(&term)
        );
    }
}

#[test]
fn ctrl_c_exits() {
    let (mut app, control, _rx) = app();
    app.on_key(
        &KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        WIDTH,
        &control,
    );
    assert!(app.should_exit());
}

#[test]
fn the_welcome_text_promises_only_what_the_screen_does() {
    let mut term = terminal();
    let (app, _control, _rx) = app();
    app.draw(&mut term).unwrap();
    let frame = screen(&term);
    assert!(frame.contains("Type a prompt"), "{frame}");
    // The frame keeps the boxed panes and their titles set into the rule.
    assert!(frame.contains("Agent Context"), "{frame}");
    assert!(frame.contains("Transcript"), "{frame}");
    assert!(frame.contains("Message"), "{frame}");
}

// ------------------------------------------------------------- end-to-end

/// A stream of terminal events the test writes by hand.
fn scripted(rx: UnboundedReceiver<Event>) -> impl Stream<Item = std::io::Result<Event>> + Unpin {
    Box::pin(futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|event| (Ok(event), rx))
    }))
}

async fn drive(
    app: &mut CoderApp,
    term: &mut Terminal<TestBackend>,
    keys: UnboundedReceiver<Event>,
    control: UnboundedSender<Control>,
    turns: &mut UnboundedReceiver<TurnEvent>,
    keepalive: UnboundedSender<TurnEvent>,
) {
    let mut events = scripted(keys);
    run_loop(term, app, &mut events, control, turns, keepalive)
        .await
        .expect("the loop returned an error");
}

fn send_keys(tx: &UnboundedSender<Event>, text: &str) {
    for ch in text.chars() {
        let _ = tx.send(Event::Key(key(KeyCode::Char(ch))));
    }
}

/// The whole loop, with a stub runtime on the other end of the real channels.
#[tokio::test]
async fn end_to_end_over_the_loop_with_a_stub_runtime() {
    let mut term = terminal();
    let mut app = CoderApp::new("openagents coder");
    let (keys_tx, keys_rx) = unbounded_channel();
    let (control_tx, mut control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();

    // The stand-in for `runtime_actor`: same channels, same message types.
    let stub_sink = turn_tx.clone();
    let keys_for_stub = keys_tx.clone();
    tokio::spawn(async move {
        while let Some(Control::Prompt(prompt)) = control_rx.recv().await {
            assert_eq!(prompt, "who are you");
            for chunk in ["I am ", "openagents ", "coder."] {
                let _ = stub_sink.send(TurnEvent::Chunk(chunk.to_string()));
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            let _ = stub_sink.send(TurnEvent::Done(String::new()));
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            let _ = keys_for_stub.send(Event::Key(key(KeyCode::Esc)));
        }
    });

    send_keys(&keys_tx, "who are you");
    let _ = keys_tx.send(Event::Key(key(KeyCode::Enter)));

    drive(
        &mut app,
        &mut term,
        keys_rx,
        control_tx,
        &mut turn_rx,
        turn_tx,
    )
    .await;

    let frame = screen(&term);
    assert!(
        frame.contains("who are you"),
        "the typed prompt is not on the final frame:\n{frame}"
    );
    assert!(
        frame.contains("I am openagents coder."),
        "the streamed reply is not on the final frame:\n{frame}"
    );
    assert!(app.should_exit(), "the loop did not exit on Esc");
}

// -------------------------------------------- end-to-end over real HTTP/SSE

/// The real loop, the real `runtime_actor`, and the real `CoderRuntimeSession`
/// against a real socket speaking real server-sent events.
///
/// The reader types a prompt, presses Enter, and the reply appears. The turn is
/// interrupted deliberately after its first chunk to prove the transcript is
/// showing text while the turn is still open — not assembling it at the end.
#[tokio::test]
async fn end_to_end_over_real_http_shows_a_chunk_before_the_turn_finishes() {
    let (gate_tx, gate_rx) = tokio::sync::oneshot::channel();
    let stub = support::start(vec!["Reading the ", "repository now."], Some(gate_rx)).await;

    let session = CoderRuntimeSession::new(
        Lane::OxAlpha,
        Some(stub.base),
        Some("oat_test".to_string()),
        HarnessToolRegistry::new(Some(std::env::temp_dir())),
    );

    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(session, control_rx, turn_tx.clone()));

    let mut term = terminal();
    let mut app = CoderApp::new("openagents coder");
    let (keys_tx, keys_rx) = unbounded_channel();

    send_keys(&keys_tx, "read the repo");
    let _ = keys_tx.send(Event::Key(key(KeyCode::Enter)));

    // Once the first chunk is on the transcript, exit — with the second chunk
    // still held behind the gate on the server.
    let keys_for_exit = keys_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        let _ = keys_for_exit.send(Event::Key(key(KeyCode::Esc)));
    });

    drive(
        &mut app,
        &mut term,
        keys_rx,
        control_tx,
        &mut turn_rx,
        turn_tx,
    )
    .await;
    let _ = gate_tx.send(());

    let frame = screen(&term);
    assert!(
        frame.contains("read the repo"),
        "the typed prompt is not on the frame:\n{frame}"
    );
    assert!(
        frame.contains("Reading the"),
        "the first streamed chunk never reached the transcript:\n{frame}"
    );
    assert!(
        !frame.contains("repository now."),
        "the held-back chunk arrived, so this run proves nothing about streaming:\n{frame}"
    );
    assert!(
        frame.contains("waiting for the reply"),
        "the turn was not still open when the frame was taken:\n{frame}"
    );
}

/// The same stack, allowed to finish, so the whole reply lands.
#[tokio::test]
async fn end_to_end_over_real_http_streams_a_whole_reply_onto_the_transcript() {
    let stub = support::start(vec!["Two files ", "changed today."], None).await;

    let session = CoderRuntimeSession::new(
        Lane::OxAlpha,
        Some(stub.base),
        Some("oat_test".to_string()),
        HarnessToolRegistry::new(Some(std::env::temp_dir())),
    );

    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(session, control_rx, turn_tx.clone()));

    let mut term = terminal();
    let mut app = CoderApp::new("openagents coder");
    let (keys_tx, keys_rx) = unbounded_channel();

    send_keys(&keys_tx, "what changed");
    let _ = keys_tx.send(Event::Key(key(KeyCode::Enter)));

    let keys_for_exit = keys_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        let _ = keys_for_exit.send(Event::Key(key(KeyCode::Esc)));
    });

    drive(
        &mut app,
        &mut term,
        keys_rx,
        control_tx,
        &mut turn_rx,
        turn_tx,
    )
    .await;

    let frame = screen(&term);
    assert!(frame.contains("what changed"), "{frame}");
    assert!(
        frame.contains("Two files changed today."),
        "the reply did not arrive whole:\n{frame}"
    );
    assert!(
        frame.contains("ready"),
        "the composer never came off hold:\n{frame}"
    );
}

/// A refused request reaches the reader as a failure, not as a finished turn.
///
/// Before this change `create_thread` answered a 401 by inventing a grant with
/// a placeholder token, and `execute_turn` answered the proxy's rejection by
/// streaming `Completed autonomous reasoning turn (offline fallback).` and
/// returning success. A session with no token therefore looked exactly like a
/// session that had worked.
#[tokio::test]
async fn a_refused_turn_says_so_on_the_transcript() {
    let stub = support::start_refusing().await;

    let session = CoderRuntimeSession::new(
        Lane::OxAlpha,
        Some(stub.base),
        None,
        HarnessToolRegistry::new(Some(std::env::temp_dir())),
    );

    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(session, control_rx, turn_tx.clone()));

    let mut term = terminal_of(100, HEIGHT);
    let mut app = CoderApp::new("openagents coder");
    let (keys_tx, keys_rx) = unbounded_channel();
    send_keys(&keys_tx, "hello");
    let _ = keys_tx.send(Event::Key(key(KeyCode::Enter)));

    let keys_for_exit = keys_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        let _ = keys_for_exit.send(Event::Key(key(KeyCode::Esc)));
    });

    drive(
        &mut app,
        &mut term,
        keys_rx,
        control_tx,
        &mut turn_rx,
        turn_tx,
    )
    .await;

    let frame = screen(&term);
    assert!(frame.contains("Turn failed"), "{frame}");
    assert!(frame.contains("401"), "{frame}");
    assert!(
        !frame.contains("offline fallback"),
        "a refused turn still reads as a completed one:\n{frame}"
    );
    assert!(
        frame.contains("ready"),
        "the composer stayed on hold:\n{frame}"
    );
}
