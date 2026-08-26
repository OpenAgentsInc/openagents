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
use openagents_cli::runtime::{CoderRuntimeSession, Lane, TurnUsage};
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

/// The first row inside the middle pane: below the header and the pane's own
/// top rule. A search that started above it would find a pane title, which is
/// drawn in the chrome's colours rather than the content's.
const PANE_TOP: u16 = 4;

/// Where a needle sits in the drawn frame, as (column, row), searching from
/// `from` downwards.
///
/// Counted in characters rather than bytes. The chrome is full of
/// box-drawing characters, which are three bytes each and one column each, so
/// a byte offset would point at the wrong cell on every row that has a rule
/// to the left of it.
fn position_from(terminal: &Terminal<TestBackend>, needle: &str, from: u16) -> (u16, u16) {
    let frame = screen(terminal);
    for (row, line) in frame.lines().enumerate().skip(from as usize) {
        if let Some(byte) = line.find(needle) {
            return (line[..byte].chars().count() as u16, row as u16);
        }
    }
    panic!("no row of the frame at or below {from} has {needle:?} on it:\n{frame}");
}

fn position_of(terminal: &Terminal<TestBackend>, needle: &str) -> (u16, u16) {
    position_from(terminal, needle, 0)
}

/// The cell the first character of `needle` is drawn in.
fn cell_of(terminal: &Terminal<TestBackend>, needle: &str) -> ratatui::buffer::Cell {
    let (column, row) = position_of(terminal, needle);
    terminal.backend().buffer()[(column, row)].clone()
}

/// The same, searching only inside the middle pane.
fn cell_in_the_pane(terminal: &Terminal<TestBackend>, needle: &str) -> ratatui::buffer::Cell {
    let (column, row) = position_from(terminal, needle, PANE_TOP);
    terminal.backend().buffer()[(column, row)].clone()
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
    (CoderApp::new("openagents coder", &Lane::default()), tx, rx)
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

/// Every key the transcript's status bar names has to do something.
///
/// The bar is the only place a reader learns what the session can do, so a
/// label with nothing behind it is the exact failure this issue was reopened
/// for. Each hint here is pressed, and each press is asserted.
#[test]
fn every_key_the_status_bar_names_does_something() {
    // Wide enough that the bar drops nothing; the narrow case is its own test.
    const WIDE: u16 = 200;
    let (mut app, control, mut rx) = app();
    let mut term = terminal_of(WIDE, HEIGHT);
    app.draw(&mut term).unwrap();
    let frame = screen(&term);

    // Whatever the bar claims, claim it here too, so a new label without a
    // key behind it fails this test.
    for hint in [
        "Enter: send",
        "Esc: exit",
        "Alt+Enter: newline",
        "Tab: complete",
        "\u{2191}\u{2193}: history",
        "PgUp/PgDn: scroll",
    ] {
        assert!(
            frame.contains(hint),
            "the bar does not offer {hint}:\n{frame}"
        );
    }

    // Neither of the keys the old bar advertised is here. `Tab: effort` had
    // nothing behind it — `execute_turn` sends no effort field. `Shift+Tab:
    // lane` could not be given anything behind it: the thread endpoint
    // publishes no model parameter and the grant pins the model.
    assert!(!frame.contains("Tab: effort"), "{frame}");
    assert!(!frame.contains("Shift+Tab"), "{frame}");

    // Alt+Enter opens a second row.
    type_str(&mut app, &control, "one");
    app.on_key(
        &KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT),
        WIDE,
        &control,
    );
    type_str(&mut app, &control, "two");
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("  two"), "{}", screen(&term));

    // Enter sends.
    app.on_key(&key(KeyCode::Enter), WIDE, &control);
    assert!(matches!(rx.try_recv(), Ok(Control::Prompt(p)) if p == "one\ntwo"));
    app.on_turn_event(TurnEvent::Done("ok".to_string()));

    // Tab completes: `/he` is only `/help`.
    type_str(&mut app, &control, "/he");
    app.on_key(&key(KeyCode::Tab), WIDE, &control);
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("\u{203a} /help "),
        "{}",
        screen(&term)
    );

    // Up recalls what was sent.
    for _ in 0..40 {
        app.on_key(&key(KeyCode::Backspace), WIDE, &control);
    }
    app.on_key(&key(KeyCode::Up), WIDE, &control);
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("\u{203a} one"), "{}", screen(&term));

    // PgUp scrolls, and PgDn comes back. Proved on its own transcript test;
    // here it is enough that the key is taken rather than typed.
    app.on_key(&key(KeyCode::PageUp), WIDE, &control);
    app.draw(&mut term).unwrap();
    assert!(!screen(&term).contains("PageUp"), "{}", screen(&term));

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

/// The bar names the model the last turn answered from, and says so
/// honestly before there has been one.
#[test]
fn the_model_shown_is_the_one_the_turn_answered_from() {
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
    app.on_turn_event(TurnEvent::Model("glm-5.3-flash-2".to_string()));
    app.on_turn_event(TurnEvent::Done("done".to_string()));
    app.draw(&mut term).unwrap();
    assert_eq!(app.model(), Some("glm-5.3-flash-2"));
    assert!(
        screen(&term).contains("Model: glm-5.3-flash-2"),
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
    let (mut app, _control, _rx) = app();

    // Wide enough for two whole hints and no more.
    let mut term = terminal_of(100, HEIGHT);
    app.draw(&mut term).unwrap();
    let row = status_row(&term);
    assert!(row.contains("Enter: send \u{b7} Esc: exit"), "{row}");
    assert!(!row.contains("Alt+Ent"), "a hint was cut in half: {row}");

    // Too narrow for even the first hint: the segments stay, the hints go.
    let mut term = terminal_of(74, HEIGHT);
    app.draw(&mut term).unwrap();
    let row = status_row(&term);
    assert!(row.contains("Lane: Coder Flash (flash)"), "{row}");
    assert!(!row.contains("Ent"), "a hint was cut in half: {row}");

    // Narrower still: the lowest-priority segment goes too, whole.
    let mut term = terminal_of(46, HEIGHT);
    app.draw(&mut term).unwrap();
    let row = status_row(&term);
    assert!(row.contains("Status: ready"), "{row}");
    assert!(row.contains("Model: not yet granted"), "{row}");
    assert!(!row.contains("Ent"), "a hint was cut in half: {row}");
    assert!(
        !row.contains("Lane"),
        "a segment was kept that could not fit: {row}"
    );
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
    let (mut app, _control, _rx) = app();
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
    let mut app = CoderApp::new("openagents coder", &Lane::default());
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
        Lane::default(),
        Some(stub.base),
        Some("oat_test".to_string()),
        HarnessToolRegistry::new(Some(std::env::temp_dir())),
    );

    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(session, control_rx, turn_tx.clone()));

    let mut term = terminal();
    let mut app = CoderApp::new("openagents coder", &Lane::default());
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
        Lane::default(),
        Some(stub.base),
        Some("oat_test".to_string()),
        HarnessToolRegistry::new(Some(std::env::temp_dir())),
    );

    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(session, control_rx, turn_tx.clone()));

    let mut term = terminal();
    let mut app = CoderApp::new("openagents coder", &Lane::default());
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
        Lane::default(),
        Some(stub.base),
        None,
        HarnessToolRegistry::new(Some(std::env::temp_dir())),
    );

    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(session, control_rx, turn_tx.clone()));

    let mut term = terminal_of(100, HEIGHT);
    let mut app = CoderApp::new("openagents coder", &Lane::default());
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

// ------------------------------------------------------- markdown and code

/// A model writes markdown. Before this the transcript printed the marks.
#[test]
fn a_reply_in_markdown_is_rendered_rather_than_printed_with_its_marks() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "explain");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Chunk(
        "Run `mix precommit` and read **the output**.".to_string(),
    ));
    app.on_turn_event(TurnEvent::Done(String::new()));
    app.draw(&mut term).unwrap();

    let frame = screen(&term);
    assert!(
        frame.contains("Run mix precommit and read the output."),
        "the markdown was not rendered:\n{frame}"
    );
    assert!(
        !frame.contains("**the output**"),
        "the marks are still on the screen:\n{frame}"
    );
    assert!(!frame.contains("`mix"), "{frame}");
}

/// A heading is drawn bold and cyan, which is a claim about the frame's cells
/// rather than about its text.
#[test]
fn a_heading_in_a_reply_is_drawn_bold() {
    use ratatui::style::Modifier;
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "go");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Chunk("## Findings".to_string()));
    app.on_turn_event(TurnEvent::Done(String::new()));
    app.draw(&mut term).unwrap();

    let frame = screen(&term);
    assert!(frame.contains("Findings"), "{frame}");
    assert!(!frame.contains("## Findings"), "{frame}");

    assert!(
        cell_of(&term, "Findings").modifier.contains(Modifier::BOLD),
        "the heading was not drawn bold:\n{frame}"
    );
}

/// A fenced block gets its rail, and the code inside it is highlighted.
#[test]
fn a_fenced_code_block_is_railed_and_highlighted_in_the_transcript() {
    use ratatui::style::Color;
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "show me");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Chunk(
        "```rust\nlet answer = 42;\n```".to_string(),
    ));
    app.on_turn_event(TurnEvent::Done(String::new()));
    app.draw(&mut term).unwrap();

    let frame = screen(&term);
    assert!(frame.contains("\u{256d}\u{2500} rust"), "{frame}");
    assert!(frame.contains("\u{2502} let answer = 42;"), "{frame}");
    assert!(frame.contains("\u{2570}\u{2500}"), "{frame}");

    // `let` is a keyword and is coloured as one.
    assert_eq!(
        cell_of(&term, "let answer").fg,
        Color::Magenta,
        "the keyword was not highlighted:\n{frame}"
    );
}

/// The half-written state every chunk but the last is in.
#[test]
fn a_code_fence_that_is_still_arriving_is_already_drawn_as_code() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "write it");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Chunk("```rust\nfn main() {".to_string()));
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("\u{2502} fn main() {"),
        "an unclosed fence held its contents back:\n{}",
        screen(&term)
    );
}

/// A notice this program wrote is not markdown and is not treated as any.
#[test]
fn a_notice_is_shown_as_the_text_it_is() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    app.on_turn_event(TurnEvent::Notice("Wrote src/some_file_name.rs".to_string()));
    app.draw(&mut term).unwrap();
    let _ = control;
    assert!(
        screen(&term).contains("Wrote src/some_file_name.rs"),
        "an underscore in a path was eaten as emphasis:\n{}",
        screen(&term)
    );
}

// ------------------------------------------------------------- status bar

#[test]
fn the_bar_names_the_lane_and_its_tier() {
    let mut app = CoderApp::new("openagents coder", &Lane::Flash);
    let mut term = terminal_of(120, HEIGHT);
    app.draw(&mut term).unwrap();
    assert!(
        status_row(&term).contains("Lane: Coder Flash (flash)"),
        "{}",
        status_row(&term)
    );
}

/// A lane that belongs to no tier is not given an invented one.
#[test]
fn a_lane_with_no_tier_is_named_without_one() {
    let mut app = CoderApp::new("openagents coder", &Lane::Named("some-model".to_string()));
    let mut term = terminal_of(120, HEIGHT);
    app.draw(&mut term).unwrap();
    let row = status_row(&term);
    assert!(row.contains("Lane: Coder (some-model)"), "{row}");
    assert!(!row.contains("(auto)"), "{row}");
}

/// Nothing is reported until the server has reported something. A zero would
/// read as "this turn was free", which is a different claim from "unknown".
#[test]
fn tokens_are_shown_only_once_the_server_has_said_what_a_turn_cost() {
    let (mut app, _control, _rx) = app();
    let mut term = terminal_of(140, HEIGHT);
    app.draw(&mut term).unwrap();
    assert!(
        !status_row(&term).contains("Tokens"),
        "{}",
        status_row(&term)
    );

    app.on_turn_event(TurnEvent::Usage(TurnUsage {
        prompt_tokens: 128,
        completion_tokens: 64,
        total_tokens: 192,
    }));
    app.draw(&mut term).unwrap();
    assert!(
        status_row(&term).contains("Tokens: 128+64=192"),
        "{}",
        status_row(&term)
    );
}

// ------------------------------------------------------------ the history

#[test]
fn up_and_down_walk_the_prompts_that_were_sent() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();

    type_str(&mut app, &control, "the first question");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Done("ok".to_string()));
    type_str(&mut app, &control, "the second question");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Done("ok".to_string()));

    app.on_key(&key(KeyCode::Up), WIDTH, &control);
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("\u{203a} the second question"),
        "Up did not recall the last prompt:\n{}",
        screen(&term)
    );

    app.on_key(&key(KeyCode::Up), WIDTH, &control);
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("\u{203a} the first question"),
        "{}",
        screen(&term)
    );

    app.on_key(&key(KeyCode::Down), WIDTH, &control);
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("\u{203a} the second question"),
        "{}",
        screen(&term)
    );
}

/// A half-typed line is not lost by looking back at the history.
#[test]
fn walking_back_and_forward_returns_the_draft() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "sent");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Done("ok".to_string()));

    type_str(&mut app, &control, "half typed");
    app.on_key(&key(KeyCode::Up), WIDTH, &control);
    app.on_key(&key(KeyCode::Down), WIDTH, &control);
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("\u{203a} half typed"),
        "the draft was lost:\n{}",
        screen(&term)
    );
}

/// A recalled prompt can be edited and sent again, which is the whole point.
#[test]
fn a_recalled_prompt_can_be_changed_and_sent_again() {
    let (mut app, control, mut rx) = app();
    type_str(&mut app, &control, "list the issues");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    let _ = rx.try_recv();
    app.on_turn_event(TurnEvent::Done("ok".to_string()));

    app.on_key(&key(KeyCode::Up), WIDTH, &control);
    type_str(&mut app, &control, " again");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    assert!(matches!(rx.try_recv(), Ok(Control::Prompt(p)) if p == "list the issues again"));
}

/// Scrolling is PgUp and PgDn, which is what the bar names. Up and Down are
/// the history, so a reader looking back at what they typed does not have the
/// transcript slide under them.
#[test]
fn up_does_not_scroll_the_transcript() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "a question");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Chunk(
        (1..=30)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n"),
    ));
    app.on_turn_event(TurnEvent::Done(String::new()));

    for _ in 0..10 {
        app.on_key(&key(KeyCode::Up), WIDTH, &control);
    }
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("line 30"),
        "Up scrolled the transcript:\n{}",
        screen(&term)
    );
}

// --------------------------------------------------------- the completions

fn scratch_directory() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::create_dir(dir.path().join("crates")).expect("crates/");
    std::fs::write(dir.path().join("README.md"), "").expect("README.md");
    dir
}

#[test]
fn tab_completes_the_only_command_that_matches() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let dir = scratch_directory();
    let mut app = CoderApp::new("openagents coder", &Lane::default())
        .with_working_directory(dir.path().to_path_buf());
    let mut term = terminal();

    type_str(&mut app, &tx, "/exp");
    app.on_key(&key(KeyCode::Tab), WIDTH, &tx);
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("\u{203a} /export "),
        "{}",
        screen(&term)
    );
}

#[test]
fn tab_lists_the_candidates_rather_than_choosing_one() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let dir = scratch_directory();
    let mut app = CoderApp::new("openagents coder", &Lane::default())
        .with_working_directory(dir.path().to_path_buf());
    let mut term = terminal();

    type_str(&mut app, &tx, "/");
    app.on_key(&key(KeyCode::Tab), WIDTH, &tx);
    app.draw(&mut term).unwrap();

    let frame = screen(&term);
    assert!(
        frame.contains("clear  diff  export  help  resume  run"),
        "the candidates were not listed:\n{frame}"
    );
    let composer = frame
        .lines()
        .find(|line| line.contains('\u{203a}'))
        .expect("a composer row");
    assert_eq!(
        composer.trim_matches('\u{2502}').trim_end(),
        "\u{203a} /",
        "Tab chose a command when five matched:\n{frame}"
    );
}

#[test]
fn tab_completes_a_path_in_the_working_directory() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let dir = scratch_directory();
    let mut app = CoderApp::new("openagents coder", &Lane::default())
        .with_working_directory(dir.path().to_path_buf());
    let mut term = terminal();

    type_str(&mut app, &tx, "look at REA");
    app.on_key(&key(KeyCode::Tab), WIDTH, &tx);
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("look at README.md"),
        "{}",
        screen(&term)
    );
}

/// The list is transient: the next keystroke narrows the set, so leaving the
/// old candidates up would be showing the answer to the previous question.
#[test]
fn the_candidate_list_goes_away_on_the_next_keystroke() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let dir = scratch_directory();
    let mut app = CoderApp::new("openagents coder", &Lane::default())
        .with_working_directory(dir.path().to_path_buf());
    let mut term = terminal();

    type_str(&mut app, &tx, "/");
    app.on_key(&key(KeyCode::Tab), WIDTH, &tx);
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("export"), "{}", screen(&term));

    type_str(&mut app, &tx, "c");
    app.draw(&mut term).unwrap();
    assert!(
        !screen(&term).contains("clear  diff  export"),
        "the stale candidate list stayed up:\n{}",
        screen(&term)
    );
}

// ------------------------------------------------------------- the commands

#[test]
fn an_unknown_command_is_refused_rather_than_sent_to_the_model() {
    let (mut app, control, mut rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "/difff");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.draw(&mut term).unwrap();

    assert!(
        rx.try_recv().is_err(),
        "a mistyped command was sent to the model as a prompt"
    );
    assert!(
        screen(&term).contains("There is no `/difff`"),
        "{}",
        screen(&term)
    );
}

#[test]
fn slash_help_lists_every_command_the_session_handles() {
    let (mut app, control, _rx) = app();
    let mut term = terminal_of(100, 40);
    type_str(&mut app, &control, "/help");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.draw(&mut term).unwrap();

    let frame = screen(&term);
    for (name, _) in openagents_cli::interactive::COMMANDS {
        assert!(
            frame.contains(&format!("/{name}")),
            "`/{name}` is handled and not listed:\n{frame}"
        );
    }
}

#[test]
fn slash_clear_empties_the_transcript() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "a question");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Done("an answer".to_string()));
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("an answer"));

    type_str(&mut app, &control, "/clear");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.draw(&mut term).unwrap();
    let frame = screen(&term);
    assert!(!frame.contains("an answer"), "{frame}");
    assert!(!frame.contains("a question"), "{frame}");
}

#[test]
fn slash_export_writes_the_transcript_where_it_was_told_to() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("session.txt");
    let (mut app, control, _rx) = app();
    let mut term = terminal();

    type_str(&mut app, &control, "what changed");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_turn_event(TurnEvent::Done("Two files.".to_string()));
    type_str(&mut app, &control, &format!("/export {}", path.display()));
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.draw(&mut term).unwrap();

    let written = std::fs::read_to_string(&path).expect("the transcript file");
    assert!(written.contains("[you] what changed"), "{written}");
    assert!(written.contains("[coder] Two files."), "{written}");
    assert!(
        screen(&term).contains("Transcript written to"),
        "{}",
        screen(&term)
    );
}

#[test]
fn slash_export_without_a_path_says_so_instead_of_guessing_one() {
    let (mut app, control, _rx) = app();
    let mut term = terminal();
    type_str(&mut app, &control, "/export");
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("needs a path"), "{}", screen(&term));
}

// --------------------------------------------------------- the diff inspector

const TWO_FILE_DIFF: &str = "\
diff --git a/lib/thing.ex b/lib/thing.ex
--- a/lib/thing.ex
+++ b/lib/thing.ex
@@ -1,3 +1,3 @@
 defmodule Thing do
-  def run, do: :old
+  def run, do: :new
 end
diff --git a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -1,2 +1,2 @@
 # Title
-first line
+second line
";

fn with_a_diff() -> (
    CoderApp,
    UnboundedSender<Control>,
    UnboundedReceiver<Control>,
) {
    let (mut app, tx, rx) = app();
    app.on_turn_event(TurnEvent::Diff(openagents_cli::diff::parse_unified(
        TWO_FILE_DIFF,
    )));
    (app, tx, rx)
}

#[test]
fn the_inspector_opens_on_a_diff_and_shows_the_change_unified() {
    let (mut app, _control, _rx) = with_a_diff();
    let mut term = terminal_of(90, HEIGHT);
    app.draw(&mut term).unwrap();

    let frame = screen(&term);
    assert!(app.inspecting(), "the inspector did not open");
    assert!(frame.contains("Diff \u{b7} lib/thing.ex"), "{frame}");
    assert!(frame.contains("1 of 2"), "{frame}");
    assert!(frame.contains("unified"), "{frame}");
    assert!(frame.contains("@@ -1,3 +1,3 @@"), "{frame}");
    assert!(frame.contains("\u{2212}   def run, do: :old"), "{frame}");
    assert!(frame.contains("+   def run, do: :new"), "{frame}");
    // The composer is not drawn: it would be a control that is not live.
    assert!(
        !frame.contains("Message"),
        "the composer was drawn under a pane that takes the keyboard:\n{frame}"
    );
}

/// Additions are green and removals red, asserted on the cells.
#[test]
fn the_two_sides_of_a_change_are_coloured_apart() {
    use ratatui::style::Color;
    let (mut app, _control, _rx) = with_a_diff();
    let mut term = terminal_of(90, HEIGHT);
    app.draw(&mut term).unwrap();

    assert_eq!(cell_of(&term, "def run, do: :old").fg, Color::Red);
    assert_eq!(cell_of(&term, "def run, do: :new").fg, Color::Green);
}

#[test]
fn v_switches_between_the_unified_and_side_by_side_views() {
    let (mut app, control, _rx) = with_a_diff();
    let mut term = terminal_of(90, HEIGHT);
    app.draw(&mut term).unwrap();

    app.on_key(&key(KeyCode::Char('v')), 90, &control);
    app.draw(&mut term).unwrap();
    let frame = screen(&term);
    assert!(frame.contains("side by side"), "{frame}");
    // Both texts on one row is what side by side means.
    assert!(
        frame
            .lines()
            .any(|line| line.contains(":old") && line.contains(":new")),
        "the two sides are not opposite each other:\n{frame}"
    );

    app.on_key(&key(KeyCode::Char('v')), 90, &control);
    app.draw(&mut term).unwrap();
    let frame = screen(&term);
    assert!(frame.contains("unified"), "{frame}");
    assert!(
        !frame
            .lines()
            .any(|line| line.contains(":old") && line.contains(":new")),
        "{frame}"
    );
}

#[test]
fn tab_moves_to_the_next_file_and_wraps_round() {
    let (mut app, control, _rx) = with_a_diff();
    let mut term = terminal_of(90, HEIGHT);
    app.draw(&mut term).unwrap();

    app.on_key(&key(KeyCode::Tab), 90, &control);
    app.draw(&mut term).unwrap();
    let frame = screen(&term);
    assert!(frame.contains("Diff \u{b7} README.md"), "{frame}");
    assert!(frame.contains("2 of 2"), "{frame}");
    assert!(frame.contains("second line"), "{frame}");

    app.on_key(&key(KeyCode::Tab), 90, &control);
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("1 of 2"), "{}", screen(&term));
}

#[test]
fn esc_closes_the_inspector_and_leaves_the_session_open() {
    let (mut app, control, _rx) = with_a_diff();
    let mut term = terminal_of(90, HEIGHT);
    app.draw(&mut term).unwrap();

    app.on_key(&key(KeyCode::Esc), 90, &control);
    app.draw(&mut term).unwrap();
    assert!(!app.inspecting());
    assert!(!app.should_exit(), "Esc in the inspector ended the session");
    assert!(screen(&term).contains("Message"), "{}", screen(&term));
}

/// While the inspector is up it has the keyboard, so a key that would have
/// been typed does not land in a composer nobody can see.
#[test]
fn the_inspector_takes_the_keyboard_from_the_composer() {
    let (mut app, control, _rx) = with_a_diff();
    let mut term = terminal_of(90, HEIGHT);
    type_str(&mut app, &control, "hello");
    app.on_key(&key(KeyCode::Esc), 90, &control);
    app.draw(&mut term).unwrap();
    assert!(
        !screen(&term).contains("\u{203a} hello"),
        "keys pressed over the inspector reached the composer:\n{}",
        screen(&term)
    );
}

#[test]
fn a_diff_with_no_changes_says_so_rather_than_opening_an_empty_pane() {
    let (mut app, _control, _rx) = app();
    let mut term = terminal();
    app.on_turn_event(TurnEvent::Diff(Vec::new()));
    app.draw(&mut term).unwrap();
    assert!(!app.inspecting());
    assert!(
        screen(&term).contains("Nothing has changed"),
        "{}",
        screen(&term)
    );
}

/// Scrolling stops at the last row rather than running off into a blank pane.
#[test]
fn scrolling_the_inspector_stops_at_the_end() {
    let (mut app, control, _rx) = with_a_diff();
    let mut term = terminal_of(90, HEIGHT);
    app.draw(&mut term).unwrap();
    for _ in 0..200 {
        app.on_key(&key(KeyCode::PageDown), 90, &control);
    }
    app.draw(&mut term).unwrap();
    let frame = screen(&term);
    assert!(
        frame.lines().any(|line| line.contains("end")),
        "the pane scrolled past everything it had:\n{frame}"
    );
}

/// `/diff` against a real repository, through the real actor. This is the
/// producer half: git is run, its output parsed, and the inspector opened.
#[tokio::test]
async fn slash_diff_shows_what_changed_in_a_real_repository() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git")
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "Test"]);
    std::fs::write(repo.join("thing.txt"), "keep\nold line\ntail\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "first"]);
    std::fs::write(repo.join("thing.txt"), "keep\nnew line\ntail\n").unwrap();

    let files = openagents_cli::interactive::collect_diff(&[], repo)
        .await
        .expect("a diff from git");
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(files[0].path, "thing.txt");
    assert_eq!(files[0].stats(), (1, 1));

    let mut app = CoderApp::new("openagents coder", &Lane::default())
        .with_working_directory(repo.to_path_buf());
    let mut term = terminal_of(90, HEIGHT);
    app.on_turn_event(TurnEvent::Diff(files));
    app.draw(&mut term).unwrap();
    let frame = screen(&term);
    assert!(frame.contains("thing.txt"), "{frame}");
    assert!(frame.contains("\u{2212} old line"), "{frame}");
    assert!(frame.contains("+ new line"), "{frame}");
}

/// Two files named directly are compared by this program, not by git, so
/// `/diff` works on files that are not in a repository at all.
#[tokio::test]
async fn slash_diff_with_two_paths_compares_the_two_files() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("before.txt"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(dir.path().join("after.txt"), "one\nTWO\nthree\nfour\n").unwrap();

    let files = openagents_cli::interactive::collect_diff(
        &["before.txt".to_string(), "after.txt".to_string()],
        dir.path(),
    )
    .await
    .expect("a diff of the two files");

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].stats(), (2, 1));
    assert_eq!(files[0].renamed_from.as_deref(), Some("before.txt"));
}

/// A directory git knows nothing about is a refusal with a reason, not a
/// silent empty inspector.
#[tokio::test]
async fn slash_diff_outside_a_repository_says_why_it_cannot() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let result = openagents_cli::interactive::collect_diff(&[], dir.path()).await;
    let message = result.expect_err("git should have refused here");
    assert!(
        message.to_lowercase().contains("git"),
        "the refusal does not say what refused: {message}"
    );
}

// ------------------------------------------------- programs under a terminal

use openagents_cli::pty::PtyControl;
use ratatui::layout::Rect;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A session whose runtime actor is real but whose model is unreachable.
///
/// `/run` and `/diff` never touch the model, so the actor these tests drive is
/// the production one and only the inference host is absent.
fn actor_session() -> CoderRuntimeSession {
    CoderRuntimeSession::new(
        Lane::default(),
        // Reserved by RFC 6890 as "this host on this network": nothing here
        // reaches it, and a test that accidentally tried would fail loudly.
        Some("http://192.0.2.1:9/api/v1".to_string()),
        None,
        HarnessToolRegistry::new(Some(std::env::temp_dir())),
    )
}

/// Deliver turn events to `app` until `done` holds or the deadline passes.
async fn pump<F>(
    app: &mut CoderApp,
    turns: &mut UnboundedReceiver<TurnEvent>,
    done: F,
    within: Duration,
) where
    F: Fn(&CoderApp) -> bool,
{
    let deadline = tokio::time::Instant::now() + within;
    loop {
        if done(app) {
            return;
        }
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return;
        }
        match tokio::time::timeout(left, turns.recv()).await {
            Ok(Some(event)) => app.on_turn_event(event),
            _ => return,
        }
    }
}

/// Start a session, run `command` in it, and pump until `done`.
async fn run_in_a_pane<F>(
    command: &str,
    size: (u16, u16),
    done: F,
    within: Duration,
) -> (
    CoderApp,
    Terminal<TestBackend>,
    UnboundedReceiver<TurnEvent>,
)
where
    F: Fn(&CoderApp) -> bool,
{
    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(actor_session(), control_rx, turn_tx.clone()));

    let mut term = terminal_of(size.0, size.1);
    let mut app = CoderApp::new("openagents coder", &Lane::default());
    // The frame's size is what the child is told, so it has to be known before
    // the child starts.
    app.draw(&mut term).unwrap();
    app.submit(command.to_string(), &control_tx);

    pump(&mut app, &mut turn_rx, done, within).await;
    app.draw(&mut term).unwrap();
    (app, term, turn_rx)
}

/// The claim this whole module exists for: the child is on a terminal.
///
/// `tty` prints the terminal it is attached to, and prints `not a tty` when it
/// is attached to a pipe. Under the buffered `Command::output` the crate used
/// everywhere else, this test would print the second.
#[tokio::test]
async fn a_program_run_in_the_frame_is_on_a_real_terminal() {
    let (_app, term, _rx) = run_in_a_pane(
        "/run tty",
        (80, 24),
        |app| app.pty_exit().is_some(),
        Duration::from_secs(10),
    )
    .await;

    let frame = screen(&term);
    assert!(
        frame.contains("/dev/"),
        "the child did not report a terminal:\n{frame}"
    );
    assert!(
        !frame.contains("not a tty"),
        "the child was given a pipe, not a pseudoterminal:\n{frame}"
    );
    assert!(frame.contains("Run \u{b7} tty"), "{frame}");
}

/// The child is told the size of the pane it is drawn into, and no other size.
///
/// `stty size` asks the kernel for the window size of its terminal, which only
/// exists because there is a terminal. An 80x24 frame leaves 78 columns and 16
/// rows inside the header, the status bar, and the pane's own rules.
#[tokio::test]
async fn the_program_is_told_the_size_of_the_pane_it_is_drawn_into() {
    let (_app, term, _rx) = run_in_a_pane(
        "/run stty size",
        (80, 24),
        |app| app.pty_exit().is_some(),
        Duration::from_secs(10),
    )
    .await;

    assert!(
        screen(&term).contains("16 78"),
        "the child was told the wrong window size:\n{}",
        screen(&term)
    );
}

/// Colour survives the trip: the child emits an SGR sequence and the frame
/// draws the cell in that colour. A pipe would have made most programs drop it.
#[tokio::test]
async fn colour_the_program_writes_reaches_the_frame() {
    use ratatui::style::Color;
    let (_app, term, _rx) = run_in_a_pane(
        r"/run printf \033[31mRED\033[0m",
        (80, 24),
        |app| app.pty_exit().is_some(),
        Duration::from_secs(10),
    )
    .await;

    assert_eq!(
        cell_in_the_pane(&term, "RED").fg,
        Color::Red,
        "the colour the child asked for was not drawn:\n{}",
        screen(&term)
    );
}

/// A full-screen program: it clears the screen, moves the cursor, and draws.
/// Nothing of that works down a pipe.
#[tokio::test]
async fn a_program_that_draws_a_screen_is_drawn_where_it_asked_to_be() {
    let (_app, term, _rx) = run_in_a_pane(
        // Cursor addressing without a semicolon in it: `5d` is line-position
        // absolute and `10G` is column absolute. A `;` would send the line to
        // a shell, which would then try to glob `[2J`.
        r"/run printf \033[2J\033[5d\033[10Gmiddle",
        (80, 24),
        |app| app.pty_exit().is_some(),
        Duration::from_secs(10),
    )
    .await;

    // The pane's inner area starts at column 1 and row 4 of the frame, and the
    // child asked for row 5, column 10 of its own screen.
    assert_eq!(
        position_from(&term, "middle", PANE_TOP),
        (1 + 9, 4 + 4),
        "\n{}",
        screen(&term)
    );
}

/// Keys typed reach the program, and `Ctrl+]` takes the keyboard back.
#[tokio::test]
async fn keys_reach_the_program_and_ctrl_bracket_takes_them_back() {
    // Wide enough that the status bar has room for its hint; the narrow case
    // is covered where the dropping rule is.
    const WIDE: u16 = 140;
    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(actor_session(), control_rx, turn_tx.clone()));

    let mut term = terminal_of(WIDE, HEIGHT);
    let mut app = CoderApp::new("openagents coder", &Lane::default());
    app.draw(&mut term).unwrap();
    app.submit("/run cat".to_string(), &control_tx);
    pump(
        &mut app,
        &mut turn_rx,
        |app| app.running(),
        Duration::from_secs(10),
    )
    .await;

    // The bar offers exactly the key that works here, and no other.
    app.draw(&mut term).unwrap();
    let frame = screen(&term);
    assert!(frame.contains("Ctrl+]: stop and go back"), "{frame}");
    assert!(!frame.contains("Enter: send"), "{frame}");
    assert!(frame.contains("Status: running"), "{frame}");

    // `cat` echoes a line once its terminal has one to give it.
    for ch in "ping".chars() {
        app.on_key(&key(KeyCode::Char(ch)), WIDE, &control_tx);
    }
    app.on_key(&key(KeyCode::Enter), WIDE, &control_tx);
    pump(
        &mut app,
        &mut turn_rx,
        |app| {
            app.pty_text()
                .is_some_and(|text| text.matches("ping").count() >= 2)
        },
        Duration::from_secs(10),
    )
    .await;
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).matches("ping").count() >= 2,
        "the keys did not reach the program:\n{}",
        screen(&term)
    );

    // Ctrl+] ends it and hands the keyboard back to the composer.
    app.on_key(
        &KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL),
        WIDE,
        &control_tx,
    );
    app.draw(&mut term).unwrap();
    let frame = screen(&term);
    assert!(!app.running(), "the program was not stopped");
    assert!(frame.contains("Stopped"), "{frame}");
    assert!(
        frame.contains("Message"),
        "the composer did not come back:\n{frame}"
    );
    assert!(!app.should_exit(), "Ctrl+] ended the session");
}

/// Esc belongs to the program, not to the session. A full-screen program that
/// could not receive Esc would be unusable, and a session that exited on it
/// would take the reader out of their editor.
#[tokio::test]
async fn esc_goes_to_the_program_rather_than_ending_the_session() {
    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(actor_session(), control_rx, turn_tx.clone()));

    let mut term = terminal();
    let mut app = CoderApp::new("openagents coder", &Lane::default());
    app.draw(&mut term).unwrap();
    app.submit("/run cat".to_string(), &control_tx);
    pump(
        &mut app,
        &mut turn_rx,
        |app| app.running(),
        Duration::from_secs(10),
    )
    .await;

    app.on_key(&key(KeyCode::Esc), WIDTH, &control_tx);
    assert!(!app.should_exit(), "Esc over a running program exited");
    assert!(app.running());

    app.on_key(
        &KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL),
        WIDTH,
        &control_tx,
    );
}

/// A program that ends leaves its output up, because that output is usually
/// the answer, and says how it ended.
#[tokio::test]
async fn a_program_that_fails_reports_its_exit_code() {
    let (mut app, term, _rx) = run_in_a_pane(
        "/run sh -c \"exit 3\"",
        (140, 24),
        |app| app.pty_exit().is_some(),
        Duration::from_secs(10),
    )
    .await;

    assert_eq!(app.pty_exit(), Some(3));
    assert!(screen(&term).contains("exited 3"), "{}", screen(&term));
    // The bar stops offering the key that stops a program that has stopped.
    assert!(
        screen(&term).contains("Enter: go back"),
        "{}",
        screen(&term)
    );
    assert!(!screen(&term).contains("Ctrl+]"), "{}", screen(&term));

    let (tx, _rx2) = tokio::sync::mpsc::unbounded_channel();
    let mut term = term;
    app.on_key(&key(KeyCode::Enter), 140, &tx);
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("exited with code 3"),
        "{}",
        screen(&term)
    );
    assert!(screen(&term).contains("Message"), "{}", screen(&term));
}

/// A command that does not exist is reported, and the session stays open.
#[tokio::test]
async fn a_command_that_is_not_there_is_reported_rather_than_hanging() {
    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(actor_session(), control_rx, turn_tx.clone()));

    let mut term = terminal_of(100, HEIGHT);
    let mut app = CoderApp::new("openagents coder", &Lane::default());
    app.draw(&mut term).unwrap();
    app.submit(
        "/run this-program-does-not-exist-anywhere".to_string(),
        &control_tx,
    );
    pump(
        &mut app,
        &mut turn_rx,
        |app| app.entries().iter().any(|e| e.text.contains("Could not")),
        Duration::from_secs(10),
    )
    .await;
    app.draw(&mut term).unwrap();

    assert!(!app.running());
    assert!(
        screen(&term).contains("Could not run it"),
        "{}",
        screen(&term)
    );
}

/// What a resize does to the program's side, without depending on a signal
/// arriving in a test's timing window.
#[derive(Debug, Default)]
struct RecordingControl {
    sizes: Mutex<Vec<(u16, u16)>>,
    killed: Mutex<bool>,
    written: Mutex<Vec<u8>>,
}

impl PtyControl for RecordingControl {
    fn write(&self, bytes: &[u8]) {
        self.written.lock().unwrap().extend_from_slice(bytes);
    }
    fn resize(&self, cols: u16, rows: u16) {
        self.sizes.lock().unwrap().push((cols, rows));
    }
    fn kill(&self) {
        *self.killed.lock().unwrap() = true;
    }
}

#[test]
fn resizing_the_window_resizes_the_program() {
    let (mut app, control, _rx) = app();
    let recorder = Arc::new(RecordingControl::default());
    app.on_turn_event(TurnEvent::PtyOpen {
        label: "cat".to_string(),
        control: recorder.clone(),
    });

    let mut term = terminal_of(80, 24);
    app.draw(&mut term).unwrap();
    // The first draw sets the pane's size; nothing has changed yet, so nothing
    // is sent — a resize the child did not need is a signal it did not need.
    assert!(recorder.sizes.lock().unwrap().is_empty());

    app.on_size(Rect::new(0, 0, 100, 30));
    assert_eq!(
        recorder.sizes.lock().unwrap().as_slice(),
        &[(98, 22)],
        "the new window size did not reach the program"
    );

    // And drawing at that size again does not send it twice.
    app.on_size(Rect::new(0, 0, 100, 30));
    assert_eq!(recorder.sizes.lock().unwrap().len(), 1);
    let _ = control;
}

#[test]
fn a_key_over_a_running_program_is_sent_as_the_bytes_a_terminal_would_send() {
    let (mut app, control, _rx) = app();
    let recorder = Arc::new(RecordingControl::default());
    app.on_turn_event(TurnEvent::PtyOpen {
        label: "cat".to_string(),
        control: recorder.clone(),
    });

    app.on_key(&key(KeyCode::Char('h')), WIDTH, &control);
    app.on_key(&key(KeyCode::Enter), WIDTH, &control);
    app.on_key(
        &KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        WIDTH,
        &control,
    );
    assert_eq!(recorder.written.lock().unwrap().as_slice(), b"h\r\x03");
    assert!(!*recorder.killed.lock().unwrap());

    app.on_key(
        &KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL),
        WIDTH,
        &control,
    );
    assert!(*recorder.killed.lock().unwrap(), "Ctrl+] did not stop it");
}

/// The whole loop, over a real pseudoterminal: keys in at the top, a program
/// run, and its ending reported on the transcript.
#[tokio::test]
async fn end_to_end_over_the_loop_running_a_program_under_a_terminal() {
    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(actor_session(), control_rx, turn_tx.clone()));

    let mut term = terminal_of(90, HEIGHT);
    let mut app = CoderApp::new("openagents coder", &Lane::default());
    let (keys_tx, keys_rx) = unbounded_channel();

    send_keys(&keys_tx, "/run tty");
    let _ = keys_tx.send(Event::Key(key(KeyCode::Enter)));

    // Once the program has ended, dismiss its pane and leave.
    let keys_for_exit = keys_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let _ = keys_for_exit.send(Event::Key(key(KeyCode::Enter)));
        tokio::time::sleep(Duration::from_millis(200)).await;
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
    assert!(app.should_exit(), "the loop did not exit");
    assert!(
        frame.contains("`tty` finished."),
        "the program did not run to completion through the loop:\n{frame}"
    );
}

/// The resize reaches the running program as a signal, not just as a number.
///
/// The shell traps `SIGWINCH` and prints the window size the kernel now
/// reports. Nothing about that is emulated: the size is set with the same
/// `TIOCSWINSZ` a terminal emulator uses, and the kernel is what raises the
/// signal.
#[tokio::test]
async fn resizing_the_frame_signals_the_running_program() {
    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(actor_session(), control_rx, turn_tx.clone()));

    let mut term = terminal_of(80, 24);
    let mut app = CoderApp::new("openagents coder", &Lane::default());
    app.draw(&mut term).unwrap();
    app.submit(
        "/run trap 'stty size' WINCH; stty size; for i in 1 2 3 4 5 6 7 8 9 10; do sleep 0.3; done"
            .to_string(),
        &control_tx,
    );

    let says = |needle: &'static str| {
        move |app: &CoderApp| app.pty_text().is_some_and(|text| text.contains(needle))
    };
    pump(
        &mut app,
        &mut turn_rx,
        says("16 78"),
        Duration::from_secs(10),
    )
    .await;
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("16 78"),
        "the program never reported its starting size:\n{}",
        screen(&term)
    );

    // Widen the window. `draw` is what notices, and what tells the child.
    term.backend_mut().resize(100, 30);
    app.draw(&mut term).unwrap();

    pump(
        &mut app,
        &mut turn_rx,
        says("22 98"),
        Duration::from_secs(10),
    )
    .await;
    app.draw(&mut term).unwrap();
    assert!(
        screen(&term).contains("22 98"),
        "the resize did not reach the program as a signal:\n{}",
        screen(&term)
    );

    app.on_key(
        &KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL),
        100,
        &control_tx,
    );
}

/// Every key the inspector's own status bar names has to do something too.
#[test]
fn every_key_the_inspectors_status_bar_names_does_something() {
    let (mut app, control, _rx) = with_a_diff();
    let mut term = terminal_of(200, HEIGHT);
    app.draw(&mut term).unwrap();
    let frame = screen(&term);
    for hint in [
        "Esc: close",
        "v: change view",
        "Tab: next file",
        "\u{2191}\u{2193} PgUp/PgDn: scroll",
    ] {
        assert!(
            frame.contains(hint),
            "the bar does not offer {hint}:\n{frame}"
        );
    }

    // v changes the view.
    app.on_key(&key(KeyCode::Char('v')), 200, &control);
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("side by side"), "{}", screen(&term));

    // Tab changes the file.
    app.on_key(&key(KeyCode::Tab), 200, &control);
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("2 of 2"), "{}", screen(&term));

    // Down scrolls, and Up comes back.
    app.on_key(&key(KeyCode::Down), 200, &control);
    app.draw(&mut term).unwrap();
    let scrolled = screen(&term);
    assert!(
        !scrolled.contains("README.md  +1"),
        "Down did not scroll the header off:\n{scrolled}"
    );
    app.on_key(&key(KeyCode::Up), 200, &control);
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("README.md  +1"), "{}", screen(&term));

    // PgDn moves further than Down did.
    app.on_key(&key(KeyCode::PageDown), 200, &control);
    app.on_key(&key(KeyCode::PageUp), 200, &control);
    app.draw(&mut term).unwrap();
    assert!(screen(&term).contains("README.md  +1"), "{}", screen(&term));

    // Esc closes.
    app.on_key(&key(KeyCode::Esc), 200, &control);
    assert!(!app.inspecting());
    assert!(!app.should_exit());
}

/// The status bar's model and token counts, over the real stack: a real socket
/// speaking real server-sent events, through the real `runtime_actor`.
///
/// The model is read from the session's `last_model` rather than from its
/// grant, which is what makes the local lane — which never opens a grant —
/// report a model at all.
#[tokio::test]
async fn end_to_end_over_real_http_the_bar_reports_the_model_and_the_tokens() {
    let stub = support::start_reporting_usage(vec!["Done."], (128, 64, 192)).await;

    let session = CoderRuntimeSession::new(
        Lane::default(),
        Some(stub.base),
        Some("oat_test".to_string()),
        HarnessToolRegistry::new(Some(std::env::temp_dir())),
    );

    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(session, control_rx, turn_tx.clone()));

    let mut term = terminal_of(140, HEIGHT);
    let mut app = CoderApp::new("openagents coder", &Lane::default());
    let (keys_tx, keys_rx) = unbounded_channel();
    send_keys(&keys_tx, "how much");
    let _ = keys_tx.send(Event::Key(key(KeyCode::Enter)));

    let keys_for_exit = keys_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(2000)).await;
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

    let row = status_row(&term);
    assert!(row.contains("Model: glm-5.3-flash"), "{row}");
    assert!(
        row.contains("Tokens: 128+64=192"),
        "the tokens the server reported are not on the bar: {row}"
    );
    assert_eq!(
        app.usage().total_tokens,
        192,
        "the session did not carry the reported usage"
    );
}

/// `/diff` end to end: typed into the composer, run by the real actor, and
/// opened in the inspector. The producer half of the inspector, over the same
/// channel the session uses.
#[tokio::test]
async fn slash_diff_typed_into_the_composer_opens_the_inspector() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git")
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "Test"]);
    std::fs::write(repo.join("thing.txt"), "keep\nold line\ntail\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "first"]);
    std::fs::write(repo.join("thing.txt"), "keep\nnew line\ntail\n").unwrap();

    // The actor runs git in the process's own working directory, so the test
    // runs from the repository it is asking about.
    let previous = std::env::current_dir().expect("a working directory");
    std::env::set_current_dir(repo).expect("move into the repository");

    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (turn_tx, mut turn_rx) = unbounded_channel::<TurnEvent>();
    tokio::spawn(runtime_actor(actor_session(), control_rx, turn_tx.clone()));

    let mut term = terminal_of(90, HEIGHT);
    let mut app = CoderApp::new("openagents coder", &Lane::default())
        .with_working_directory(repo.to_path_buf());
    type_str(&mut app, &control_tx, "/diff");
    app.on_key(&key(KeyCode::Enter), 90, &control_tx);

    pump(
        &mut app,
        &mut turn_rx,
        |app| app.inspecting(),
        Duration::from_secs(20),
    )
    .await;
    std::env::set_current_dir(previous).expect("go back");

    app.draw(&mut term).unwrap();
    let frame = screen(&term);
    assert!(
        app.inspecting(),
        "`/diff` never opened the inspector:\n{frame}"
    );
    assert!(frame.contains("thing.txt"), "{frame}");
    assert!(frame.contains("\u{2212} old line"), "{frame}");
    assert!(frame.contains("+ new line"), "{frame}");
}
