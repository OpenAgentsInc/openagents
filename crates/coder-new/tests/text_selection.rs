//! Selecting transcript text with the mouse: drag, double-click a word,
//! triple-click a line. Copies leave out the marks and gutters and join
//! soft-wrapped rows back into their line; the highlight follows the text
//! as the view scrolls.

use coder_new::{App, Mode, ViewMode, live::Entry, theme, ui};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use serde_json::json;

const LONG: &str = "The composer rail named a vendor's model because a saved setting chose it, and the default named the free router; both now read auto unless the person picks a model in /models.";

fn chat() -> App {
    let old = "fn a() {}\n\n/// Reads a file inside the workspace.\nfn read() {}\n";
    let new = "fn a() {}\n\n/// Reads a file on this computer.\nfn read() {}\n";
    let diff = coder_new::file_tools::unified(old, new);
    let mut app = App::default();
    app.set_mode(Mode::Live);
    // These tests select the detailed layout: diffs, results, blank rows.
    app.view = ViewMode::Verbose;
    // The chat already ran on Coder's own loop, so the rail names it.
    app.live.runs_on = Some(coder_new::harness_plugin::Harness::CoderLoop);
    app.live.entries.extend([
        Entry::User("Why does the rail name a vendor's model?".into()),
        Entry::Tool {
            name: "Run".into(),
            input: json!({"command": "cargo test -p coder-new"}),
            output: json!({"output": "running 415 tests\ntest result: ok. 415 passed"}),
            running: false,
        },
        Entry::Tool {
            name: "Edit".into(),
            input: json!({"path": "src/read.rs"}),
            output: json!({
                "path": "src/read.rs",
                "diff": diff.text,
                "added": diff.added,
                "removed": diff.removed,
            }),
            running: false,
        },
        Entry::Assistant {
            text: LONG.into(),
            model: None,
            elapsed_ms: None,
        },
    ]);
    app
}

fn render(app: &mut App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    terminal.backend().buffer().clone()
}

fn rows(buffer: &Buffer) -> Vec<String> {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

fn row_of(buffer: &Buffer, starts: &str) -> u16 {
    rows(buffer)
        .iter()
        .position(|row| row.starts_with(starts))
        .unwrap_or_else(|| {
            panic!(
                "no row starts with {starts:?}:\n{}",
                rows(buffer).join("\n")
            )
        }) as u16
}

/// The screen column `word` starts at on `row`.
fn column_of(buffer: &Buffer, row: u16, word: &str) -> u16 {
    let text = &rows(buffer)[usize::from(row)];
    text[..text.find(word).unwrap()].chars().count() as u16
}

fn mouse(app: &mut App, kind: MouseEventKind, column: u16, row: u16) {
    app.handle(Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }));
}

fn drag(app: &mut App, from: (u16, u16), to: (u16, u16)) {
    let left = MouseButton::Left;
    mouse(app, MouseEventKind::Down(left), from.0, from.1);
    mouse(app, MouseEventKind::Drag(left), to.0, to.1);
    mouse(app, MouseEventKind::Up(left), to.0, to.1);
}

fn click(app: &mut App, times: usize, column: u16, row: u16) {
    for _ in 0..times {
        mouse(app, MouseEventKind::Down(MouseButton::Left), column, row);
        mouse(app, MouseEventKind::Up(MouseButton::Left), column, row);
    }
}

/// What the app put on the clipboard since last asked.
fn copied(app: &mut App) -> Option<String> {
    let mut out = None;
    app.copy_pending(|text| {
        out = Some(text.to_owned());
        Ok(())
    });
    out
}

#[test]
fn triple_click_copies_a_wrapped_reply_as_its_one_line() {
    let mut app = chat();
    let buffer = render(&mut app, 40, 60);
    let first = row_of(&buffer, "● The composer");
    click(&mut app, 3, 10, first + 2);
    assert_eq!(copied(&mut app).as_deref(), Some(LONG));
}

#[test]
fn a_drag_copies_text_without_marks_gutters_or_diff_columns() {
    let mut app = chat();
    let buffer = render(&mut app, 60, 60);
    let user = row_of(&buffer, "❯ Why");
    let summary = row_of(&buffer, "● Edit src/read.rs (+1 -1)");
    let last_diff = row_of(&buffer, "     4   fn read");
    // From the middle of the question to the end of the diff, dragged
    // bottom to top.
    drag(&mut app, (59, last_diff), (6, user));
    assert_eq!(
        copied(&mut app).as_deref(),
        Some(
            "does the rail name a vendor's model?\n\n\
             Run cargo test -p coder-new\n\
             running 415 tests\n\
             test result: ok. 415 passed\n\n\
             Edit src/read.rs (+1 -1)\n\
             fn a() {}\n\
             \n\
             /// Reads a file inside the workspace.\n\
             /// Reads a file on this computer.\n\
             fn read() {}"
        )
    );
    assert!(summary > user);
}

#[test]
fn double_click_copies_a_word_and_the_highlight_marks_only_it() {
    let mut app = chat();
    let buffer = render(&mut app, 60, 60);
    let user = row_of(&buffer, "❯ Why");
    // "vendor's" ends at the quote; "rail" is a plain word.
    let column = column_of(&buffer, user, "rail") + 1;
    click(&mut app, 2, column, user);
    assert_eq!(copied(&mut app).as_deref(), Some("rail"));
    let buffer = render(&mut app, 60, 60);
    let lit: Vec<u16> = (0..60)
        .filter(|x| buffer[(*x, user)].bg == theme::TEXT_PRIMARY)
        .collect();
    assert_eq!(lit, (column - 1..column + 3).collect::<Vec<_>>());
    assert_eq!(buffer[(column, user)].fg, theme::BG_BASE);
    // The mark is never lit.
    assert_ne!(buffer[(0, user)].bg, theme::TEXT_PRIMARY);
}

#[test]
fn a_selection_stays_on_its_text_while_the_wheel_scrolls() {
    let mut app = chat();
    let height = 20;
    // Following the latest output, as a live chat does.
    app.scroll = u16::MAX;
    let buffer = render(&mut app, 40, height);
    let row = row_of(&buffer, "  model because");
    let column = column_of(&buffer, row, "saved");
    click(&mut app, 2, column, row);
    assert_eq!(copied(&mut app).as_deref(), Some("saved"));
    // The wheel still scrolls the transcript.
    mouse(&mut app, MouseEventKind::ScrollUp, 20, 3);
    let buffer = render(&mut app, 40, height);
    let moved = row_of(&buffer, "  model because");
    assert_eq!(moved, row + 3);
    let lit: Vec<u16> = (0..40)
        .filter(|x| buffer[(*x, moved)].bg == theme::TEXT_PRIMARY)
        .collect();
    assert_eq!(lit.len(), "saved".len());
    // Ctrl+Shift+C copies it again.
    app.handle(Event::Key(KeyEvent::new(
        KeyCode::Char('C'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    )));
    assert_eq!(copied(&mut app).as_deref(), Some("saved"));
    // Esc puts it down.
    app.handle(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    let buffer = render(&mut app, 40, height);
    assert!(
        (0..40).all(|x| buffer[(x, moved)].bg != theme::TEXT_PRIMARY),
        "the highlight stayed after Esc"
    );
}

#[test]
fn dragging_past_the_top_edge_scrolls_and_extends_the_selection() {
    let mut app = chat();
    let height = 20;
    // Following the latest output, as a live chat does.
    app.scroll = u16::MAX;
    let buffer = render(&mut app, 40, height);
    let last = row_of(&buffer, "  the person picks");
    let left = MouseButton::Left;
    mouse(&mut app, MouseEventKind::Down(left), 39, last);
    mouse(&mut app, MouseEventKind::Drag(left), 2, 0);
    for _ in 0..40 {
        app.tick();
        render(&mut app, 40, height);
    }
    mouse(&mut app, MouseEventKind::Up(left), 2, 0);
    let text = copied(&mut app).unwrap();
    assert!(text.starts_with("Why does the rail"), "{text}");
    assert!(text.ends_with("picks a model in /models."), "{text}");
}

#[test]
fn a_click_without_a_drag_copies_nothing() {
    let mut app = chat();
    let buffer = render(&mut app, 60, 60);
    let user = row_of(&buffer, "❯ Why");
    click(&mut app, 1, 8, user);
    assert_eq!(copied(&mut app), None);
}

/// A copy says so on the input's top-left rail, and adds no row of its own:
/// the transcript above keeps its height.
#[test]
fn a_copy_shows_copied_on_the_inputs_top_left_rail() {
    let mut app = chat();
    let before = rows(&render(&mut app, 60, 30));
    let top = before
        .iter()
        .position(|row| row.starts_with('┌'))
        .expect("the input's top rule") as u16;
    assert!(!before[usize::from(top)].contains("Copied"));
    let user = row_of(&render(&mut app, 60, 30), "❯ Why");
    click(&mut app, 3, 10, user);
    assert!(copied(&mut app).is_some());
    let after = rows(&render(&mut app, 60, 30));
    // The label rides the same top rule, and the agent keeps the right end.
    let rule = &after[usize::from(top)];
    assert!(rule.starts_with("┌─ Copied "), "{rule:?}");
    assert!(rule.trim_end().ends_with("─┐"), "{rule:?}");
    assert!(rule.contains("Coder"), "{rule:?}");
    // No row appears below the input for it, and the input keeps its height.
    assert_eq!(after.len(), before.len());
    assert!(after[usize::from(top) + 2].starts_with('└'));
    assert!(
        after[usize::from(top) + 3..]
            .iter()
            .all(|row| row.trim().is_empty())
    );
}
