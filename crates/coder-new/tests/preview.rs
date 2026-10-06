use coder_new::{App, Screen, snapshot, ui};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

fn screen(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn both_views_and_tiny_terminals_render() {
    let mut app = App::default();
    let conversation = screen(&mut app, 110, 36);
    assert!(conversation.contains("Conversation first"));
    assert!(conversation.contains("Message"));
    assert!(conversation.contains("Sample data"));
    key(&mut app, KeyCode::Tab);
    assert!(screen(&mut app, 110, 36).contains("What do you want to build?"));
    for (width, height) in [(80, 24), (40, 12), (24, 10), (23, 9), (1, 1)] {
        screen(&mut app, width, height);
        app.screen = Screen::Conversation;
        app.handle(Event::Paste(
            "A long draft with 界 and multiple\nlines to wrap across a small terminal.".into(),
        ));
        screen(&mut app, width, height);
    }
}

#[test]
fn paste_and_enter_remain_local_and_key_releases_do_not_send_twice() {
    let mut app = App::default();
    app.handle(Event::Paste("first\r\nsecond\tline\u{1b}".into()));
    assert_eq!(app.draft.text, "first\nsecond    line");
    assert!(app.messages.is_empty());
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.messages, ["first\nsecond    line"]);
    let mut release = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    release.kind = KeyEventKind::Release;
    app.handle(Event::Key(release));
    assert_eq!(app.messages.len(), 1);
    assert!(app.draft.text.is_empty());
    let rendered = screen(&mut app, 80, 24);
    assert!(rendered.contains("No agent is connected."));
    assert!(rendered.contains("❯ first"));
    assert!(rendered.contains("second    line"));
    assert!(!rendered.contains("firstsecond"));
    assert!(!app.handle(Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL
    ))));
}

#[test]
fn smallest_supported_viewport_keeps_the_draft_and_cursor_visible() {
    let mut app = App::default();
    app.handle(Event::Paste("hello".into()));
    let mut terminal = Terminal::new(TestBackend::new(24, 12)).unwrap();
    terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    assert!(cursor.x < 24 && cursor.y < 12);
    assert!(screen(&mut app, 24, 12).contains("hello"));
    assert!(screen(&mut app, 24, 11).contains("Resize to continue."));
}

#[test]
fn deleting_a_separator_keeps_the_cursor_on_the_merged_grapheme_boundary() {
    let mut app = App::default();
    app.handle(Event::Paste("a\n\u{301}".into()));
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Delete);
    assert_eq!(app.draft.text, "a\u{301}");
    assert_eq!(app.draft.wrapped(10).1, (1, 0));
    key(&mut app, KeyCode::Backspace);
    assert!(app.draft.text.is_empty());
}

#[test]
fn editing_and_wrapping_preserve_graphemes_and_terminal_cells() {
    let mut app = App::default();
    app.handle(Event::Paste("a👩‍💻e\u{301}界".into()));
    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.draft.text, "a👩‍💻界");
    key(&mut app, KeyCode::Delete);
    assert_eq!(app.draft.text, "a👩‍💻");
    let (lines, cursor) = app.draft.wrapped(3);
    assert_eq!(lines, ["a👩‍💻", ""]);
    assert_eq!(cursor, (0, 1));
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Delete);
    assert_eq!(app.draft.text, "👩‍💻");
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Backspace);
    assert!(app.draft.text.is_empty());
}

#[test]
fn exported_preview_escapes_drafts_and_uses_the_rendered_canvas() {
    let mut app = App::default();
    app.handle(Event::Paste("<build & test>".into()));
    let svg = snapshot::svg(&mut app, 110, 36);
    assert!(svg.starts_with("<svg "));
    assert!(svg.contains("fill=\"#141414\""));
    assert!(svg.contains("&lt;"));
    assert!(svg.contains("&amp;"));
    assert!(svg.contains("&gt;"));
    assert!(!svg.contains("<build"));
}
