use coder_new::{
    App, Mode,
    live::{Entry, Update},
    models::DEFAULT_MODEL,
    theme, ui,
};
use openrouter::Streamed;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use unicode_width::UnicodeWidthStr;

fn render(app: &mut App, width: u16, height: u16) -> (Buffer, (u16, u16)) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    (terminal.backend().buffer().clone(), (cursor.x, cursor.y))
}

fn rows(buffer: &Buffer) -> Vec<String> {
    let area = buffer.area;
    (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

fn app() -> App {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.plugins.enabled = true;
    app.plugins.key_configured = true;
    app
}

#[test]
fn reply_uses_actual_model_and_keeps_it_when_the_selected_model_changes() {
    let mut app = app();
    app.live.entries.push(Entry::User("Say hello.".into()));
    app.live.busy = true;
    app.apply_update(Update::Finished {
        id: app.request_id,
        result: Ok(Streamed {
            text: "Hello from the routed model.".into(),
            model: "openai/gpt-6-luna".into(),
            ..Default::default()
        }),
    });
    let (buffer, _) = render(&mut app, 110, 36);
    let text = rows(&buffer);
    let model_row = text
        .iter()
        .position(|line| line.contains("openai/gpt-6-luna"))
        .unwrap();
    assert!(text[model_row].trim_end().ends_with("openai/gpt-6-luna"));
    assert_eq!(text[model_row].trim_end().chars().count(), 108);
    assert!(text[model_row + 1].contains("Hello from the routed model."));
    let byte = text[model_row].find("openai/").unwrap();
    let x = UnicodeWidthStr::width(&text[model_row][..byte]) as u16;
    assert_eq!(buffer[(x, model_row as u16)].fg, theme::GRAY);
    assert!(
        text.iter()
            .any(|line| line.contains(DEFAULT_MODEL) && line.starts_with('─'))
    );
    app.plugins.model = "anthropic/claude-fable-5.1".into();
    let (buffer, _) = render(&mut app, 110, 36);
    let text = rows(&buffer);
    assert!(
        text.iter()
            .any(|line| line.contains("openai/gpt-6-luna") && !line.starts_with('─'))
    );
    assert!(
        text.iter()
            .any(|line| line.contains("anthropic/claude-fable-5.1") && line.starts_with('─'))
    );
}

#[test]
fn composer_registration_follows_enablement_and_preserves_input_geometry() {
    let mut app = app();
    for (width, height) in [(24, 12), (80, 24), (110, 36)] {
        let (buffer, cursor) = render(&mut app, width, height);
        let text = rows(&buffer);
        let top = text
            .iter()
            .position(|line| line.starts_with('─') && line.contains(":default"))
            .unwrap();
        assert!(text[top].starts_with('─') && text[top].ends_with('─'));
        assert!(text[top + 1].starts_with(" ❯ "));
        assert!(text[top + 2].starts_with('─') && text[top + 2].contains("OpenRouter ready"));
        assert_eq!(cursor, (3, (top + 1) as u16));
        let byte = text[top].find(":default").unwrap();
        let x = UnicodeWidthStr::width(&text[top][..byte]) as u16;
        assert_eq!(buffer[(x, top as u16)].fg, theme::GRAY);
    }
    app.plugins.enabled = false;
    assert!(
        !rows(&render(&mut app, 110, 36).0)
            .iter()
            .any(|line| line.contains(DEFAULT_MODEL))
    );
    app.plugins.enabled = true;
    app.draft.text = "first\nsecond\nthird".into();
    app.draft.cursor = app.draft.text.len();
    let (buffer, cursor) = render(&mut app, 80, 24);
    let text = rows(&buffer);
    let top = text
        .iter()
        .position(|line| line.contains(DEFAULT_MODEL))
        .unwrap();
    assert_eq!(cursor, (8, (top + 3) as u16));
    assert!(text[top + 4].starts_with('─') && text[top + 4].contains("OpenRouter ready"));
}

#[test]
fn streaming_attribution_stays_with_a_stopped_reply_and_never_guesses_from_settings() {
    let mut app = app();
    app.live.busy = true;
    app.apply_update(Update::Model {
        id: app.request_id,
        model: "x-ai/grok-4.7".into(),
    });
    app.apply_update(Update::Delta {
        id: app.request_id,
        text: "A partial reply.".into(),
    });
    assert!(
        rows(&render(&mut app, 110, 36).0)
            .iter()
            .any(|line| line.contains("x-ai/grok-4.7"))
    );
    app.cancel_request();
    app.plugins.model = "google/gemini-3.5-flash".into();
    assert!(
        rows(&render(&mut app, 110, 36).0)
            .iter()
            .any(|line| line.contains("x-ai/grok-4.7"))
    );
    app.live.entries.clear();
    app.live.notice = None;
    app.live.busy = true;
    app.apply_update(Update::Finished {
        id: app.request_id,
        result: Ok(Streamed {
            text: "No model metadata.".into(),
            ..Default::default()
        }),
    });
    let text = rows(&render(&mut app, 110, 36).0);
    assert_eq!(
        text.iter()
            .filter(|line| line.contains("google/gemini-3.5-flash"))
            .count(),
        1
    );
    assert!(text.iter().any(|line| line.contains("No model metadata.")));
}

#[test]
fn long_attribution_does_not_overwrite_the_reply_or_the_draft_after_resize() {
    let mut app = app();
    app.plugins.model = "anthropic/claude-fable-5.1".into();
    app.live.entries.push(Entry::Assistant {
        text: "Visible reply body.".into(),
        model: Some("anthropic/claude-fable-5.1".into()),
    });
    app.draft.text = "draft".into();
    app.draft.cursor = 5;
    let (buffer, cursor) = render(&mut app, 24, 12);
    let text = rows(&buffer);
    assert!(text.iter().any(|line| line.contains("Visible reply body.")));
    assert!(text.iter().any(|line| line.contains("❯ draft")));
    assert_eq!(cursor.0, 8);
    assert!(
        text.iter()
            .any(|line| line.starts_with('─') && line.contains(":default"))
    );
}

#[test]
fn input_rails_show_provider_connection_and_explicit_or_default_reasoning() {
    let mut app = app();
    app.plugins.model = "openai/gpt-6-luna".into();
    app.plugins.options.reasoning = Some("low".into());
    app.apply_update(Update::Checked {
        id: app.request_id,
        result: Ok(coder_new::provider::KeyInfo {
            status: "verified",
            limit_remaining: None,
        }),
    });
    for width in [24, 80, 110] {
        let text = rows(&render(&mut app, width, 24).0);
        assert!(
            text.iter()
                .any(|line| line.starts_with('─') && line.contains(":low"))
        );
        assert!(
            text.iter()
                .any(|line| line.contains("OpenRouter connected") || line.contains("OpenRouter ✓"))
        );
    }
    app.plugins.options.reasoning = None;
    let text = rows(&render(&mut app, 110, 24).0);
    assert!(
        text.iter()
            .any(|line| line.contains("openai/gpt-6-luna:default"))
    );
    app.apply_update(Update::Checked {
        id: app.request_id,
        result: Err("HTTP 401".into()),
    });
    let text = rows(&render(&mut app, 110, 24).0);
    assert!(
        text.iter()
            .any(|line| line.contains("OpenRouter connection error"))
    );
    assert!(
        !text
            .iter()
            .any(|line| line.contains("OpenRouter connected"))
    );
}
