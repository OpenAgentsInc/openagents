use coder_new::{
    App, Mode,
    live::{Entry, Update},
    models::DEFAULT_MODEL,
    theme, ui,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::{Buffer, Cell},
    style::Modifier,
};
use unicode_width::UnicodeWidthStr;

fn render(app: &mut App, width: u16, height: u16) -> (Buffer, (u16, u16)) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    (terminal.backend().buffer().clone(), (cursor.x, cursor.y))
}

fn rows(buffer: &Buffer) -> Vec<String> {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect()
        })
        .collect()
}

fn position(buffer: &Buffer, needle: &str) -> (u16, u16) {
    rows(buffer)
        .iter()
        .enumerate()
        .find_map(|(y, row)| {
            row.find(needle)
                .map(|byte| (UnicodeWidthStr::width(&row[..byte]) as u16, y as u16))
        })
        .unwrap_or_else(|| panic!("missing rendered text: {needle}"))
}

fn cell<'a>(buffer: &'a Buffer, needle: &str) -> &'a Cell {
    &buffer[position(buffer, needle)]
}

fn live_app() -> App {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.plugins.enabled = true;
    app
}

#[test]
fn replies_render_markdown_structure_and_grok_code_styles() {
    let source = "# Available tools\n\n\
        * **Search the web** - retrieve current information.\n\
        * **Call functions** - invoke an API.\n\n\
        Try *carefully*, remove ~~obsolete~~ steps, and read [the guide](https://example.com/guide).\n\n\
        > Keep the result concise.\n\n\
        | Setting | Value |\n\
        | --- | --- |\n\
        | Mode | enabled |\n\n\
        ```rust\n\
        fn render() {\n\
            let message = \"ready\";\n\
        }\n\
        ```";
    let mut app = live_app();
    app.live.entries.push(Entry::Assistant {
        text: source.into(),
        model: Some("openai/gpt-6-luna".into()),
    });
    let (buffer, _) = render(&mut app, 110, 60);
    let text = rows(&buffer).join("\n");
    for raw in ["**", "~~", "```", "# Available", "[the guide](", "| --- |"] {
        assert!(!text.contains(raw), "raw Markdown remained visible: {raw}");
    }
    assert!(text.contains("• Search the web"));
    assert!(text.contains("• Call functions"));
    assert!(text.contains("│ Keep the result concise."));
    assert!(text.contains("Setting") && text.contains("enabled"));
    assert!(text.contains("https://example.com/guide"));
    assert!(
        cell(&buffer, "Available tools")
            .modifier
            .contains(Modifier::BOLD)
    );
    assert!(
        cell(&buffer, "Search the web")
            .modifier
            .contains(Modifier::BOLD)
    );
    assert!(
        cell(&buffer, "carefully")
            .modifier
            .contains(Modifier::ITALIC)
    );
    assert!(
        cell(&buffer, "obsolete")
            .modifier
            .contains(Modifier::CROSSED_OUT)
    );
    assert!(
        cell(&buffer, "the guide")
            .modifier
            .contains(Modifier::UNDERLINED)
    );
    let keyword = cell(&buffer, "fn render()");
    let string = cell(&buffer, "\"ready\"");
    assert_eq!(keyword.bg, theme::BG_DARK);
    assert_eq!(string.bg, theme::BG_DARK);
    assert_ne!(keyword.fg, string.fg);
    assert_eq!(cell(&buffer, "openai/gpt-6-luna").fg, theme::GRAY);
    assert_eq!(app.live.messages()[0].content, source);
    let svg = coder_new::snapshot::svg(&mut app, 110, 60);
    assert!(svg.contains("font-style=\"italic\""));
    assert!(svg.contains("text-decoration=\"underline\""));
    assert!(svg.contains("text-decoration=\"line-through\""));
}

#[test]
fn open_fences_keep_their_highlighting_when_streaming_and_stopping() {
    let mut app = live_app();
    app.live
        .entries
        .push(Entry::User("Show a Rust example.".into()));
    app.live.busy = true;
    app.apply_update(Update::Model {
        id: app.request_id,
        model: "x-ai/grok-4.7".into(),
    });
    app.apply_update(Update::Delta {
        id: app.request_id,
        text: "**Working**\n\n```rust\nfn example() {\n    let label = \"hello\";\n".into(),
    });
    let (streaming, _) = render(&mut app, 80, 24);
    assert!(!rows(&streaming).join("\n").contains("```"));
    assert!(
        cell(&streaming, "Working")
            .modifier
            .contains(Modifier::BOLD)
    );
    assert_eq!(cell(&streaming, "let label").bg, theme::BG_DARK);
    app.apply_update(Update::Delta {
        id: app.request_id,
        text: "    println!(\"{label}\");\n".into(),
    });
    let (continued, _) = render(&mut app, 80, 24);
    assert_eq!(cell(&streaming, "let label"), cell(&continued, "let label"));
    app.cancel_request();
    let (stopped, _) = render(&mut app, 80, 24);
    assert_eq!(cell(&continued, "let label"), cell(&stopped, "let label"));
    assert!(rows(&stopped).join("\n").contains("Reply stopped."));
    assert_eq!(cell(&stopped, "x-ai/grok-4.7").fg, theme::GRAY);
    assert!(app.live.messages()[1].content.contains("```rust"));
}

#[test]
fn user_and_demo_messages_render_markdown_without_changing_the_source_or_composer() {
    let source = "Please review **delta** and `config.toml`.";
    let mut app = live_app();
    app.live.entries.push(Entry::User(source.into()));
    app.draft.text = "**Unsent draft**".into();
    app.draft.cursor = app.draft.text.len();
    let (buffer, cursor) = render(&mut app, 80, 24);
    assert_eq!(app.live.messages()[0].content, source);
    assert!(
        rows(&buffer)
            .join("\n")
            .contains("❯ Please review delta and config.toml.")
    );
    assert!(cell(&buffer, "delta").modifier.contains(Modifier::BOLD));
    assert_eq!(cell(&buffer, "config.toml").fg, theme::MD_CODE);
    let (arrow_x, arrow_y) = position(&buffer, "❯ Please");
    assert_eq!(buffer[(arrow_x, arrow_y)].bg, theme::BG_LIGHT);
    assert_ne!(buffer[(arrow_x, arrow_y - 1)].bg, theme::BG_LIGHT);
    assert_ne!(buffer[(arrow_x, arrow_y + 1)].bg, theme::BG_LIGHT);
    assert!(rows(&buffer).join("\n").contains("❯ **Unsent draft**"));
    assert_eq!(cursor.0, 3 + app.draft.text.len() as u16);

    let mut demo = App::default();
    assert!(demo.handle(Event::Paste(source.into())));
    assert!(demo.handle(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE
    ))));
    assert_eq!(demo.messages, [source]);
    let (buffer, _) = render(&mut demo, 110, 80);
    assert!(
        rows(&buffer)
            .join("\n")
            .contains("❯ Please review delta and config.toml.")
    );
    assert!(cell(&buffer, "delta").modifier.contains(Modifier::BOLD));
    assert_eq!(cell(&buffer, "config.toml").fg, theme::MD_CODE);
    assert!(demo.request.is_none());
}

#[test]
fn narrow_transcripts_keep_long_code_and_table_content_accessible_by_scrolling() {
    let code_token = "code_begin_0123456789_abcdefghijklmnopqrstuvwxyz_code_end";
    let table_token = "table_begin_0123456789_abcdefghijklmnopqrstuvwxyz_table_end";
    let mut app = live_app();
    app.live.entries.push(Entry::Assistant {
        text: format!(
            "## Before the example\n\n```rust\nconst LABEL: &str = \"{code_token}\";\n```\n\n| Output |\n| --- |\n| {table_token} |\n\nAfter the table."
        ),
        model: Some("openai/gpt-6-luna".into()),
    });
    app.draft.text = "draft".into();
    app.draft.cursor = 5;
    let (wide, _) = render(&mut app, 110, 36);
    assert!(rows(&wide).join("\n").contains(code_token));
    assert!(rows(&wide).join("\n").contains(table_token));
    let mut saw_code = false;
    let mut saw_table = false;
    let mut saw_end = false;
    let mut maximum_scroll = 0;
    for requested in 0..256 {
        app.scroll = requested;
        let (buffer, cursor) = render(&mut app, 24, 12);
        let text = rows(&buffer).join("\n");
        let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
        saw_code |= compact.contains(code_token);
        saw_table |= compact.contains(table_token);
        saw_end |= text.contains("After the table.");
        assert!(text.contains("❯ draft"));
        assert_eq!(cursor.0, 8);
        assert!(
            rows(&buffer)
                .iter()
                .any(|row| row.starts_with('─') && row.contains(DEFAULT_MODEL))
        );
        maximum_scroll = maximum_scroll.max(app.scroll);
        if app.scroll < requested {
            break;
        }
    }
    assert!(maximum_scroll > 0);
    assert!(saw_code, "wrapped code lost part of its long token");
    assert!(saw_table, "wrapped table lost part of its long token");
    assert!(
        saw_end,
        "the end of the Markdown reply could not be reached"
    );
    app.scroll = 0;
    let (wide_again, _) = render(&mut app, 110, 36);
    assert_eq!(cell(&wide_again, "openai/gpt-6-luna").fg, theme::GRAY);
    assert!(rows(&wide_again).join("\n").contains(code_token));
}
