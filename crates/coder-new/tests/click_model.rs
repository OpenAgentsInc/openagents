//! A plain click on a transcript message shows the model that produced it
//! at the clicked row's right end, in the dim gray, for a few seconds. A
//! drag still selects and shows no label, and the label never reaches the
//! transcript or the export.

use coder_new::{App, Mode, live::Entry, theme, trajectory, ui};
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

const WIDTH: u16 = 70;

fn chat() -> App {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.live.runs_on = Some(coder_new::harness_plugin::Harness::CoderLoop);
    app.live.entries.extend([
        Entry::User("Which file reads the settings?".into()),
        Entry::Assistant {
            text: "The settings load in preferences.rs.".into(),
            model: Some("codex:gpt-6.1-sol:high".into()),
            elapsed_ms: None,
        },
        Entry::User("And who writes them?".into()),
    ]);
    app
}

fn render(app: &mut App) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, 30)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    terminal.backend().buffer().clone()
}

fn row_text(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}

fn row_of(buffer: &Buffer, starts: &str) -> u16 {
    (0..buffer.area.height)
        .find(|y| row_text(buffer, *y).starts_with(starts))
        .unwrap_or_else(|| panic!("no row starts with {starts:?}"))
}

fn mouse(app: &mut App, kind: MouseEventKind, column: u16, row: u16) {
    app.handle(Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }));
}

fn click(app: &mut App, column: u16, row: u16) {
    mouse(app, MouseEventKind::Down(MouseButton::Left), column, row);
    mouse(app, MouseEventKind::Up(MouseButton::Left), column, row);
}

const LABEL: &str = "Codex · GPT-6.1-Sol";

#[test]
fn a_click_on_a_reply_shows_its_model_at_the_right_in_dim_gray() {
    let mut app = chat();
    let buffer = render(&mut app);
    let reply = row_of(&buffer, "● The settings");
    assert!(!row_text(&buffer, reply).contains(LABEL));
    click(&mut app, 4, reply);
    let buffer = render(&mut app);
    let text = row_text(&buffer, reply);
    assert!(text.trim_end().ends_with(LABEL), "{text:?}");
    let column = text[..text.find("Codex").unwrap()].chars().count() as u16;
    assert_eq!(buffer[(column, reply)].fg, theme::GRAY_DIM);
    // The person's question shows the model that answered it.
    let question = row_of(&buffer, "❯ Which");
    click(&mut app, 4, question);
    let buffer = render(&mut app);
    assert!(row_text(&buffer, question).trim_end().ends_with(LABEL));
    assert!(!row_text(&buffer, reply).contains(LABEL), "the label moved");
    // A message with no reply yet shows nothing.
    let unanswered = row_of(&buffer, "❯ And who");
    click(&mut app, 4, unanswered);
    let buffer = render(&mut app);
    assert!(!rows(&buffer).contains(LABEL));
}

fn rows(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| row_text(buffer, y))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_drag_still_selects_and_shows_no_label() {
    let mut app = chat();
    let buffer = render(&mut app);
    let reply = row_of(&buffer, "● The settings");
    let left = MouseButton::Left;
    mouse(&mut app, MouseEventKind::Down(left), 2, reply);
    mouse(&mut app, MouseEventKind::Drag(left), 14, reply);
    mouse(&mut app, MouseEventKind::Up(left), 14, reply);
    let mut copied = None;
    app.copy_pending(|text| {
        copied = Some(text.to_owned());
        Ok(())
    });
    assert_eq!(copied.as_deref(), Some("The settings"));
    let buffer = render(&mut app);
    assert!(!rows(&buffer).contains(LABEL));
}

#[test]
fn the_label_is_never_added_to_the_transcript_or_the_export() {
    let mut app = chat();
    let cwd = std::path::Path::new("/tmp");
    let entries = app.live.entries.clone();
    let before = trajectory::app_document(&app, cwd);
    let buffer = render(&mut app);
    click(&mut app, 4, row_of(&buffer, "● The settings"));
    assert!(rows(&render(&mut app)).contains(LABEL));
    assert_eq!(app.live.entries, entries);
    let after = trajectory::app_document(&app, cwd);
    assert!(!after.to_string().contains(LABEL));
    // Steps without their times, which an unstamped chat takes from now.
    let steps = |document: &serde_json::Value| {
        let mut steps = document["steps"].clone();
        for step in steps.as_array_mut().into_iter().flatten() {
            step.as_object_mut().map(|step| step.remove("timestamp"));
        }
        steps
    };
    assert_eq!(
        steps(&before),
        steps(&after),
        "the click changed the export"
    );
}
