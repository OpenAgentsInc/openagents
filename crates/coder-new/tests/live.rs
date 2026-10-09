use coder_new::{
    App, Mode, Screen,
    live::{Entry, Request, Update, Work},
    plugins::{Connection, SettingsFocus},
    provider::KeyInfo,
    slash::Command,
    snapshot, ui,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use openrouter::Streamed;
use ratatui::{Terminal, backend::TestBackend};

const KEY_INPUT: &str = "redaction_probe";

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

fn paste(app: &mut App, text: &str) {
    assert!(app.handle(Event::Paste(text.into())));
}

fn focus(app: &mut App, target: SettingsFocus) {
    for _ in 0..6 {
        if app.plugins.focus == target {
            return;
        }
        key(app, KeyCode::Tab);
    }
    panic!("settings focus did not reach the requested field");
}

fn render(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    if terminal.backend().cursor_visible() {
        let cursor = terminal.get_cursor_position().unwrap();
        assert!(cursor.x < width && cursor.y < height);
    }
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

fn assert_key_hidden(app: &mut App) {
    for (width, height) in [(80, 24), (24, 12)] {
        assert!(!render(app, width, height).contains(KEY_INPUT));
        let svg = snapshot::svg(app, width, height);
        let text: String = svg
            .lines()
            .filter(|line| line.starts_with("<text "))
            .map(|line| {
                let (_, text) = line.split_once('>').unwrap();
                text.strip_suffix("</text>").unwrap()
            })
            .collect();
        assert!(!text.contains(KEY_INPUT));
    }
}

fn save_key(app: &mut App, input: &str, model: &str) {
    app.open_plugin_settings();
    paste(app, input);
    key(app, KeyCode::Enter);
    key(app, KeyCode::Home);
    for _ in 0..app.plugins.field(false).0.chars().count() {
        key(app, KeyCode::Delete);
    }
    paste(app, model);
    key(app, KeyCode::Enter);
    key(app, KeyCode::Enter);
    assert!(app.screen == Screen::Plugins);
}

fn take_check(app: &mut App, expected_key: &str) -> u64 {
    let request = app.request.take().expect("a key check was queued");
    assert!(matches!(request.kind, Work::Check));
    assert_eq!(request.id, app.request_id);
    assert_eq!(request.key.expose(), expected_key);
    assert!(!format!("{:?}", request.key).contains(expected_key));
    request.id
}

fn verified(app: &mut App, id: u64) {
    app.apply_update(Update::Checked {
        id,
        result: Ok(KeyInfo {
            status: "Verified",
            limit_remaining: Some(5.0),
        }),
    });
}

fn live_app() -> App {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.open_plugins();
    key(&mut app, KeyCode::Char(' '));
    save_key(&mut app, KEY_INPUT, "");
    let id = take_check(&mut app, KEY_INPUT);
    verified(&mut app, id);
    key(&mut app, KeyCode::Esc);
    assert!(app.screen == Screen::Conversation);
    app
}

fn submit(app: &mut App, text: &str) -> Request {
    paste(app, text);
    key(app, KeyCode::Enter);
    let request = app.request.take().expect("a chat request was queued");
    assert!(matches!(request.kind, Work::Chat { .. }));
    assert_eq!(request.id, app.request_id);
    request
}

fn transcript(app: &App) -> Vec<(&str, &str)> {
    app.live
        .entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::User(text) => Some(("user", text.as_str())),
            Entry::Assistant { text, .. } => Some(("assistant", text.as_str())),
            Entry::Tool { .. } | Entry::Delegation { .. } => None,
        })
        .collect()
}

fn reply(text: &str, tokens: u64) -> Streamed {
    let mut reply = Streamed {
        text: text.into(),
        ..Streamed::default()
    };
    reply.usage.total_tokens = tokens;
    reply
}

#[test]
fn slash_picker_completes_and_executes_commands_without_submitting_messages() {
    let mut app = App::default();
    paste(&mut app, "/");
    assert_eq!(
        app.slash_hints(),
        [
            Command::Demo,
            Command::Plugins,
            Command::Appearance,
            Command::Export,
            Command::Resume,
            Command::Login,
            Command::Help
        ]
    );
    key(&mut app, KeyCode::Up);
    assert_eq!(app.slash_selected, 0);
    for _ in 0..6 {
        key(&mut app, KeyCode::Down);
    }
    assert_eq!(app.slash_selected, 6);
    assert_eq!(app.selected_agent, None);
    assert!(render(&mut app, 80, 24).contains("❯ /help"));
    for _ in 0..5 {
        key(&mut app, KeyCode::Up);
    }
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.draft.text, "/plugins");
    assert_eq!(app.draft.cursor, "/plugins".len());
    assert!(app.screen == Screen::Conversation);
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::Plugins);
    assert!(app.draft.text.is_empty());
    key(&mut app, KeyCode::Esc);

    paste(&mut app, "/h");
    assert_eq!(app.slash_hints(), [Command::Help]);
    key(&mut app, KeyCode::Enter);
    assert!(app.notice.as_deref().unwrap().contains("/plugins"));
    paste(&mut app, "/unknown");
    assert!(app.slash_hints().is_empty());
    key(&mut app, KeyCode::Enter);
    assert!(app.notice.as_deref().unwrap().contains("Unknown command"));
    assert!(app.messages.is_empty());
    assert!(app.live.entries.is_empty());
    assert!(app.request.is_none());

    paste(&mut app, "/");
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.draft.text, "/");
    assert!(app.slash_hints().is_empty());
    key(&mut app, KeyCode::Backspace);
    paste(&mut app, "/d");
    assert_eq!(app.slash_hints(), [Command::Demo]);
    key(&mut app, KeyCode::Enter);
    assert!(app.mode == Mode::Live);
    assert!(app.draft.text.is_empty());
    assert!(app.request.is_none());
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Up);
    assert_eq!(app.selected_agent, None);
    app.draft = Default::default(); // Up now recalls prompt history.
    paste(&mut app, "/unknown");
    key(&mut app, KeyCode::Enter);
    assert!(app.request.is_none());
    assert!(app.live.entries.is_empty());
    paste(&mut app, "/demo");
    key(&mut app, KeyCode::Enter);
    assert!(app.mode == Mode::Demo);
    assert!(app.messages.is_empty());
    assert!(app.request.is_none());
}

#[test]
fn live_chat_uses_the_no_setup_fallback_or_an_enabled_saved_key() {
    let mut fallback = App::default();
    fallback.set_mode(Mode::Live);
    fallback.plugins.bundled.microcoder = false;
    paste(&mut fallback, "live question");
    key(&mut fallback, KeyCode::Enter);
    let request = fallback.request.take().unwrap();
    assert!(request.key.expose().is_empty());
    assert!(matches!(request.kind, Work::Microcoder { .. }));
    assert!(fallback.live.busy);
    assert!(fallback.draft.text.is_empty());
    assert_eq!(transcript(&fallback), [("user", "live question")]);
    fallback.cancel_request();

    let mut app = App::default();
    app.set_mode(Mode::Live);
    save_key(&mut app, KEY_INPUT, "");
    assert!(app.plugins.key_configured);
    assert!(app.plugins.field(true).0.is_empty());
    let check_id = take_check(&mut app, KEY_INPUT);
    assert_key_hidden(&mut app);
    verified(&mut app, check_id);
    assert_eq!(app.plugins.status(), "Verified");
    app.open_plugin_settings();
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.plugins.status(), "Verified");
    assert!(app.request.is_none());
    key(&mut app, KeyCode::Esc);
    paste(&mut app, "live question");
    key(&mut app, KeyCode::Enter);
    let request = app.request.take().unwrap();
    assert_eq!(request.key.expose(), KEY_INPUT);
    assert!(!format!("{:?}", request.key).contains(KEY_INPUT));
    match request.kind {
        Work::Chat {
            model,
            messages,
            options,
            ..
        } => {
            assert_eq!(model, "openrouter/free");
            assert_eq!(options, coder_new::models::GenerationOptions::default());
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].role, "user");
            assert_eq!(messages[0].content, "live question");
        }
        _ => panic!("expected a chat request"),
    }
    assert!(app.live.busy);
    assert!(app.draft.text.is_empty());
    assert!(app.messages.is_empty());
    assert_eq!(transcript(&app), [("user", "live question")]);
    let rendered = render(&mut app, 80, 24);
    assert!(rendered.contains("live question"));
    assert!(rendered.contains(&format!(
        "{} Working",
        coder_new::tools::spinner(app.animation_frame)
    )));
    assert_key_hidden(&mut app);
}

#[test]
fn testing_a_key_uses_the_draft_and_ignores_results_after_edit_or_cancel() {
    let mut app = App::default();
    app.open_plugin_settings();
    paste(&mut app, KEY_INPUT);
    focus(&mut app, SettingsFocus::TestKey);
    key(&mut app, KeyCode::Enter);
    assert!(app.request.is_none());
    assert!(app.plugins.key_for_check().is_none());
    focus(&mut app, SettingsFocus::Save);
    key(&mut app, KeyCode::Enter);
    assert!(app.plugins.key_configured);
    assert!(app.plugins.key_for_request().is_none());
    assert!(app.request.is_none());

    app.set_mode(Mode::Live);
    app.open_plugin_settings();
    paste(&mut app, KEY_INPUT);
    focus(&mut app, SettingsFocus::TestKey);
    key(&mut app, KeyCode::Enter);
    let first_id = take_check(&mut app, KEY_INPUT);
    assert!(!app.plugins.key_configured);
    assert!(app.plugins.key_for_request().is_none());
    assert!(matches!(app.plugins.connection, Connection::Checking));
    assert_key_hidden(&mut app);
    focus(&mut app, SettingsFocus::ApiKey);
    key(&mut app, KeyCode::Char('x'));
    assert_ne!(app.request_id, first_id);
    verified(&mut app, first_id);
    assert!(matches!(app.plugins.connection, Connection::Unchecked));
    key(&mut app, KeyCode::Backspace);
    focus(&mut app, SettingsFocus::TestKey);
    key(&mut app, KeyCode::Enter);
    let second_id = take_check(&mut app, KEY_INPUT);
    key(&mut app, KeyCode::Esc);
    verified(&mut app, second_id);
    assert!(!app.plugins.key_configured);
    assert!(!matches!(app.plugins.connection, Connection::Verified));
    assert!(app.plugins.key_for_request().is_none());
    assert!(app.plugins.field(true).0.is_empty());

    save_key(&mut app, KEY_INPUT, "test-model");
    let saved_id = take_check(&mut app, KEY_INPUT);
    verified(&mut app, saved_id);
    app.open_plugin_settings();
    focus(&mut app, SettingsFocus::TestKey);
    key(&mut app, KeyCode::Enter);
    let test_id = take_check(&mut app, KEY_INPUT);
    app.apply_update(Update::Checked {
        id: test_id,
        result: Err("OpenRouter rejected the API key (HTTP 401).".into()),
    });
    assert!(matches!(app.plugins.connection, Connection::Failed(_)));
    assert!(app.plugins.key_configured);
    assert_key_hidden(&mut app);
}

#[test]
fn demo_and_live_keep_separate_preferences_drafts_and_transcripts() {
    let mut app = App::default();
    app.open_plugins();
    key(&mut app, KeyCode::Char(' '));
    save_key(&mut app, "demo_probe", "demo-model");
    assert!(app.request.is_none());
    key(&mut app, KeyCode::Esc);
    paste(&mut app, "demo message");
    key(&mut app, KeyCode::Enter);
    paste(&mut app, "unsent demo draft");
    key(&mut app, KeyCode::Left);
    let demo_draft = (app.draft.text.clone(), app.draft.cursor);

    app.set_mode(Mode::Live);
    assert!(app.draft.text.is_empty());
    assert!(!app.plugins.enabled);
    assert!(!app.plugins.key_configured);
    assert_eq!(app.plugins.model, "openrouter/free");
    app.open_plugins();
    key(&mut app, KeyCode::Char(' '));
    save_key(&mut app, KEY_INPUT, "live-model");
    let check_id = take_check(&mut app, KEY_INPUT);
    verified(&mut app, check_id);
    key(&mut app, KeyCode::Esc);
    let request = submit(&mut app, "live message");
    app.apply_update(Update::Finished {
        id: request.id,
        result: Ok(reply("live answer", 9)),
    });
    paste(&mut app, "unsent live draft");
    key(&mut app, KeyCode::Left);
    let live_draft = (app.draft.text.clone(), app.draft.cursor);
    let live_canvas = render(&mut app, 110, 70);
    assert!(live_canvas.contains("live answer"));
    assert!(!live_canvas.contains("demo message"));
    assert!(!live_canvas.contains("Delegate claude-code"));
    app.open_plugin_settings();
    paste(&mut app, "replacement_probe");
    focus(&mut app, SettingsFocus::TestKey);
    key(&mut app, KeyCode::Enter);
    let pending_id = take_check(&mut app, "replacement_probe");

    app.set_mode(Mode::Demo);
    assert_eq!((app.draft.text.clone(), app.draft.cursor), demo_draft);
    assert!(app.plugins.enabled);
    assert!(app.plugins.key_configured);
    assert_eq!(app.plugins.model, "demo-model");
    assert!(app.plugins.key_for_request().is_none());
    assert!(app.plugins.field(true).0.is_empty());
    assert!(app.request.is_none());
    app.apply_update(Update::Checked {
        id: pending_id,
        result: Err("late check result".into()),
    });
    assert_ne!(app.plugins.connection_label(), "late check result");
    let demo_canvas = render(&mut app, 110, 70);
    assert!(demo_canvas.contains("demo message"));
    assert!(!demo_canvas.contains("live answer"));

    app.set_mode(Mode::Live);
    assert_eq!((app.draft.text.clone(), app.draft.cursor), live_draft);
    assert!(app.plugins.enabled);
    assert!(app.plugins.key_configured);
    assert_eq!(app.plugins.model, "live-model");
    assert_eq!(app.plugins.key_for_request().unwrap().expose(), KEY_INPUT);
    assert_eq!(
        transcript(&app),
        [("user", "live message"), ("assistant", "live answer")]
    );
    assert_eq!(app.live.tokens, 9);
    assert!(app.plugins.field(true).0.is_empty());
    assert_key_hidden(&mut app);
}

#[test]
fn live_streaming_preserves_stopped_text_and_ignores_stale_updates() {
    let mut app = live_app();
    let first = submit(&mut app, "first question");
    app.apply_update(Update::Model {
        id: first.id,
        model: "provider/stopped-model".into(),
    });
    app.apply_update(Update::Delta {
        id: first.id,
        text: "partial reply".into(),
    });
    assert!(app.live.busy);
    assert_eq!(app.live.partial, "partial reply");
    assert_eq!(
        app.live.partial_model.as_deref(),
        Some("provider/stopped-model")
    );
    assert!(render(&mut app, 80, 24).contains("partial reply"));

    app.open_plugin_settings();
    key(&mut app, KeyCode::Esc);
    assert!(app.live.busy);
    assert_eq!(app.request_id, first.id);
    assert_eq!(app.plugins.status(), "Verified");
    key(&mut app, KeyCode::Esc);
    app.open_plugin_settings();
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    assert!(app.live.busy);
    assert_eq!(app.request_id, first.id);
    assert!(app.request.is_none());
    key(&mut app, KeyCode::Esc);
    paste(&mut app, "follow-up question");
    key(&mut app, KeyCode::Enter);
    assert!(app.draft.text.is_empty());
    assert!(app.request.is_none());
    assert!(app.live.busy);
    key(&mut app, KeyCode::Esc); // Retrieve queued input without stopping.
    assert!(app.live.busy);
    assert_eq!(app.draft.text, "follow-up question");
    key(&mut app, KeyCode::Esc); // No queue remains: stop active work.
    assert!(!app.live.busy);
    assert!(app.live.partial.is_empty());
    assert!(app.live.partial_model.is_none());
    assert!(matches!(
        &app.live.entries[1],
        Entry::Assistant { model, .. }
            if model.as_deref() == Some("provider/stopped-model")
    ));
    assert_eq!(
        transcript(&app),
        [("user", "first question"), ("assistant", "partial reply")]
    );
    app.apply_update(Update::Delta {
        id: first.id,
        text: "late delta".into(),
    });
    app.apply_update(Update::Model {
        id: first.id,
        model: "provider/late-model".into(),
    });
    app.apply_update(Update::Finished {
        id: first.id,
        result: Ok(reply("late answer", 999)),
    });
    app.apply_update(Update::Checked {
        id: first.id,
        result: Err("late key error".into()),
    });
    assert_eq!(app.live.tokens, 0);
    assert_eq!(app.live.entries.len(), 2);
    assert!(matches!(app.plugins.connection, Connection::Verified));
    assert!(!render(&mut app, 80, 24).contains("late"));

    // Retrieved input is still owned by the composer after stopping.
    key(&mut app, KeyCode::Enter);
    let second = app.request.take().unwrap();
    assert_ne!(second.id, first.id);
    assert!(app.live.partial_model.is_none());
    app.apply_update(Update::Model {
        id: first.id,
        model: "provider/late-model".into(),
    });
    assert!(app.live.partial_model.is_none());
    match second.kind {
        Work::Chat { messages, .. } => {
            let contents: Vec<_> = messages
                .iter()
                .map(|message| (message.role.as_str(), message.content.as_str()))
                .collect();
            assert_eq!(
                contents,
                [
                    ("user", "first question"),
                    ("assistant", "partial reply"),
                    ("user", "follow-up question"),
                ]
            );
        }
        _ => panic!("expected a chat request"),
    }
    app.apply_update(Update::Delta {
        id: second.id,
        text: "complete".into(),
    });
    app.apply_update(Update::Finished {
        id: second.id,
        result: Ok(reply("complete reply", 7)),
    });
    assert!(!app.live.busy);
    assert!(app.live.partial.is_empty());
    assert!(app.live.notice.is_none());
    assert_eq!(app.live.tokens, 7);
    assert_eq!(app.live.entries.len(), 4);
    assert_eq!(
        transcript(&app).last(),
        Some(&("assistant", "complete reply"))
    );
    assert!(render(&mut app, 80, 24).contains("complete reply"));

    let third = submit(&mut app, "last question");
    app.apply_update(Update::Model {
        id: third.id,
        model: "provider/interrupted-model".into(),
    });
    app.apply_update(Update::Delta {
        id: third.id,
        text: "interrupted reply".into(),
    });
    app.apply_update(Update::Finished {
        id: third.id,
        result: Err("OpenRouter rejected the API key (HTTP 401).".into()),
    });
    assert!(!app.live.busy);
    assert!(app.live.partial.is_empty());
    assert_eq!(app.live.tokens, 7);
    assert_eq!(
        transcript(&app).last(),
        Some(&("assistant", "interrupted reply"))
    );
    assert!(matches!(
        app.live.entries.last().unwrap(),
        Entry::Assistant { model, .. }
            if model.as_deref() == Some("provider/interrupted-model")
    ));
    assert!(app.live.partial_model.is_none());
    assert!(matches!(app.plugins.connection, Connection::Failed(_)));
    let rendered = render(&mut app, 80, 24);
    assert!(rendered.contains("interrupted reply"));
    assert!(rendered.contains("HTTP 401"));
}

#[test]
fn completed_replies_keep_their_served_model_without_a_requested_model_fallback() {
    let mut app = live_app();
    assert_eq!(app.plugins.model, "openrouter/free");
    let first = submit(&mut app, "first question");
    app.apply_update(Update::Model {
        id: first.id,
        model: "provider/initial-model".into(),
    });
    let mut first_reply = reply("first answer", 3);
    first_reply.model = "provider/served-model:free".into();
    app.apply_update(Update::Finished {
        id: first.id,
        result: Ok(first_reply),
    });
    app.apply_update(Update::Model {
        id: first.id,
        model: "provider/late-model".into(),
    });
    assert!(app.live.partial_model.is_none());
    app.plugins.model = "provider/next-model".into();
    let second = submit(&mut app, "second question");
    app.apply_update(Update::Model {
        id: second.id,
        model: "provider/observed-model".into(),
    });
    app.apply_update(Update::Finished {
        id: second.id,
        result: Ok(reply("second answer", 4)),
    });
    let models: Vec<_> = app
        .live
        .entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Assistant { model, .. } => Some(model.as_deref()),
            Entry::User(_) | Entry::Tool { .. } | Entry::Delegation { .. } => None,
        })
        .collect();
    assert_eq!(models, [Some("provider/served-model:free"), None]);
    let sent = serde_json::to_string(&app.live.messages()).unwrap();
    assert!(!sent.contains("provider/"));
    assert!(!sent.contains("openrouter/free"));
    assert!(sent.contains("first answer"));
    assert!(sent.contains("second answer"));
}

#[test]
fn invalid_model_metadata_clears_observed_attribution_and_is_not_stored() {
    let mut app = live_app();
    let request = submit(&mut app, "a question");
    app.apply_update(Update::Delta {
        id: request.id,
        text: "partial answer".into(),
    });
    for invalid in [
        "".to_owned(),
        "provider/model\x1b[31m".to_owned(),
        "provider/model\nforged label".to_owned(),
        "provider/modèl".to_owned(),
        "x".repeat(1025),
    ] {
        app.apply_update(Update::Model {
            id: request.id,
            model: "provider/model".into(),
        });
        assert!(app.live.partial_model.is_some());
        app.apply_update(Update::Model {
            id: request.id,
            model: invalid,
        });
        assert!(app.live.partial_model.is_none());
    }
    app.apply_update(Update::Finished {
        id: request.id,
        result: Err("Fixture stream interrupted.".into()),
    });
    assert!(matches!(
        app.live.entries.last().unwrap(),
        Entry::Assistant { text, model: None, .. } if text == "partial answer"
    ));

    let second = submit(&mut app, "next question");
    let mut reply = reply("complete answer", 1);
    reply.model = "provider/model\x1b[31m".into();
    app.apply_update(Update::Finished {
        id: second.id,
        result: Ok(reply),
    });
    assert!(matches!(
        app.live.entries.last().unwrap(),
        Entry::Assistant { model: None, .. }
    ));
}

#[test]
fn main_transcript_starts_at_the_terminal_top_edge() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.live
        .entries
        .push(Entry::User("Top-edge message".into()));
    for (width, height) in [(80, 24), (24, 12)] {
        let canvas = render(&mut app, width, height);
        assert!(canvas.lines().next().unwrap().contains("Top-edge message"));
    }
}

#[test]
fn run_component_has_one_command_one_status_row_and_multiline_preview() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.live.entries.push(Entry::Tool {
        name: "Run".into(),
        input: serde_json::json!({"command":"echo fixture"}),
        output: serde_json::json!({"command":"echo fixture", "exit":0, "timed_out":false, "seconds":0.1, "output":"first\nsecond\nthird\nfourth\nfifth\nsixth"}),
        running: false,
    });
    let canvas = render(&mut app, 100, 24);
    let rows: Vec<_> = canvas.lines().collect();
    assert!(rows[0].contains("Run echo fixture"));
    assert_eq!(canvas.matches("echo fixture").count(), 1);
    assert!(rows[1].contains("exit: 0"));
    assert!(rows[1].contains(" · timed_out: false"));
    assert!(rows[2].contains("first"));
    assert!(rows[3].contains("second"));
    // The finished card keeps every retained output line; the transcript
    // scrolls instead of eliding them (#11117).
    for (row, text) in ["third", "fourth", "fifth", "sixth"].iter().enumerate() {
        assert!(rows[4 + row].contains(text), "{canvas}");
    }
    assert!(!canvas.contains("more lines"));
    assert!(!canvas.contains("value:"));
    for width in [24, 40] {
        render(&mut app, width, 12);
    }
}

#[test]
fn page_up_from_the_followed_tail_scrolls_back_from_the_latest_output() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    let output: Vec<String> = (1..=60).map(|line| format!("line {line}")).collect();
    app.live.entries.push(Entry::Tool {
        name: "Run".into(),
        input: serde_json::json!({"command":"seq 60"}),
        output: serde_json::json!({"exit":0, "output":output.join("\n")}),
        running: false,
    });
    app.scroll = u16::MAX;
    let latest = render(&mut app, 80, 24);
    assert!(latest.contains("line 60"));
    assert_eq!(app.scroll, u16::MAX, "following keeps the tail");
    key(&mut app, KeyCode::PageUp);
    assert!(app.scroll < u16::MAX - 5);
    let back = render(&mut app, 80, 24);
    assert!(!back.contains("line 60"), "{back}");
    assert!(back.contains("line 50"), "{back}");
    assert!(app.handle(Event::Key(KeyEvent::new(
        KeyCode::End,
        KeyModifiers::CONTROL
    ))));
    assert!(render(&mut app, 80, 24).contains("line 60"));
}

#[test]
fn running_and_failed_run_keep_the_command_visible() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.live.entries.push(Entry::Tool {
        name: "Run".into(),
        input: serde_json::json!({"command":"false"}),
        output: serde_json::Value::Null,
        running: true,
    });
    assert!(render(&mut app, 80, 24).contains("Run false"));
    if let Entry::Tool {
        output, running, ..
    } = &mut app.live.entries[0]
    {
        *running = false;
        *output = serde_json::json!({"error":"Could not start command"});
    }
    let canvas = render(&mut app, 80, 24);
    assert!(canvas.contains("× Run false"));
    assert!(canvas.contains("Could not start command"));
}

#[test]
fn local_codex_model_label_is_shown_above_the_input_rail() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.plugins.enabled = false;
    app.live.partial_model = Some("codex:gpt-6.1-sol".into());
    let screen = render(&mut app, 80, 24);
    assert!(
        screen
            .lines()
            .any(|line| line.contains("codex:gpt-6.1-sol") && line.contains('─'))
    );

    app.live.partial_model = None;
    app.live.entries.push(Entry::Assistant {
        text: "Done".into(),
        model: Some("codex:gpt-6.1-sol".into()),
        elapsed_ms: None,
    });
    let screen = render(&mut app, 80, 24);
    assert!(
        screen
            .lines()
            .any(|line| line.contains("codex:gpt-6.1-sol") && line.contains('─'))
    );
}
