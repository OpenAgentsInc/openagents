use coder_new::{
    App, Mode,
    live::{Entry, Update},
    models::DEFAULT_MODEL,
    theme, ui,
};
use openrouter::Streamed;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use unicode_width::UnicodeWidthStr;

/// Points the plan's usage book at an empty folder, so the bottom rule never
/// carries this computer's Claude plan ("5h 33% · week 82%").
fn without_this_computers_plan() {
    static EMPTY: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    EMPTY.get_or_init(|| {
        let empty = tempfile::tempdir().unwrap();
        // Tests run in threads; every one renders through here first and
        // wants the book empty, so the write races with nothing that cares.
        unsafe { std::env::set_var("OPENAGENTS_TASKS", empty.path()) };
        empty
    });
}

fn render(app: &mut App, width: u16, height: u16) -> (Buffer, (u16, u16)) {
    without_this_computers_plan();
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

/// The transcript's rows: everything but the input's box.
fn transcript(buffer: &Buffer) -> Vec<String> {
    rows(buffer)
        .into_iter()
        .filter(|line| !line.starts_with(['┌', '│', '└']))
        .collect()
}

/// The input box's top rule, which names the mode (`coder`, `openrouter`, or
/// `claude`) or the model the person chose in /models.
fn top_rule(text: &[String]) -> Option<usize> {
    text.iter().position(|line| line.starts_with('┌'))
}

/// The label a rule carries, without its line and corners.
fn label(line: &str) -> &str {
    line.trim_matches(['─', '┌', '┐', '└', '┘']).trim()
}

fn app() -> App {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.plugins.bundled.claude = false;
    app.plugins.enabled = true;
    app.plugins.key_configured = true;
    app
}

#[test]
fn busy_chats_show_working_on_the_input_rail_with_or_without_openrouter() {
    let mut app = app();
    app.live.busy = true;
    for (enabled, key_configured) in [(true, true), (true, false), (false, false)] {
        app.plugins.enabled = enabled;
        app.plugins.key_configured = key_configured;
        let text = rows(&render(&mut app, 80, 24).0);
        let top = top_rule(&text).unwrap();
        // The spinner rides the input's top-left rail, beside the mode.
        assert!(
            label(&text[top]).starts_with(&format!(
                "{} Working",
                coder_new::tools::spinner(app.animation_frame)
            )),
            "{:?}",
            text[top]
        );
        // The transcript above stays free of it.
        assert!(
            !transcript(&render(&mut app, 80, 24).0)
                .join("\n")
                .contains("Working")
        );
        assert!(!text.join("\n").contains("is replying"));
        assert!(!text.join("\n").contains("is working"));
    }
}

#[test]
fn completed_reply_shows_neither_the_model_nor_the_elapsed_time() {
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
    // The entry keeps both for exports; the screen shows neither.
    let Entry::Assistant {
        elapsed_ms, model, ..
    } = &app.live.entries[0]
    else {
        panic!("expected the completed reply");
    };
    assert!(elapsed_ms.unwrap() >= 5500);
    assert_eq!(model.as_deref(), Some("anthropic/claude-fable-5.1"));
    for width in [24, 80, 110] {
        let text = transcript(&render(&mut app, width, 24).0);
        assert!(text.iter().any(|line| line.contains("Finished reply.")));
        assert!(!text.iter().any(|line| line.contains("anthropic/")));
        assert!(!text.iter().any(|line| line.contains("5.5s")));
    }
}

#[test]
fn reply_keeps_its_actual_model_without_showing_it_in_the_transcript() {
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
    assert!(matches!(
        &app.live.entries[1],
        Entry::Assistant { model: Some(model), .. } if model == "openai/gpt-6-luna"
    ));
    let (buffer, _) = render(&mut app, 110, 36);
    let text = transcript(&buffer);
    assert!(
        text.iter()
            .any(|line| line.starts_with("● Hello from the routed model."))
    );
    assert!(!text.iter().any(|line| line.contains("openai/gpt-6-luna")));
    assert_eq!(
        label(&rows(&buffer)[top_rule(&rows(&buffer)).unwrap()]),
        "Coder"
    );
    app.plugins.model = "anthropic/claude-fable-5.1".into();
    let (buffer, _) = render(&mut app, 110, 36);
    assert!(
        !transcript(&buffer)
            .iter()
            .any(|line| line.contains("openai/gpt-6-luna"))
    );
    assert_eq!(
        label(&rows(&buffer)[top_rule(&rows(&buffer)).unwrap()]),
        "claude-fable-5.1"
    );
    assert!(!rows(&buffer).iter().any(|line| line.contains("anthropic/")));
}

#[test]
fn composer_registration_follows_enablement_and_preserves_input_geometry() {
    let mut app = app();
    for (width, height) in [(24, 12), (80, 24), (110, 36)] {
        let (buffer, cursor) = render(&mut app, width, height);
        let text = rows(&buffer);
        let top = top_rule(&text).unwrap();
        assert_eq!(label(&text[top]), "Coder");
        assert!(text[top].ends_with('┐'));
        assert!(text[top + 1].starts_with("│❯ ") && text[top + 1].ends_with('│'));
        assert!(text[top + 2].starts_with('└') && text[top + 2].ends_with('┘'));
        assert_eq!(cursor, (3, (top + 1) as u16));
        let byte = text[top].find("Coder").unwrap();
        let x = UnicodeWidthStr::width(&text[top][..byte]) as u16;
        // Which engine runs is a quiet fact in gray, never a colored
        // control that reads as a mode (owner, 2026-10-12).
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
    let top = top_rule(&text).unwrap();
    assert_eq!(label(&text[top]), "Coder");
    assert_eq!(cursor, (8, (top + 3) as u16));
    assert!(text[top + 4].starts_with('└'));
}

/// A Claude Code chat names its harness on the same rule, and its working
/// shows like a live reply's until it answers.
#[test]
fn claude_code_names_its_harness_and_shows_its_working() {
    let mut app = app();
    app.live.runs_on = Some(coder_new::harness_plugin::Harness::ClaudeCode);
    app.draft.text = "hello".into();
    app.draft.cursor = 5;
    app.submit_live();
    app.apply_update(Update::Delta {
        id: app.request_id,
        text: "The fixture replied.".into(),
    });
    let (buffer, _) = render(&mut app, 110, 36);
    let text = rows(&buffer);
    let top = top_rule(&text).unwrap();
    // The mode's name stays on the rail; while the turn runs, the working
    // spinner rides its left end.
    assert!(
        label(&text[top]).contains("Claude ("),
        "{}",
        label(&text[top])
    );
    assert!(text.iter().any(|line| line.contains("hello")));
    let answer = text
        .iter()
        .position(|line| line.contains("The fixture replied."))
        .unwrap();
    assert_eq!(transcript(&buffer)[answer].trim().chars().next(), Some('●'));
}

#[test]
fn streaming_and_stopped_replies_show_no_model_and_never_guess_from_settings() {
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
    let text = transcript(&render(&mut app, 110, 36).0);
    assert!(
        text.iter()
            .any(|line| line.starts_with("● A partial reply."))
    );
    assert!(!text.iter().any(|line| line.contains("x-ai/grok-4.7")));
    app.cancel_request();
    app.plugins.model = "google/gemini-3.5-flash".into();
    assert!(app.live.entries.iter().any(|entry| matches!(
        entry,
        Entry::Assistant { model: Some(model), .. } if model == "x-ai/grok-4.7"
    )));
    let text = transcript(&render(&mut app, 110, 36).0);
    assert!(!text.iter().any(|line| line.contains("x-ai/grok-4.7")));
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
    let text = transcript(&render(&mut app, 110, 36).0);
    assert!(
        !text
            .iter()
            .any(|line| line.contains("google/gemini-3.5-flash"))
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
    assert!(text[top_rule(&text).unwrap()].contains("claude-fable"));
    assert!(!text.iter().any(|line| line.contains("anthropic/")));
}

#[test]
fn input_rails_name_the_chosen_model_only_and_the_bottom_rule_names_the_repository() {
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
        // The rail names the chosen model only, never its settings.
        assert!(text[top_rule(&text).unwrap()].contains("gpt-6"));
        assert!(!text.iter().any(|line| line.contains(":low")));
        // The bottom rule names the repository, never the key's status.
        let bottom = text.iter().rfind(|line| line.starts_with('└')).unwrap();
        assert_eq!(label(bottom), "openagents");
    }
    app.plugins.options.reasoning = None;
    let text = rows(&render(&mut app, 110, 24).0);
    assert_eq!(label(&text[top_rule(&text).unwrap()]), "gpt-6-luna");
    app.apply_update(Update::Checked {
        id: app.request_id,
        result: Err("HTTP 401".into()),
    });
    let text = rows(&render(&mut app, 110, 24).0);
    let bottom = text.iter().rfind(|line| line.starts_with('└')).unwrap();
    assert_eq!(label(bottom), "openagents");
    assert!(!text.iter().any(|line| line.contains("401")));
}
