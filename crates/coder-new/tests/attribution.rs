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
fn busy_chats_show_working_beside_the_spinner_with_or_without_openrouter() {
    let mut app = app();
    app.live.busy = true;
    for (enabled, key_configured) in [(true, true), (true, false), (false, false)] {
        app.plugins.enabled = enabled;
        app.plugins.key_configured = key_configured;
        let text = rows(&render(&mut app, 80, 24).0).join("\n");
        assert!(text.contains(&format!(
            "{} Working",
            coder_new::tools::spinner(app.animation_frame)
        )));
        assert!(!text.contains("is replying"));
        assert!(!text.contains("is working"));
    }
}

#[test]
fn completed_reply_footer_keeps_the_model_and_elapsed_time_after_resize() {
    let mut app = app();
    app.live.reply_started_at =
        Some(std::time::Instant::now() - std::time::Duration::from_millis(5500));
    app.live.busy = true;
    app.apply_update(Update::Finished {
        id: app.request_id,
        result: Ok(Streamed {
            text: "Finished reply.".into(),
            model: "anthropic/claude-fable-5.1".into(),
            ..Default::default()
        }),
    });
    let Entry::Assistant { elapsed_ms, .. } = &mut app.live.entries[0] else {
        panic!("expected the completed reply");
    };
    assert!(elapsed_ms.unwrap() >= 5500);
    *elapsed_ms = Some(5500);
    for width in [24, 80, 110] {
        let (buffer, _) = render(&mut app, width, 24);
        let text = rows(&buffer);
        let reply = text
            .iter()
            .position(|line| line.contains("Finished reply."))
            .unwrap();
        let footer = text
            .iter()
            .position(|line| line.contains(" · 5.5s"))
            .unwrap();
        assert!(footer > reply);
        assert!(text[footer].contains("anthropic/"));
        assert!(text[footer].trim_end().ends_with(" · 5.5s"));
        let byte = text[footer].rfind("5.5s").unwrap();
        let x = UnicodeWidthStr::width(&text[footer][..byte]) as u16;
        assert_eq!(buffer[(x, footer as u16)].fg, theme::GRAY);
    }
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
    assert!(
        text[..model_row]
            .iter()
            .any(|line| line.contains("Hello from the routed model."))
    );
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
            .position(|line| line.starts_with('─') && line.contains(DEFAULT_MODEL))
            .unwrap();
        assert!(text[top].starts_with('─') && text[top].ends_with('─'));
        assert!(text[top + 1].starts_with(" ❯ "));
        assert!(text[top + 2].chars().all(|c| c == '─'));
        assert_eq!(cursor, (3, (top + 1) as u16));
        let byte = text[top].find(DEFAULT_MODEL).unwrap();
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
    assert!(text[top + 4].chars().all(|c| c == '─'));
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
        elapsed_ms: None,
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
            .any(|line| line.starts_with('─') && line.contains("anthropic/"))
    );
}

#[test]
fn input_rails_show_reasoning_and_leave_the_bottom_rule_empty() {
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
                .rfind(|line| line.starts_with('─'))
                .unwrap()
                .chars()
                .all(|c| c == '─')
        );
    }
    app.plugins.options.reasoning = None;
    let text = rows(&render(&mut app, 110, 24).0);
    assert!(
        text.iter()
            .any(|line| line.trim_matches('─').trim() == "openai/gpt-6-luna")
    );
    app.apply_update(Update::Checked {
        id: app.request_id,
        result: Err("HTTP 401".into()),
    });
    let text = rows(&render(&mut app, 110, 24).0);
    assert!(
        text.iter()
            .rfind(|line| line.starts_with('─'))
            .unwrap()
            .chars()
            .all(|c| c == '─')
    );
}
